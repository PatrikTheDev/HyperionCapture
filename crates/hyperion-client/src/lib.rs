//! Typed clients for sending captured frames to Hyperion.
//!
//! [`FlatbufferClient`] is intended for continuous raw RGB video. The existing
//! [`HyperionClient`] sends independent JPEG images through the HTTP JSON API,
//! which Hyperion limits to 25 updates per second.

mod flatbuffer;
mod http;

pub use flatbuffer::{
    DEFAULT_FLATBUFFER_PORT, Error as FlatbufferError, FlatbufferClient, FlatbufferConfig,
    MAX_FLATBUFFER_PRIORITY, MIN_FLATBUFFER_PRIORITY,
};
pub use http::{ClientConfig, DEFAULT_PRIORITY, Error, HyperionClient, MAX_PRIORITY, MIN_PRIORITY};
