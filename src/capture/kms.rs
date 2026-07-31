use std::path::PathBuf;

use super::{CaptureError, CaptureSource, Frame};

/// Direct DRM/KMS capture backend.
///
/// This type establishes the compositor-independent boundary used by Sunshine.
/// Importing DRM framebuffers and converting DMA-BUF content to RGB is the next
/// implementation milestone; keeping it here prevents desktop-specific capture
/// details from leaking into the server pipeline.
#[derive(Debug)]
pub struct KmsCapture {
    device: PathBuf,
}

impl KmsCapture {
    pub fn open(device: impl Into<PathBuf>) -> Result<Self, CaptureError> {
        let device = device.into();

        #[cfg(not(target_os = "linux"))]
        return Err(CaptureError::Unavailable(format!(
            "DRM/KMS capture only runs on Linux (requested {})",
            device.display()
        )));

        #[cfg(target_os = "linux")]
        {
            if !device.exists() {
                return Err(CaptureError::Unavailable(format!(
                    "DRM device {} does not exist",
                    device.display()
                )));
            }
            Ok(Self { device })
        }
    }
}

impl CaptureSource for KmsCapture {
    fn capture(&mut self) -> Result<Frame, CaptureError> {
        Err(CaptureError::Unavailable(format!(
            "DRM framebuffer import for {} is not implemented yet; use --capture test-pattern to exercise the Hyperion pipeline",
            self.device.display()
        )))
    }
}
