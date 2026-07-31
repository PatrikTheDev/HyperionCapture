use std::{
    error::Error as StdError,
    ffi::{CStr, c_char, c_int, c_uint, c_void},
    fmt,
    marker::PhantomData,
    os::fd::{AsRawFd, BorrowedFd, OwnedFd},
    ptr,
    rc::Rc,
};

type EglBoolean = c_uint;
type EglDisplay = *mut c_void;
type EglConfig = *mut c_void;
type EglContext = *mut c_void;
type EglSurface = *mut c_void;
type EglImage = *mut c_void;
type EglClientBuffer = *mut c_void;
type EglInt = c_int;
type GbmDevice = c_void;

const EGL_FALSE: EglBoolean = 0;
const EGL_NONE: EglInt = 0x3038;
const EGL_EXTENSIONS: EglInt = 0x3055;
const EGL_SURFACE_TYPE: EglInt = 0x3033;
const EGL_PBUFFER_BIT: EglInt = 0x0001;
const EGL_RENDERABLE_TYPE: EglInt = 0x3040;
const EGL_OPENGL_BIT: EglInt = 0x0008;
const EGL_WIDTH: EglInt = 0x3057;
const EGL_HEIGHT: EglInt = 0x3056;
const EGL_OPENGL_API: c_uint = 0x30A2;
const EGL_LINUX_DMA_BUF_EXT: EglInt = 0x3270;
const EGL_LINUX_DRM_FOURCC_EXT: EglInt = 0x3271;
const EGL_DMA_BUF_PLANE0_FD_EXT: EglInt = 0x3272;
const EGL_DMA_BUF_PLANE0_OFFSET_EXT: EglInt = 0x3273;
const EGL_DMA_BUF_PLANE0_PITCH_EXT: EglInt = 0x3274;
const EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT: EglInt = 0x3443;
const EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT: EglInt = 0x3444;

const GL_TEXTURE_2D: c_uint = 0x0DE1;
const GL_TEXTURE_MIN_FILTER: c_uint = 0x2801;
const GL_TEXTURE_MAG_FILTER: c_uint = 0x2800;
const GL_NEAREST: c_int = 0x2600;
const GL_RGBA: c_uint = 0x1908;
const GL_UNSIGNED_BYTE: c_uint = 0x1401;
const GL_NO_ERROR: c_uint = 0;

type CreateImage = unsafe extern "C" fn(
    EglDisplay,
    EglContext,
    EglInt,
    EglClientBuffer,
    *const EglInt,
) -> EglImage;
type DestroyImage = unsafe extern "C" fn(EglDisplay, EglImage) -> EglBoolean;
type ImageTargetTexture = unsafe extern "C" fn(c_uint, *const c_void);

#[link(name = "gbm")]
unsafe extern "C" {
    fn gbm_create_device(fd: c_int) -> *mut GbmDevice;
    fn gbm_device_destroy(device: *mut GbmDevice);
}

#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetDisplay(native_display: *mut c_void) -> EglDisplay;
    fn eglInitialize(display: EglDisplay, major: *mut EglInt, minor: *mut EglInt) -> EglBoolean;
    fn eglTerminate(display: EglDisplay) -> EglBoolean;
    fn eglBindAPI(api: c_uint) -> EglBoolean;
    fn eglChooseConfig(
        display: EglDisplay,
        attributes: *const EglInt,
        configs: *mut EglConfig,
        config_size: EglInt,
        count: *mut EglInt,
    ) -> EglBoolean;
    fn eglCreatePbufferSurface(
        display: EglDisplay,
        config: EglConfig,
        attributes: *const EglInt,
    ) -> EglSurface;
    fn eglDestroySurface(display: EglDisplay, surface: EglSurface) -> EglBoolean;
    fn eglCreateContext(
        display: EglDisplay,
        config: EglConfig,
        share: EglContext,
        attributes: *const EglInt,
    ) -> EglContext;
    fn eglDestroyContext(display: EglDisplay, context: EglContext) -> EglBoolean;
    fn eglMakeCurrent(
        display: EglDisplay,
        draw: EglSurface,
        read: EglSurface,
        context: EglContext,
    ) -> EglBoolean;
    fn eglQueryString(display: EglDisplay, name: EglInt) -> *const c_char;
    fn eglGetProcAddress(name: *const c_char) -> *const c_void;
    fn eglGetError() -> EglInt;
}

