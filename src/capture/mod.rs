mod frame;
mod kms;
mod test_pattern;

pub use frame::Frame;
pub use kms::{KmsCapture, KmsCaptureOptions};
pub use test_pattern::TestPatternCapture;

/// A source of packed RGB8 frames.
pub trait CaptureSource {
    /// Captures the next available frame.
    fn capture(&mut self) -> Result<Frame, CaptureError>;
}

/// Capture backend errors.
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("capture backend is unavailable: {0}")]
    Unavailable(String),
}
