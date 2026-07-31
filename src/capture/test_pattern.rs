use super::{CaptureError, CaptureSource, Frame};

#[derive(Debug)]
pub struct TestPatternCapture {
    width: u32,
    height: u32,
    frame_number: u8,
}

impl TestPatternCapture {
    pub const fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            frame_number: 0,
        }
    }
}

impl CaptureSource for TestPatternCapture {
    fn capture(&mut self) -> Result<Frame, CaptureError> {
        let capacity = usize::try_from(self.width)
            .ok()
            .and_then(|width| {
                usize::try_from(self.height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(3))
            .ok_or_else(|| {
                CaptureError::Unavailable("test frame dimensions overflow".to_owned())
            })?;
        let mut pixels = Vec::with_capacity(capacity);

        for y in 0..self.height {
            for x in 0..self.width {
                let red =
                    u8::try_from((u64::from(x) * 255) / u64::from(self.width)).unwrap_or(u8::MAX);
                let green =
                    u8::try_from((u64::from(y) * 255) / u64::from(self.height)).unwrap_or(u8::MAX);
                pixels.extend_from_slice(&[red.wrapping_add(self.frame_number), green, 96]);
            }
        }
        self.frame_number = self.frame_number.wrapping_add(1);

        Frame::new(self.width, self.height, pixels)
            .ok_or_else(|| CaptureError::Unavailable("generated an invalid test frame".to_owned()))
    }
}
