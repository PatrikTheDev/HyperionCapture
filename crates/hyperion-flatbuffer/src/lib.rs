//! Safe façade over generated bindings for Hyperion's `FlatBuffers` protocol.

use flatbuffers::FlatBufferBuilder;

#[allow(
    clippy::all,
    clippy::pedantic,
    clippy::restriction,
    missing_docs,
    unsafe_code
)]
mod request {
    include!(concat!(env!("OUT_DIR"), "/hyperion_request_generated.rs"));
}

#[allow(
    clippy::all,
    clippy::pedantic,
    clippy::restriction,
    missing_docs,
    unsafe_code
)]
mod reply {
    include!(concat!(env!("OUT_DIR"), "/hyperion_reply_generated.rs"));
}

/// A reusable request encoder.
pub struct Encoder {
    builder: FlatBufferBuilder<'static>,
}

impl Encoder {
    /// Creates an encoder with a small initial allocation that grows to fit frames.
    #[must_use]
    pub fn new() -> Self {
        Self {
            builder: FlatBufferBuilder::with_capacity(1024),
        }
    }

    /// Encodes a source-registration request.
    pub fn register(&mut self, origin: &str, priority: i32) -> &[u8] {
        use request::hyperionnet::{Command, Register, RegisterArgs, Request, RequestArgs};

        self.builder.reset();
        let origin = self.builder.create_string(origin);
        let register = Register::create(
            &mut self.builder,
            &RegisterArgs {
                origin: Some(origin),
                priority,
            },
        );
        let request = Request::create(
            &mut self.builder,
            &RequestArgs {
                command_type: Command::Register,
                command: Some(register.as_union_value()),
            },
        );
        self.builder.finish_minimal(request);
        self.builder.finished_data()
    }

    /// Encodes one packed RGB8 image request.
    pub fn image(&mut self, width: i32, height: i32, pixels: &[u8], duration: i32) -> &[u8] {
        use request::hyperionnet::{
            Command, Image, ImageArgs, ImageType, RawImage, RawImageArgs, Request, RequestArgs,
        };

        self.builder.reset();
        let data = self.builder.create_vector(pixels);
        let raw = RawImage::create(
            &mut self.builder,
            &RawImageArgs {
                data: Some(data),
                width,
                height,
            },
        );
        let image = Image::create(
            &mut self.builder,
            &ImageArgs {
                data_type: ImageType::RawImage,
                data: Some(raw.as_union_value()),
                duration,
            },
        );
        let request = Request::create(
            &mut self.builder,
            &RequestArgs {
                command_type: Command::Image,
                command: Some(image.as_union_value()),
            },
        );
        self.builder.finish_minimal(request);
        self.builder.finished_data()
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

/// The fields relevant to a client from a Hyperion reply.
#[derive(Debug, Eq, PartialEq)]
pub struct Reply {
    /// Error reported by Hyperion, if any.
    pub error: Option<String>,
    /// Registered priority, or `-1` when the reply is not a registration response.
    pub registered: i32,
}

/// Verifies and decodes a Hyperion reply.
///
/// # Errors
///
/// Returns an error when the payload is not a valid reply according to the
/// pinned Hyperion schema.
pub fn parse_reply(buffer: &[u8]) -> Result<Reply, flatbuffers::InvalidFlatbuffer> {
    let reply = reply::hyperionnet::root_as_reply(buffer)?;
    Ok(Reply {
        error: reply.error().map(str::to_owned),
        registered: reply.registered(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use request::hyperionnet::Command;

    #[test]
    fn encodes_registration_request() {
        let mut encoder = Encoder::new();
        let request = request::hyperionnet::root_as_request(encoder.register("capture-test", 150))
            .unwrap_or_else(|error| panic!("{error}"));
        let registration = request
            .command_as_register()
            .unwrap_or_else(|| panic!("registration command is missing"));

        assert_eq!(request.command_type(), Command::Register);
        assert_eq!(registration.origin(), "capture-test");
        assert_eq!(registration.priority(), 150);
    }

    #[test]
    fn encodes_raw_rgb_image_request() {
        let pixels = [255, 0, 0, 0, 255, 0];
        let mut encoder = Encoder::new();
        let request = request::hyperionnet::root_as_request(encoder.image(2, 1, &pixels, 250))
            .unwrap_or_else(|error| panic!("{error}"));
        let image = request
            .command_as_image()
            .unwrap_or_else(|| panic!("image command is missing"));
        let raw = image
            .data_as_raw_image()
            .unwrap_or_else(|| panic!("raw image data is missing"));

        assert_eq!(request.command_type(), Command::Image);
        assert_eq!(image.duration(), 250);
        assert_eq!(raw.width(), 2);
        assert_eq!(raw.height(), 1);
        assert_eq!(raw.data().map(|data| data.bytes()), Some(pixels.as_slice()));
    }

    #[test]
    fn verifies_and_decodes_reply() {
        let mut builder = FlatBufferBuilder::new();
        let error = builder.create_string("nope");
        let reply = reply::hyperionnet::Reply::create(
            &mut builder,
            &reply::hyperionnet::ReplyArgs {
                error: Some(error),
                video: -1,
                registered: 150,
            },
        );
        builder.finish_minimal(reply);

        assert_eq!(
            parse_reply(builder.finished_data()).unwrap_or_else(|error| panic!("{error}")),
            Reply {
                error: Some("nope".to_owned()),
                registered: 150,
            }
        );
    }

    #[test]
    fn rejects_malformed_reply() {
        assert!(parse_reply(&[0, 1, 2, 3]).is_err());
    }
}