#[link(name = "GL")]
unsafe extern "C" {
    fn glGenTextures(count: c_int, textures: *mut c_uint);
    fn glDeleteTextures(count: c_int, textures: *const c_uint);
    fn glBindTexture(target: c_uint, texture: c_uint);
    fn glTexParameteri(target: c_uint, name: c_uint, value: c_int);
    fn glGetTexImage(
        target: c_uint,
        level: c_int,
        format: c_uint,
        kind: c_uint,
        pixels: *mut c_void,
    );
    fn glGetError() -> c_uint;
}

/// One DMA-BUF plane belonging to a DRM framebuffer.
#[derive(Debug)]
pub struct DmaBufPlane<'a> {
    /// Live DMA-BUF file descriptor.
    pub fd: BorrowedFd<'a>,
    /// Byte offset of this plane in the DMA-BUF.
    pub offset: u32,
    /// Number of bytes between adjacent rows.
    pub pitch: u32,
    /// DRM format modifier, or `None` when the kernel did not report one.
    pub modifier: Option<u64>,
}

/// DMA-BUF metadata required by `EGL_EXT_image_dma_buf_import`.
#[derive(Debug)]
pub struct DmaBufFrame<'a> {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// DRM `FourCC` value.
    pub fourcc: u32,
    /// One to four DMA-BUF planes.
    pub planes: &'a [DmaBufPlane<'a>],
}

/// EGL/OpenGL initialization or readback failure.
#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl StdError for Error {}

/// Thread-affine EGL/OpenGL reader associated with one DRM device.
pub struct Reader {
    _drm_fd: OwnedFd,
    gbm: *mut GbmDevice,
    display: EglDisplay,
    context: EglContext,
    surface: EglSurface,
    create_image: CreateImage,
    destroy_image: DestroyImage,
    image_target_texture: ImageTargetTexture,
    supports_modifiers: bool,
    _thread_affine: PhantomData<Rc<()>>,
}

