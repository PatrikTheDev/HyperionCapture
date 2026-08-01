//! Isolated EGL/OpenGL ES DMA-BUF import, scaling, and readback.
//!
//! This crate contains the workspace's reviewed unsafe graphics boundary. The
//! public interface is safe and keeps native handles, borrowed DMA-BUFs, and
//! output allocations valid for every FFI call.

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::{
    DmaBufFrame, DmaBufPlane, Error, ReadbackOptions, Reader, RgbFrame, TransferFunction,
};
