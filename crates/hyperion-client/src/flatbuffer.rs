//! Persistent Hyperion `FlatBuffers` image-stream transport.

use std::io;

use hyperion_flatbuffer::{Encoder, Reply, parse_reply};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

/// Default TCP port of Hyperion's `FlatBuffers` server.
pub const DEFAULT_FLATBUFFER_PORT: u16 = 19_400;
/// Lowest priority accepted by Hyperion's `FlatBuffers` server.
pub const MIN_FLATBUFFER_PRIORITY: u16 = 100;
/// Highest priority accepted by Hyperion's `FlatBuffers` server.
pub const MAX_FLATBUFFER_PRIORITY: u16 = 199;

const MAX_REPLY_SIZE: u32 = 64 * 1024;

/// Configuration for a [`FlatbufferClient`].
#[derive(Clone, Debug)]
pub struct FlatbufferConfig {
    /// Hyperion host name or IP address.
    pub host: String,
    /// Hyperion `FlatBuffers` TCP port.
    pub port: u16,
    /// Streaming priority. Hyperion reserves the range 100..=199.
    pub priority: u16,
    /// Origin shown in Hyperion's source list.
    pub origin: String,
}

impl FlatbufferConfig {
    /// Creates a configuration using Hyperion's standard `FlatBuffers` port and
    /// the crate's default streaming priority.
    #[must_use]
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port: DEFAULT_FLATBUFFER_PORT,
            priority: super::DEFAULT_PRIORITY,
            origin: "hyperion-capture".to_owned(),
        }
    }
}

/// A persistent asynchronous client for Hyperion's `FlatBuffers` server.
///
/// Raw packed RGB frames are copied once into the `FlatBuffer` message. This
/// protocol-boundary copy is required because `FlatBuffers` builds a contiguous,
/// owned TCP payload; unlike the HTTP client, no JPEG or Base64 conversion is
/// performed.
pub struct FlatbufferClient {
    config: FlatbufferConfig,
    stream: Option<TcpStream>,
    encoder: Encoder,
}

impl FlatbufferClient {
    /// Creates a disconnected client. The first frame establishes and
    /// registers the persistent TCP connection.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured priority is outside the range
    /// accepted by Hyperion's `FlatBuffers` server.
    pub fn new(config: FlatbufferConfig) -> Result<Self, Error> {
        if !(MIN_FLATBUFFER_PRIORITY..=MAX_FLATBUFFER_PRIORITY).contains(&config.priority) {
            return Err(Error::InvalidPriority(config.priority));
        }
        if config.host.is_empty() {
            return Err(Error::EmptyHost);
        }

        Ok(Self {
            config,
            stream: None,
            encoder: Encoder::new(),
        })
    }

    async fn connect(&mut self) -> Result<(), Error> {
        if self.stream.is_some() {
            return Ok(());
        }

        let mut stream = TcpStream::connect((self.config.host.as_str(), self.config.port))
            .await
            .map_err(|source| Error::Connect {
                host: self.config.host.clone(),
                port: self.config.port,
                source,
            })?;
        stream.set_nodelay(true)?;

        let request = self
            .encoder
            .register(&self.config.origin, i32::from(self.config.priority));
        let reply = decode_reply(&exchange(&mut stream, request).await?)?;
        let registered = reply.registered;
        if registered != i32::from(self.config.priority) {
            return Err(Error::InvalidReply(
                "registration reply did not contain the requested priority".to_owned(),
            ));
        }

        self.stream = Some(stream);
        Ok(())
    }

    /// Sends one packed, top-down RGB8 frame over the persistent connection.
    ///
    /// If a previous I/O failure disconnected the client, this method opens
    /// and registers a new connection before sending the frame.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid frame data or duration, connection and I/O
    /// failures, malformed replies, or a command rejected by Hyperion.
    pub async fn send_rgb8(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u8],
        duration_ms: u32,
    ) -> Result<(), Error> {
        let (width, height) = validate_rgb8(width, height, pixels)?;
        let duration =
            i32::try_from(duration_ms).map_err(|_| Error::InvalidDuration(duration_ms))?;

        self.connect().await?;
        let request = self.encoder.image(width, height, pixels, duration);

        let result = match self.stream.as_mut() {
            Some(stream) => exchange(stream, request).await,
            None => {
                return Err(Error::InvalidReply(
                    "connection disappeared after registration".to_owned(),
                ));
            }
        };

        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                self.stream = None;
                return Err(error);
            }
        };
        decode_reply(&reply).map(|_| ())
    }
}