impl fmt::Debug for Reader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Reader")
            .field("display", &self.display)
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl Reader {
    /// Creates GBM, EGL, and desktop OpenGL state for a duplicated DRM FD.
    ///
    /// # Errors
    ///
    /// Returns an error when GBM/EGL initialization or a required DMA-BUF
    /// import extension is unavailable.
    pub fn new(drm_fd: OwnedFd) -> Result<Self, Error> {
        // SAFETY: `drm_fd` remains owned by the returned Reader. Each handle is
        // checked before use and partial initialization is unwound explicitly.
        unsafe {
            let gbm = gbm_create_device(drm_fd.as_raw_fd());
            if gbm.is_null() {
                return Err(Error("gbm_create_device failed".to_owned()));
            }
            let display = eglGetDisplay(gbm.cast());
            if display.is_null() {
                gbm_device_destroy(gbm);
                return Err(last_egl_error("eglGetDisplay failed"));
            }
            if eglInitialize(display, ptr::null_mut(), ptr::null_mut()) == EGL_FALSE {
                gbm_device_destroy(gbm);
                return Err(last_egl_error("eglInitialize failed"));
            }
            if eglBindAPI(EGL_OPENGL_API) == EGL_FALSE {
                eglTerminate(display);
                gbm_device_destroy(gbm);
                return Err(last_egl_error("EGL does not expose desktop OpenGL"));
            }

            let (context, surface) = match create_context(display) {
                Ok(state) => state,
                Err(error) => {
                    eglTerminate(display);
                    gbm_device_destroy(gbm);
                    return Err(error);
                }
            };

            let extensions = required_extensions(display);
            let (create_image, destroy_image, image_target_texture, supports_modifiers) =
                match extensions {
                    Ok(extensions) => extensions,
                    Err(error) => {
                        eglMakeCurrent(display, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
                        eglDestroyContext(display, context);
                        eglDestroySurface(display, surface);
                        eglTerminate(display);
                        gbm_device_destroy(gbm);
                        return Err(error);
                    }
                };

            Ok(Self {
                _drm_fd: drm_fd,
                gbm,
                display,
                context,
                surface,
                create_image,
                destroy_image,
                image_target_texture,
                supports_modifiers,
                _thread_affine: PhantomData,
            })
        }
    }

    /// Imports a framebuffer and reads normalized packed RGB8 pixels.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata, unsupported modifiers, EGL
    /// import failures, or OpenGL readback failures.
    pub fn read_rgb(&self, frame: &DmaBufFrame<'_>) -> Result<Vec<u8>, Error> {
        let attributes = build_attributes(frame, self.supports_modifiers)?;

        // SAFETY: the context belongs to this thread-affine Reader; the
        // terminated attribute array and all plane FDs remain live until the
        // EGL image is destroyed. The RGBA allocation is overflow-checked.
        unsafe {
            if eglMakeCurrent(self.display, self.surface, self.surface, self.context) == EGL_FALSE {
                return Err(last_egl_error("eglMakeCurrent failed before readback"));
            }
            let image = (self.create_image)(
                self.display,
                ptr::null_mut(),
                EGL_LINUX_DMA_BUF_EXT,
                ptr::null_mut(),
                attributes.as_ptr(),
            );
            if image.is_null() {
                return Err(last_egl_error("EGL DMA-BUF import failed"));
            }

            let mut texture = 0;
            glGenTextures(1, &mut texture);
            if texture == 0 {
                (self.destroy_image)(self.display, image);
                return Err(last_gl_error("glGenTextures failed"));
            }
            glBindTexture(GL_TEXTURE_2D, texture);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
            (self.image_target_texture)(GL_TEXTURE_2D, image.cast_const());
            let image_target_error = glGetError();
            if image_target_error != GL_NO_ERROR {
                glDeleteTextures(1, &texture);
                (self.destroy_image)(self.display, image);
                return Err(Error(format!(
                    "glEGLImageTargetTexture2DOES failed (OpenGL error 0x{image_target_error:04x})"
                )));
            }

            let pixel_count = usize::try_from(frame.width)
                .ok()
                .and_then(|width| {
                    usize::try_from(frame.height)
                        .ok()
                        .and_then(|height| width.checked_mul(height))
                })
                .ok_or_else(|| Error("frame dimensions overflow addressable memory".to_owned()))?;
            let rgba_len = pixel_count
                .checked_mul(4)
                .ok_or_else(|| Error("RGBA frame size overflow".to_owned()))?;
            let mut rgba = vec![0_u8; rgba_len];
            glGetTexImage(
                GL_TEXTURE_2D,
                0,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                rgba.as_mut_ptr().cast(),
            );
            let gl_error = glGetError();
            glBindTexture(GL_TEXTURE_2D, 0);
            glDeleteTextures(1, &texture);
            (self.destroy_image)(self.display, image);
            if gl_error != GL_NO_ERROR {
                return Err(Error(format!(
                    "OpenGL texture readback failed: 0x{gl_error:04x}"
                )));
            }

            let output_len = pixel_count
                .checked_mul(3)
                .ok_or_else(|| Error("RGB frame size overflow".to_owned()))?;
            let mut rgb = Vec::with_capacity(output_len);
            for pixel in rgba.chunks_exact(4) {
                rgb.extend_from_slice(&pixel[..3]);
            }
            Ok(rgb)
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        // SAFETY: all handles were created by this Reader and are destroyed
        // once, in reverse dependency order, while the DRM FD is still owned.
        unsafe {
            let _ = eglMakeCurrent(
                self.display,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let _ = eglDestroyContext(self.display, self.context);
            let _ = eglDestroySurface(self.display, self.surface);
            let _ = eglTerminate(self.display);
            gbm_device_destroy(self.gbm);
        }
    }
}

fn to_egl_int(value: u32, label: &str) -> Result<EglInt, Error> {
    EglInt::try_from(value).map_err(|error| Error(format!("{label} is too large: {error}")))
}

unsafe fn create_context(display: EglDisplay) -> Result<(EglContext, EglSurface), Error> {
    // SAFETY: `display` is initialized; the helper releases every partial
    // surface/context before returning an error.
    let surfaceless = unsafe { has_egl_extension(display, b"EGL_KHR_surfaceless_context") }?;
    let config_attributes = if surfaceless {
        [
            EGL_RENDERABLE_TYPE,
            EGL_OPENGL_BIT,
            EGL_NONE,
            EGL_NONE,
            EGL_NONE,
        ]
    } else {
        [
            EGL_SURFACE_TYPE,
            EGL_PBUFFER_BIT,
            EGL_RENDERABLE_TYPE,
            EGL_OPENGL_BIT,
            EGL_NONE,
        ]
    };
    let mut config = ptr::null_mut();
    let mut config_count = 0;
    // SAFETY: output pointers and the terminated attribute array are valid.
    if unsafe {
        eglChooseConfig(
            display,
            config_attributes.as_ptr(),
            &mut config,
            1,
            &mut config_count,
        )
    } == EGL_FALSE
        || config_count != 1
    {
        return Err(last_egl_error(
            "no compatible EGL OpenGL config is available",
        ));
    }

    let surface = if surfaceless {
        ptr::null_mut()
    } else {
        let attributes = [EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE];
        // SAFETY: `config` was selected for pbuffer support.
        let surface = unsafe { eglCreatePbufferSurface(display, config, attributes.as_ptr()) };
        if surface.is_null() {
            return Err(last_egl_error("eglCreatePbufferSurface failed"));
        }
        surface
    };
    // SAFETY: the attribute array is terminated and `config` belongs to the
    // initialized display.
    let context =
        unsafe { eglCreateContext(display, config, ptr::null_mut(), [EGL_NONE].as_ptr()) };
    if context.is_null() {
        if !surface.is_null() {
            // SAFETY: the pbuffer was created above and has no dependants.
            let _ = unsafe { eglDestroySurface(display, surface) };
        }
        return Err(last_egl_error("eglCreateContext failed"));
    }
    // SAFETY: the context and optional surface belong to this display;
    // EGL_KHR_surfaceless_context permits null draw/read surfaces.
    if unsafe { eglMakeCurrent(display, surface, surface, context) } == EGL_FALSE {
        // SAFETY: these handles were created above and are released once.
        let _ = unsafe { eglDestroyContext(display, context) };
        if !surface.is_null() {
            // SAFETY: this is the live pbuffer created above.
            let _ = unsafe { eglDestroySurface(display, surface) };
        }
        return Err(last_egl_error("eglMakeCurrent failed"));
    }
    Ok((context, surface))
}

unsafe fn required_extensions(
    display: EglDisplay,
) -> Result<(CreateImage, DestroyImage, ImageTargetTexture, bool), Error> {
    // SAFETY: `display` is initialized and current for the calling Reader.
    let extensions = unsafe { eglQueryString(display, EGL_EXTENSIONS) };
    if extensions.is_null() {
        return Err(Error("EGL extension query failed".to_owned()));
    }
    // SAFETY: EGL guarantees a NUL-terminated string for a successful query;
    // its storage remains owned by EGL for the display lifetime.
    let extension_bytes = unsafe { CStr::from_ptr(extensions) }.to_bytes();
    let has_extension = |expected: &[u8]| {
        extension_bytes
            .split(|byte| *byte == b' ')
            .any(|extension| extension == expected)
    };
    if !has_extension(b"EGL_EXT_image_dma_buf_import") {
        return Err(Error(
            "EGL_EXT_image_dma_buf_import is not supported".to_owned(),
        ));
    }
    let supports_modifiers = has_extension(b"EGL_EXT_image_dma_buf_import_modifiers");
    // SAFETY: each symbol is paired with its published Khronos ABI type.
    let create_image = unsafe { load_proc::<CreateImage>(c"eglCreateImageKHR") }?;
    // SAFETY: each symbol is paired with its published Khronos ABI type.
    let destroy_image = unsafe { load_proc::<DestroyImage>(c"eglDestroyImageKHR") }?;
    // SAFETY: each symbol is paired with its published Khronos ABI type.
    let image_target = unsafe { load_proc::<ImageTargetTexture>(c"glEGLImageTargetTexture2DOES") }?;
    Ok((
        create_image,
        destroy_image,
        image_target,
        supports_modifiers,
    ))
}

unsafe fn has_egl_extension(display: EglDisplay, expected: &[u8]) -> Result<bool, Error> {
    // SAFETY: `display` was returned by EGL and initialized by the caller.
    let extensions = unsafe { eglQueryString(display, EGL_EXTENSIONS) };
    if extensions.is_null() {
        return Err(Error("EGL extension query failed".to_owned()));
    }
    // SAFETY: a successful EGL string query returns stable NUL-terminated
    // storage owned by the display.
    Ok(unsafe { CStr::from_ptr(extensions) }
        .to_bytes()
        .split(|byte| *byte == b' ')
        .any(|extension| extension == expected))
}

fn build_attributes(
    frame: &DmaBufFrame<'_>,
    supports_modifiers: bool,
) -> Result<Vec<EglInt>, Error> {
    if frame.width == 0 || frame.height == 0 {
        return Err(Error("DMA-BUF dimensions must be non-zero".to_owned()));
    }
    if frame.planes.is_empty() || frame.planes.len() > 4 {
        return Err(Error(
            "DMA-BUF must contain between one and four planes".to_owned(),
        ));
    }
    if !supports_modifiers && frame.planes.iter().any(|plane| plane.modifier.is_some()) {
        return Err(Error(
            "EGL_EXT_image_dma_buf_import_modifiers is not supported".to_owned(),
        ));
    }
    let mut attributes = vec![
        EGL_WIDTH,
        to_egl_int(frame.width, "frame width")?,
        EGL_HEIGHT,
        to_egl_int(frame.height, "frame height")?,
        EGL_LINUX_DRM_FOURCC_EXT,
        EglInt::from_ne_bytes(frame.fourcc.to_ne_bytes()),
    ];
    for (index, plane) in frame.planes.iter().enumerate() {
        let index = EglInt::try_from(index)
            .map_err(|error| Error(format!("invalid plane index: {error}")))?;
        attributes.extend_from_slice(&[
            EGL_DMA_BUF_PLANE0_FD_EXT + index * 3,
            plane.fd.as_raw_fd(),
            EGL_DMA_BUF_PLANE0_OFFSET_EXT + index * 3,
            to_egl_int(plane.offset, "plane offset")?,
            EGL_DMA_BUF_PLANE0_PITCH_EXT + index * 3,
            to_egl_int(plane.pitch, "plane pitch")?,
        ]);
        if let Some(modifier) = plane.modifier {
            let low = u32::try_from(modifier & u64::from(u32::MAX))
                .map_err(|error| Error(format!("invalid modifier low bits: {error}")))?;
            let high = u32::try_from(modifier >> 32)
                .map_err(|error| Error(format!("invalid modifier high bits: {error}")))?;
            attributes.extend_from_slice(&[
                EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT + index * 2,
                EglInt::from_ne_bytes(low.to_ne_bytes()),
                EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT + index * 2,
                EglInt::from_ne_bytes(high.to_ne_bytes()),
            ]);
        }
    }
    attributes.push(EGL_NONE);
    Ok(attributes)
}

unsafe fn load_proc<T: Copy>(name: &CStr) -> Result<T, Error> {
    // SAFETY: EGL returns a function pointer for the exact extension name. The
    // caller supplies the matching ABI type and checks for null first.
    let pointer = unsafe { eglGetProcAddress(name.as_ptr()) };
    if pointer.is_null() {
        return Err(Error(format!(
            "required EGL/GL symbol {} is unavailable",
            name.to_string_lossy()
        )));
    }
    // SAFETY: guarded by the ABI/name invariant documented above; function
    // pointers and data pointers have the same representation on supported
    // Linux EGL platforms.
    Ok(unsafe { std::mem::transmute_copy(&pointer) })
}

fn last_egl_error(message: &str) -> Error {
    // SAFETY: eglGetError has no arguments and is valid after any EGL failure.
    let code = unsafe { eglGetError() };
    Error(format!("{message} (EGL error 0x{code:04x})"))
}

fn last_gl_error(message: &str) -> Error {
    // SAFETY: callers have made this Reader's context current.
    let code = unsafe { glGetError() };
    Error(format!("{message} (OpenGL error 0x{code:04x})"))
}

#[cfg(test)]
mod tests {
    use std::{fs::File, os::fd::AsFd};

    use super::{
        DmaBufFrame, DmaBufPlane, EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT,
        EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT, EGL_NONE, build_attributes,
    };

    #[test]
    fn builds_terminated_modifier_attributes() -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open("/dev/null")?;
        let planes = [DmaBufPlane {
            fd: file.as_fd(),
            offset: 64,
            pitch: 4096,
            modifier: Some(0x0123_4567_89ab_cdef),
        }];
        let attributes = build_attributes(
            &DmaBufFrame {
                width: 1920,
                height: 1080,
                fourcc: 0x3432_5258,
                planes: &planes,
            },
            true,
        )?;

        assert_eq!(attributes.last(), Some(&EGL_NONE));
        assert!(attributes.contains(&EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT));
        assert!(attributes.contains(&EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT));
        Ok(())
    }

    #[test]
    fn rejects_modifier_without_egl_extension() -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open("/dev/null")?;
        let planes = [DmaBufPlane {
            fd: file.as_fd(),
            offset: 0,
            pitch: 4,
            modifier: Some(0),
        }];
        let result = build_attributes(
            &DmaBufFrame {
                width: 1,
                height: 1,
                fourcc: 0x3432_5258,
                planes: &planes,
            },
            false,
        );
        assert!(result.is_err_and(|error| error.to_string().contains("modifiers")));
        Ok(())
    }

    #[test]
    fn rejects_a_frame_without_planes() {
        let result = build_attributes(
            &DmaBufFrame {
                width: 1,
                height: 1,
                fourcc: 0x3432_5258,
                planes: &[],
            },
            true,
        );
        assert!(result.is_err_and(|error| error.to_string().contains("one and four")));
    }
}
