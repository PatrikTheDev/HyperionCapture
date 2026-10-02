use std::path::PathBuf;

use super::{CaptureError, CaptureSource, Frame};

/// Default maximum output height for the KMS GPU conversion pass.
pub const DEFAULT_OUTPUT_HEIGHT: u32 = 480;

/// Policy applied while opening a KMS capture source.
#[derive(Clone, Debug)]
pub struct KmsCaptureOptions {
    /// Stable connector name such as `DP-1`, or the first active connector.
    pub connector: Option<String>,
    /// Maximum output height. Aspect ratio is preserved and smaller inputs are
    /// never enlarged.
    pub max_output_height: u32,
}

impl Default for KmsCaptureOptions {
    fn default() -> Self {
        Self {
            connector: None,
            max_output_height: DEFAULT_OUTPUT_HEIGHT,
        }
    }
}

/// Direct DRM/KMS capture backend.
///
/// The Linux implementation follows Sunshine's KMS path: it selects an active
/// connector and primary plane, exports the plane's current framebuffer as a
/// DMA-BUF, imports it into EGL, and reads the resulting OpenGL texture back as
/// RGB. Resources are deliberately enumerated for every frame so a compositor
/// modeset or replacement (including a switch between Gamescope and a desktop
/// session) does not leave stale object IDs in the capture process.
#[derive(Debug)]
pub struct KmsCapture {
    device: PathBuf,
    #[cfg(target_os = "linux")]
    connector: Option<String>,
    #[cfg(target_os = "linux")]
    max_output_height: u32,
    #[cfg(target_os = "linux")]
    backend: linux::LinuxKmsCapture,
}

impl KmsCapture {
    /// Opens a DRM card with explicit connector and GPU-output policy.
    pub fn open_with_options(
        device: impl Into<PathBuf>,
        options: KmsCaptureOptions,
    ) -> Result<Self, CaptureError> {
        let device = device.into();
        let KmsCaptureOptions {
            connector,
            max_output_height,
        } = options;
        if max_output_height == 0 {
            return Err(CaptureError::Unavailable(
                "KMS maximum output height must be non-zero".to_owned(),
            ));
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (connector, max_output_height);
            Err(CaptureError::Unavailable(format!(
                "DRM/KMS capture only runs on Linux (requested {})",
                device.display()
            )))
        }

        #[cfg(target_os = "linux")]
        {
            let backend = linux::LinuxKmsCapture::open(&device)?;
            Ok(Self {
                device,
                connector,
                max_output_height,
                backend,
            })
        }
    }
}

impl CaptureSource for KmsCapture {
    fn capture(&mut self) -> Result<Frame, CaptureError> {
        #[cfg(not(target_os = "linux"))]
        return Err(CaptureError::Unavailable(format!(
            "DRM/KMS capture only runs on Linux (requested {})",
            self.device.display()
        )));

        #[cfg(target_os = "linux")]
        self.backend
            .capture(self.connector.as_deref(), self.max_output_height)
            .map_err(|error| {
                CaptureError::Unavailable(format!("{}: {error}", self.device.display()))
            })
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        fs::{File, OpenOptions},
        os::fd::{AsFd, BorrowedFd, OwnedFd},
        path::Path,
    };

    use drm::{
        ClientCapability, Device as BasicDevice, buffer,
        control::{Device as ControlDevice, PlaneType, connector, crtc, framebuffer, plane},
    };
    use drm_fourcc::{DrmFourcc, DrmModifier};
    use kms_egl::{
        DmaBufFrame, DmaBufPlane, ReadbackOptions, Reader as EglReader, TransferFunction,
    };

    use super::{CaptureError, Frame};

    #[derive(Debug)]
    struct Card(File);

