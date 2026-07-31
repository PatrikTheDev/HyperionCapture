//! Persistent Hyperion `FlatBuffers` image-stream transport.

use std::{fmt, io, str};

use flatbuffers::{FlatBufferBuilder, TableFinishedWIPOffset, WIPOffset};
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

const COMMAND_IMAGE: u8 = 2;
const COMMAND_REGISTER: u8 = 4;
const IMAGE_TYPE_RAW: u8 = 1;

const FIELD_0: u16 = 4;
const FIELD_1: u16 = 6;
const FIELD_2: u16 = 8;

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
    builder: FlatBufferBuilder<'static>,
}

impl fmt::Debug for FlatbufferClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FlatbufferClient")
            .field("config", &self.config)
            .field("connected", &self.stream.is_some())
            .finish_non_exhaustive()
    }
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
            builder: FlatBufferBuilder::with_capacity(1024),
        })
    }

    /// Connects to Hyperion and registers this streaming source.
    ///
    /// Calling this method on an already-connected client is a no-op.
    ///
    /// # Errors
    ///
    /// Returns an error when the TCP connection, registration exchange, or
    /// Hyperion response fails.
    pub async fn connect(&mut self) -> Result<(), Error> {
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
        stream.set_nodelay(true).map_err(Error::Io)?;

        self.builder.reset();
        build_register_request(
            &mut self.builder,
            &self.config.origin,
            i32::from(self.config.priority),
        );
        let reply = exchange(&mut stream, self.builder.finished_data()).await?;
        let reply = parse_reply(&reply)?;
        let registered = reply.registered;
        reply.into_result()?;
        if registered != i32::from(self.config.priority) {
            return Err(Error::InvalidReply(
                "registration reply did not contain the requested priority",
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
        self.builder.reset();
        build_image_request(&mut self.builder, width, height, pixels, duration);

        let result = match self.stream.as_mut() {
            Some(stream) => exchange(stream, self.builder.finished_data()).await,
            None => {
                return Err(Error::InvalidReply(
                    "connection disappeared after registration",
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
        parse_reply(&reply)?.into_result()
    }

    /// Returns whether a registered TCP connection is currently held.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.stream.is_some()
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

fn build_register_request(builder: &mut FlatBufferBuilder<'_>, origin: &str, priority: i32) {
    let origin = builder.create_string(origin);
    let start = builder.start_table();
    builder.push_slot_always(FIELD_0, origin);
    builder.push_slot(FIELD_1, priority, 0);
    let register = builder.end_table(start);
    finish_request(builder, COMMAND_REGISTER, register);
}

fn build_image_request(
    builder: &mut FlatBufferBuilder<'_>,
    width: i32,
    height: i32,
    pixels: &[u8],
    duration: i32,
) {
    let data = builder.create_vector(pixels);
    let start = builder.start_table();
    builder.push_slot_always(FIELD_0, data);
    builder.push_slot(FIELD_1, width, -1);
    builder.push_slot(FIELD_2, height, -1);
    let raw_image = builder.end_table(start);

    let start = builder.start_table();
    builder.push_slot(FIELD_0, IMAGE_TYPE_RAW, 0);
    builder.push_slot_always(FIELD_1, raw_image.as_union_value());
    builder.push_slot(FIELD_2, duration, -1);
    let image = builder.end_table(start);

    finish_request(builder, COMMAND_IMAGE, image);
}

fn finish_request(
    builder: &mut FlatBufferBuilder<'_>,
    command_type: u8,
    command: WIPOffset<TableFinishedWIPOffset>,
) {
    let start = builder.start_table();
    builder.push_slot(FIELD_0, command_type, 0);
    builder.push_slot_always(FIELD_1, command.as_union_value());
    let request = builder.end_table(start);
    builder.finish_minimal(request);
}

async fn exchange(stream: &mut TcpStream, request: &[u8]) -> Result<Vec<u8>, Error> {
    let length = u32::try_from(request.len()).map_err(|_| Error::FrameTooLarge(request.len()))?;
    stream
        .write_all(&length.to_be_bytes())
        .await
        .map_err(Error::Io)?;
    stream.write_all(request).await.map_err(Error::Io)?;

    let mut header = [0_u8; 4];
    stream.read_exact(&mut header).await.map_err(Error::Io)?;
    let reply_length = u32::from_be_bytes(header);
    if reply_length == 0 || reply_length > MAX_REPLY_SIZE {
        return Err(Error::ReplyTooLarge(reply_length));
    }

    let mut reply = vec![0; reply_length as usize];
    stream.read_exact(&mut reply).await.map_err(Error::Io)?;
    Ok(reply)
}

#[derive(Debug)]
struct Reply {
    error: Option<String>,
    registered: i32,
}

impl Reply {
    fn into_result(self) -> Result<(), Error> {
        match self.error {
            Some(error) => Err(Error::Rejected(error)),
            None => Ok(()),
        }
    }
}

fn parse_reply(buffer: &[u8]) -> Result<Reply, Error> {
    let table = root_table(buffer)?;
    let error = match table_field(buffer, table, 0)? {
        Some(field) => Some(read_string(buffer, field)?),
        None => None,
    };
    let registered = match table_field(buffer, table, 2)? {
        Some(field) => read_i32(buffer, field)?,
        None => -1,
    };
    Ok(Reply { error, registered })
}

fn root_table(buffer: &[u8]) -> Result<usize, Error> {
    let offset = usize::try_from(read_u32(buffer, 0)?)
        .map_err(|_| Error::InvalidReply("root table offset overflows usize"))?;
    buffer
        .get(offset..)
        .ok_or(Error::InvalidReply("root table offset is out of bounds"))?;
    Ok(offset)
}

fn table_field(buffer: &[u8], table: usize, field_index: usize) -> Result<Option<usize>, Error> {
    let vtable_offset = usize::try_from(read_i32(buffer, table)?)
        .map_err(|_| Error::InvalidReply("negative vtable offset"))?;
    let vtable = table
        .checked_sub(vtable_offset)
        .ok_or(Error::InvalidReply("vtable offset is out of bounds"))?;
    let vtable_length = usize::from(read_u16(buffer, vtable)?);
    let entry = vtable
        .checked_add(4)
        .and_then(|entry| entry.checked_add(field_index.saturating_mul(2)))
        .ok_or(Error::InvalidReply("vtable field offset overflows"))?;
    if entry + 2 > vtable + vtable_length {
        return Ok(None);
    }
    let field_offset = usize::from(read_u16(buffer, entry)?);
    if field_offset == 0 {
        return Ok(None);
    }
    let field = table
        .checked_add(field_offset)
        .ok_or(Error::InvalidReply("table field offset overflows"))?;
    buffer
        .get(field..)
        .ok_or(Error::InvalidReply("table field is out of bounds"))?;
    Ok(Some(field))
}

fn read_string(buffer: &[u8], field: usize) -> Result<String, Error> {
    let relative = usize::try_from(read_u32(buffer, field)?)
        .map_err(|_| Error::InvalidReply("string offset overflows usize"))?;
    let string = field
        .checked_add(relative)
        .ok_or(Error::InvalidReply("string offset overflows"))?;
    let length = usize::try_from(read_u32(buffer, string)?)
        .map_err(|_| Error::InvalidReply("string length overflows usize"))?;
    let start = string
        .checked_add(4)
        .ok_or(Error::InvalidReply("string data offset overflows"))?;
    let end = start
        .checked_add(length)
        .ok_or(Error::InvalidReply("string length overflows"))?;
    let bytes = buffer
        .get(start..end)
        .ok_or(Error::InvalidReply("string is out of bounds"))?;
    str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| Error::InvalidReply("error string is not UTF-8"))
}

fn read_u16(buffer: &[u8], offset: usize) -> Result<u16, Error> {
    let bytes = buffer
        .get(offset..offset.saturating_add(2))
        .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
        .ok_or(Error::InvalidReply("truncated 16-bit value"))?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(buffer: &[u8], offset: usize) -> Result<u32, Error> {
    let bytes = buffer
        .get(offset..offset.saturating_add(4))
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .ok_or(Error::InvalidReply("truncated 32-bit value"))?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_i32(buffer: &[u8], offset: usize) -> Result<i32, Error> {
    let bytes = buffer
        .get(offset..offset.saturating_add(4))
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .ok_or(Error::InvalidReply("truncated signed 32-bit value"))?;
    Ok(i32::from_le_bytes(bytes))
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
    Io(io::Error),
    /// Hyperion advertised an invalid reply length.
    #[error("invalid Hyperion FlatBuffers reply length: {0}")]
    ReplyTooLarge(u32),
    /// Hyperion returned a malformed `FlatBuffer` reply.
    #[error("invalid Hyperion FlatBuffers reply: {0}")]
    InvalidReply(&'static str),
    /// Hyperion rejected a `FlatBuffers` command.
    #[error("Hyperion rejected the FlatBuffers command: {0}")]
    Rejected(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_registration_request() {
        let mut builder = FlatBufferBuilder::new();
        build_register_request(&mut builder, "capture-test", 150);
        let buffer = builder.finished_data();

        let request = root_table(buffer).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(read_table_u8(buffer, request, 0), COMMAND_REGISTER);
        let register = read_table(buffer, request, 1);
        assert_eq!(read_table_string(buffer, register, 0), "capture-test");
        assert_eq!(read_table_i32(buffer, register, 1), 150);
    }

    #[test]
    fn serializes_raw_rgb_image_request() {
        let pixels = [255, 0, 0, 0, 255, 0];
        let mut builder = FlatBufferBuilder::new();
        build_image_request(&mut builder, 2, 1, &pixels, 250);
        let buffer = builder.finished_data();

        let request = root_table(buffer).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(read_table_u8(buffer, request, 0), COMMAND_IMAGE);
        let image = read_table(buffer, request, 1);
        assert_eq!(read_table_u8(buffer, image, 0), IMAGE_TYPE_RAW);
        assert_eq!(read_table_i32(buffer, image, 2), 250);
        let raw = read_table(buffer, image, 1);
        assert_eq!(read_table_vector(buffer, raw, 0), pixels);
        assert_eq!(read_table_i32(buffer, raw, 1), 2);
        assert_eq!(read_table_i32(buffer, raw, 2), 1);
    }

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

    fn read_table(buffer: &[u8], table: usize, field: usize) -> usize {
        let field = table_field(buffer, table, field)
            .unwrap_or_else(|error| panic!("{error}"))
            .unwrap_or_else(|| panic!("missing table field {field}"));
        field
            + usize::try_from(read_u32(buffer, field).unwrap_or_else(|error| panic!("{error}")))
                .unwrap_or_else(|error| panic!("{error}"))
    }

    fn read_table_u8(buffer: &[u8], table: usize, field: usize) -> u8 {
        let position = table_field(buffer, table, field)
            .unwrap_or_else(|error| panic!("{error}"))
            .unwrap_or_else(|| panic!("missing byte field {field}"));
        buffer[position]
    }

    fn read_table_i32(buffer: &[u8], table: usize, field: usize) -> i32 {
        let position = table_field(buffer, table, field)
            .unwrap_or_else(|error| panic!("{error}"))
            .unwrap_or_else(|| panic!("missing integer field {field}"));
        read_i32(buffer, position).unwrap_or_else(|error| panic!("{error}"))
    }

    fn read_table_string(buffer: &[u8], table: usize, field: usize) -> String {
        let position = table_field(buffer, table, field)
            .unwrap_or_else(|error| panic!("{error}"))
            .unwrap_or_else(|| panic!("missing string field {field}"));
        read_string(buffer, position).unwrap_or_else(|error| panic!("{error}"))
    }

    fn read_table_vector(buffer: &[u8], table: usize, field: usize) -> &[u8] {
        let field = table_field(buffer, table, field)
            .unwrap_or_else(|error| panic!("{error}"))
            .unwrap_or_else(|| panic!("missing vector field {field}"));
        let vector = field
            + usize::try_from(read_u32(buffer, field).unwrap_or_else(|error| panic!("{error}")))
                .unwrap_or_else(|error| panic!("{error}"));
        let length =
            usize::try_from(read_u32(buffer, vector).unwrap_or_else(|error| panic!("{error}")))
                .unwrap_or_else(|error| panic!("{error}"));
        &buffer[vector + 4..vector + 4 + length]
    }
}