fn validate_rgb8(width: u32, height: u32, pixels: &[u8]) -> Result<(i32, i32), Error> {
    let expected = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(3));

    if width == 0 || height == 0 || expected != Some(pixels.len()) {
        return Err(Error::InvalidRgbBuffer { width, height });
    }

    let protocol_width =
        i32::try_from(width).map_err(|_| Error::InvalidDimensions { width, height })?;
    let protocol_height =
        i32::try_from(height).map_err(|_| Error::InvalidDimensions { width, height })?;
    Ok((protocol_width, protocol_height))
}

async fn exchange(stream: &mut TcpStream, request: &[u8]) -> Result<Vec<u8>, Error> {
    let length = u32::try_from(request.len()).map_err(|_| Error::FrameTooLarge(request.len()))?;
    stream.write_all(&length.to_be_bytes()).await?;
    stream.write_all(request).await?;

    let mut header = [0_u8; 4];
    stream.read_exact(&mut header).await?;
    let reply_length = u32::from_be_bytes(header);
    if reply_length == 0 || reply_length > MAX_REPLY_SIZE {
        return Err(Error::ReplyTooLarge(reply_length));
    }

    let mut reply = vec![0; reply_length as usize];
    stream.read_exact(&mut reply).await?;
    Ok(reply)
}

fn decode_reply(buffer: &[u8]) -> Result<Reply, Error> {
    let reply = parse_reply(buffer).map_err(|error| Error::InvalidReply(error.to_string()))?;
    match reply.error {
        Some(error) => Err(Error::Rejected(error)),
        None => Ok(reply),
    }
}

/// Errors produced by the `FlatBuffers` streaming client.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No Hyperion host was configured.
    #[error("Hyperion FlatBuffers host cannot be empty")]
    EmptyHost,
    /// The priority is outside the range reserved for `FlatBuffers` streams.
    #[error(
        "Hyperion FlatBuffers priority must be between {MIN_FLATBUFFER_PRIORITY} and {MAX_FLATBUFFER_PRIORITY}, got {0}"
    )]
    InvalidPriority(u16),
    /// Frame dimensions cannot be represented by the protocol.
    #[error("RGB frame dimensions {width}x{height} exceed the FlatBuffers protocol limits")]
    InvalidDimensions {
        /// Declared image width.
        width: u32,
        /// Declared image height.
        height: u32,
    },
    /// The RGB buffer length does not match its dimensions.
    #[error("RGB buffer does not contain exactly {width}x{height} pixels")]
    InvalidRgbBuffer {
        /// Declared image width.
        width: u32,
        /// Declared image height.
        height: u32,
    },
    /// The frame duration cannot be represented by the protocol.
    #[error("frame duration {0}ms exceeds the FlatBuffers protocol limit")]
    InvalidDuration(u32),
    /// The serialized frame is too large for Hyperion's TCP framing.
    #[error("serialized FlatBuffers frame is too large: {0} bytes")]
    FrameTooLarge(usize),
    /// A TCP connection could not be established.
    #[error("failed to connect to Hyperion FlatBuffers server at {host}:{port}: {source}")]
    Connect {
        /// Configured server host.
        host: String,
        /// Configured server port.
        port: u16,
        /// Underlying socket error.
        source: io::Error,
    },
    /// An established TCP exchange failed.
    #[error("Hyperion FlatBuffers connection failed: {0}")]
    Io(#[from] io::Error),
    /// Hyperion advertised an invalid reply length.
    #[error("invalid Hyperion FlatBuffers reply length: {0}")]
    ReplyTooLarge(u32),
    /// Hyperion returned a malformed `FlatBuffer` reply.
    #[error("invalid Hyperion FlatBuffers reply: {0}")]
    InvalidReply(String),
    /// Hyperion rejected a `FlatBuffers` command.
    #[error("Hyperion rejected the FlatBuffers command: {0}")]
    Rejected(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_priority_outside_flatbuffer_range() {
        let mut config = FlatbufferConfig::new("127.0.0.1");
        config.priority = 99;
        assert!(matches!(
            FlatbufferClient::new(config),
            Err(Error::InvalidPriority(99))
        ));
    }

    #[test]
    fn rejects_mismatched_rgb_buffer() {
        assert!(matches!(
            validate_rgb8(2, 2, &[0; 11]),
            Err(Error::InvalidRgbBuffer {
                width: 2,
                height: 2
            })
        ));
    }
}