    impl AsFd for Card {
        fn as_fd(&self) -> BorrowedFd<'_> {
            self.0.as_fd()
        }
    }

    impl BasicDevice for Card {}
    impl ControlDevice for Card {}

    impl Card {
        fn try_clone(&self) -> std::io::Result<Self> {
            self.0.try_clone().map(Self)
        }
    }

    struct EffectiveAdmin;

    impl EffectiveAdmin {
        fn raise() -> Result<Self, String> {
            caps::raise(
                None,
                caps::CapSet::Effective,
                caps::Capability::CAP_SYS_ADMIN,
            )
            .map_err(|error| {
                format!(
                    "cannot enable CAP_SYS_ADMIN to inspect the active framebuffer: {error}; grant it in the permitted capability set"
                )
            })?;
            Ok(Self)
        }
    }

    impl Drop for EffectiveAdmin {
        fn drop(&mut self) {
            let _ = caps::drop(
                None,
                caps::CapSet::Effective,
                caps::Capability::CAP_SYS_ADMIN,
            );
        }
    }

    #[derive(Debug)]
    pub(super) struct LinuxKmsCapture {
        card: Card,
        egl: EglReader,
    }

    #[derive(Debug)]
    struct ActiveFramebuffer {
        connector: String,
        handle: framebuffer::Handle,
        transfer_function: TransferFunction,
    }

    #[derive(Debug)]
    struct FramebufferDescriptor {
        size: (u32, u32),
        format: DrmFourcc,
        buffers: [Option<buffer::Handle>; 4],
        pitches: [u32; 4],
        offsets: [u32; 4],
        modifier: DrmModifier,
        plane_count: usize,
    }

    impl LinuxKmsCapture {
        pub(super) fn open(path: &Path) -> Result<Self, CaptureError> {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .map_err(|error| {
                    CaptureError::Unavailable(format!(
                        "cannot open DRM device {} for reading and writing: {error}",
                        path.display()
                    ))
                })?;
            let card = Card(file);
            // Opening a primary node implicitly grants DRM master when no
            // compositor owns it. Release that ownership before any other
            // initialization (including duplicating the descriptor for EGL).
            // DROP_MASTER checks CAP_SYS_ADMIN even for non-master clients;
            // EINVAL means this descriptor was already not the current master.
            {
                let _admin = EffectiveAdmin::raise().map_err(CaptureError::Unavailable)?;
                match card.release_master_lock() {
                    Ok(()) => tracing::warn!(
                        device = %path.display(),
                        "released implicitly acquired DRM master; start capture after the compositor"
                    ),
                    Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
                    Err(error) => {
                        return Err(CaptureError::Unavailable(format!(
                            "cannot release DRM master on {}: {error}",
                            path.display()
                        )));
                    }
                }
            }
            card.set_client_capability(ClientCapability::UniversalPlanes, true)
                .map_err(|error| {
                    CaptureError::Unavailable(format!(
                        "{} does not expose universal KMS planes: {error}",
                        path.display()
                    ))
                })?;
            // Atomic properties improve primary-plane identification. Some older
            // drivers do not support them, so the CRTC framebuffer remains the
            // fallback rather than making atomic support mandatory.
            let _ = card.set_client_capability(ClientCapability::Atomic, true);
            let egl_file = card.try_clone().map_err(|error| {
                CaptureError::Unavailable(format!(
                    "cannot duplicate {} for EGL: {error}",
                    path.display()
                ))
            })?;
            let egl = EglReader::new(egl_file.0.into()).map_err(|error| {
                CaptureError::Unavailable(format!(
                    "cannot create the EGL DMA-BUF reader for {}: {error}",
                    path.display()
                ))
            })?;
            if caps::has_cap(
                None,
                caps::CapSet::Effective,
                caps::Capability::CAP_SYS_ADMIN,
            )
            .unwrap_or(false)
            {
                caps::drop(
                    None,
                    caps::CapSet::Effective,
                    caps::Capability::CAP_SYS_ADMIN,
                )
                .map_err(|error| {
                    CaptureError::Unavailable(format!(
                        "cannot drop effective CAP_SYS_ADMIN after opening {}: {error}",
                        path.display()
                    ))
                })?;
            }
            Ok(Self { card, egl })
        }

        pub(super) fn capture(
            &mut self,
            requested_connector: Option<&str>,
            max_output_height: u32,
        ) -> Result<Frame, String> {
            // Re-enumerating here is intentional. DRM object and framebuffer IDs
            // are not stable across modesets or compositor replacement.
            let active = self.active_framebuffer(requested_connector)?;
            let descriptor = self.framebuffer_descriptor(active.handle)?;
            let file_descriptors = self.export_buffers(&descriptor)?;
            let (width, height) = descriptor.size;

            let modifier = (descriptor.modifier != DrmModifier::Invalid)
                .then(|| u64::from(descriptor.modifier));
            let planes = file_descriptors
                .iter()
                .enumerate()
                .take(descriptor.plane_count)
                .map(|(index, fd)| {
                    fd.as_ref()
                        .map(|fd| DmaBufPlane {
                            fd: fd.as_fd(),
                            offset: descriptor.offsets[index],
                            pitch: descriptor.pitches[index],
                            modifier,
                        })
                        .ok_or_else(|| format!("framebuffer plane {index} has no DMA-BUF"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let output = self
                .egl
                .read_rgb(
                    &DmaBufFrame {
                        width,
                        height,
                        fourcc: descriptor.format as u32,
                        planes: &planes,
                    },
                    ReadbackOptions {
                        max_height: max_output_height,
                        transfer_function: active.transfer_function,
                    },
                )
                .map_err(|error| {
                    format!(
                        "EGL could not read the {:?} framebuffer for {}: {error}",
                        descriptor.modifier, active.connector
                    )
                })?;

            Frame::new(output.width, output.height, output.pixels)
                .ok_or_else(|| "DRM conversion produced an invalid RGB frame".to_owned())
        }

        fn active_framebuffer(
            &self,
            requested_connector: Option<&str>,
        ) -> Result<ActiveFramebuffer, String> {
            let resources = self
                .card
                .resource_handles()
                .map_err(|error| format!("cannot enumerate DRM resources: {error}"))?;
            let mut connected = Vec::new();

            for connector_handle in resources.connectors() {
                let info = self
                    .card
                    .get_connector(*connector_handle, false)
                    .map_err(|error| {
                        format!("cannot inspect DRM connector {connector_handle:?}: {error}")
                    })?;
                if info.state() != connector::State::Connected {
                    continue;
                }
                let name = info.to_string();
                connected.push(name.clone());
                if requested_connector.is_some_and(|requested| requested != name) {
                    continue;
                }
                let Some(encoder_handle) = info.current_encoder() else {
                    continue;
                };
                let encoder = self
                    .card
                    .get_encoder(encoder_handle)
                    .map_err(|error| format!("cannot inspect encoder for {name}: {error}"))?;
                let Some(crtc_handle) = encoder.crtc() else {
                    continue;
                };
                if let Some(handle) = self.primary_framebuffer(crtc_handle)? {
                    let transfer_function = self.connector_transfer_function(*connector_handle)?;
                    return Ok(ActiveFramebuffer {
                        connector: name,
                        handle,
                        transfer_function,
                    });
                }
            }

            match requested_connector {
                Some(requested) => Err(format!(
                    "connector {requested} is not active; connected connectors: {}",
                    display_list(&connected)
                )),
                None => Err(format!(
                    "no connected connector has an active primary plane; connected connectors: {}",
                    display_list(&connected)
                )),
            }
        }

        fn primary_framebuffer(
            &self,
            crtc_handle: crtc::Handle,
        ) -> Result<Option<framebuffer::Handle>, String> {
            // The framebuffer is kept in the compositor's logical orientation.
            // KMS `rotation` commonly describes how a portrait-native handheld
            // panel is mounted; applying it again would rotate Gamescope's
            // already-landscape content. Source crop/scaling properties are not
            // applied yet. Supporting unusual primary-plane crops should carry
            // a normalized source rectangle into ReadbackOptions and transform
            // the shader UVs, without adding a CPU copy here.
            let crtc_framebuffer = self
                .card
                .get_crtc(crtc_handle)
                .map_err(|error| format!("cannot inspect CRTC {crtc_handle:?}: {error}"))?
                .framebuffer();
            let planes = self
                .card
                .plane_handles()
                .map_err(|error| format!("cannot enumerate DRM planes: {error}"))?;
            let mut legacy_match = None;

            for handle in planes {
                let info = self
                    .card
                    .get_plane(handle)
                    .map_err(|error| format!("cannot inspect DRM plane {handle:?}: {error}"))?;
                if info.crtc() != Some(crtc_handle) {
                    continue;
                }
                let Some(framebuffer) = info.framebuffer() else {
                    continue;
                };
                if self.plane_type(handle)? == Some(PlaneType::Primary as u64) {
                    return Ok(Some(framebuffer));
                }
                if crtc_framebuffer == Some(framebuffer) {
                    legacy_match = Some(framebuffer);
                }
            }
            Ok(legacy_match)
        }

        fn connector_transfer_function(
            &self,
            handle: connector::Handle,
        ) -> Result<TransferFunction, String> {
            let properties = self.card.get_properties(handle).map_err(|error| {
                format!("cannot inspect HDR properties for connector {handle:?}: {error}")
            })?;
            for (property_handle, raw_value) in properties.iter() {
                let property = self.card.get_property(*property_handle).map_err(|error| {
                    format!("cannot inspect connector property {property_handle:?}: {error}")
                })?;
                if property.name().to_bytes() != b"HDR_OUTPUT_METADATA" || *raw_value == 0 {
                    continue;
                }
                let blob = self.card.get_property_blob(*raw_value).map_err(|error| {
                    format!("cannot read HDR_OUTPUT_METADATA blob {raw_value}: {error}")
                })?;
                return parse_hdr_transfer_function(&blob);
            }
            Ok(TransferFunction::Srgb)
        }

        fn plane_type(&self, handle: plane::Handle) -> Result<Option<u64>, String> {
            let properties = self
                .card
                .get_properties(handle)
                .map_err(|error| format!("cannot inspect properties for {handle:?}: {error}"))?;
            for (property_handle, value) in properties.iter() {
                let property = self.card.get_property(*property_handle).map_err(|error| {
                    format!("cannot inspect plane property {property_handle:?}: {error}")
                })?;
                if property.name().to_bytes() == b"type" {
                    return Ok(Some(*value));
                }
            }
            Ok(None)
        }

        fn framebuffer_descriptor(
            &self,
            handle: framebuffer::Handle,
        ) -> Result<FramebufferDescriptor, String> {
            // GETFB/GETFB2 only returns GEM handles to CAP_SYS_ADMIN. Keep the
            // capability effective for this narrow ioctl window, as Sunshine
            // does, rather than retaining it throughout capture and transport.
            let _admin = EffectiveAdmin::raise()?;
            match self.card.get_planar_framebuffer(handle) {
                Ok(info) => {
                    let buffers = info.buffers();
                    let plane_count = buffers
                        .iter()
                        .rposition(Option::is_some)
                        .map_or(0, |index| index + 1);
                    if plane_count == 0 {
                        return Err(format!(
                            "DRM framebuffer {handle:?} has no GEM handles; CAP_SYS_ADMIN is usually required"
                        ));
                    }
                    Ok(FramebufferDescriptor {
                        size: info.size(),
                        format: info.pixel_format(),
                        buffers,
                        pitches: info.pitches(),
                        offsets: info.offsets(),
                        modifier: info.modifier().unwrap_or(DrmModifier::Invalid),
                        plane_count,
                    })
                }
                Err(planar_error) => {
                    let info = self.card.get_framebuffer(handle).map_err(|legacy_error| {
                        format!(
                            "cannot inspect DRM framebuffer {handle:?} (GETFB2: {planar_error}; GETFB: {legacy_error})"
                        )
                    })?;
                    let buffer = info.buffer().ok_or_else(|| {
                        format!(
                            "DRM framebuffer {handle:?} has no GEM handle; CAP_SYS_ADMIN is usually required"
                        )
                    })?;
                    Ok(FramebufferDescriptor {
                        size: info.size(),
                        format: DrmFourcc::Xrgb8888,
                        buffers: [Some(buffer), None, None, None],
                        pitches: [info.pitch(), 0, 0, 0],
                        offsets: [0; 4],
                        modifier: DrmModifier::Invalid,
                        plane_count: 1,
                    })
                }
            }
        }

        fn export_buffers(
            &self,
            descriptor: &FramebufferDescriptor,
        ) -> Result<[Option<OwnedFd>; 4], String> {
            let mut exported: [Option<OwnedFd>; 4] = std::array::from_fn(|_| None);
            for (index, handle) in descriptor.buffers.iter().enumerate() {
                if let Some(handle) = handle {
                    match self.card.buffer_to_prime_fd(*handle, 0) {
                        Ok(fd) => exported[index] = Some(fd),
                        Err(error) => {
                            self.close_gem_handles(descriptor);
                            return Err(format!(
                                "cannot export framebuffer plane {index} as DMA-BUF: {error}"
                            ));
                        }
                    }
                }
            }
            self.close_gem_handles(descriptor);
            Ok(exported)
        }

        fn close_gem_handles(&self, descriptor: &FramebufferDescriptor) {
            let mut closed = Vec::with_capacity(descriptor.plane_count);
            for handle in descriptor.buffers.iter().flatten() {
                if !closed.contains(handle) {
                    // The exported DMA-BUF FD owns its reference. GETFB/GETFB2's
                    // temporary GEM handles must be closed once per unique ID.
                    let _ = self.card.close_buffer(*handle);
                    closed.push(*handle);
                }
            }
        }
    }

    fn display_list(connectors: &[String]) -> String {
        if connectors.is_empty() {
            "none".to_owned()
        } else {
            connectors.join(", ")
        }
    }

    fn parse_hdr_transfer_function(blob: &[u8]) -> Result<TransferFunction, String> {
        // Linux's `hdr_output_metadata` starts with a native u32 metadata type,
        // followed by CTA-861 Static Metadata Type 1's EOTF and metadata-type
        // bytes. KMS UAPI structures use the host's native endianness.
        let header = blob.get(..6).ok_or_else(|| {
            format!(
                "HDR_OUTPUT_METADATA blob is too small: {} bytes",
                blob.len()
            )
        })?;
        let metadata_type = u32::from_ne_bytes(
            header[..4]
                .try_into()
                .map_err(|_| "HDR metadata type is truncated".to_owned())?,
        );
        if metadata_type != 0 {
            return Err(format!(
                "unsupported HDR_OUTPUT_METADATA type {metadata_type}; only CTA-861 Static Metadata Type 1 is supported"
            ));
        }
        if header[5] != 0 {
            return Err(format!(
                "unsupported CTA-861 HDR metadata type {}",
                header[5]
            ));
        }
        match header[4] {
            0 => Ok(TransferFunction::Srgb),
            2 => Ok(TransferFunction::Pq),
            3 => Ok(TransferFunction::Hlg),
            1 => Err(
                "traditional-gamma HDR scanout is not supported; PQ and HLG are supported"
                    .to_owned(),
            ),
            eotf => Err(format!(
                "unsupported CTA-861 HDR transfer function {eotf}; PQ and HLG are supported"
            )),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{TransferFunction, parse_hdr_transfer_function};

        #[test]
        fn parses_pq_and_hlg_hdr_metadata() -> Result<(), String> {
            assert_eq!(
                parse_hdr_transfer_function(&[0, 0, 0, 0, 2, 0])?,
                TransferFunction::Pq
            );
            assert_eq!(
                parse_hdr_transfer_function(&[0, 0, 0, 0, 3, 0])?,
                TransferFunction::Hlg
            );
            Ok(())
        }

        #[test]
        fn rejects_unknown_hdr_transfer_function() {
            let result = parse_hdr_transfer_function(&[0, 0, 0, 0, 9, 0]);
            assert!(result.is_err_and(|error| error.contains("transfer function 9")));
        }
    }
}
