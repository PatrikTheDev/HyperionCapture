//! Typed clients for sending captured frames to Hyperion.
//!
//! [`FlatbufferClient`] is intended for continuous raw RGB video. The existing
//! [`HyperionClient`] sends independent JPEG images through the HTTP JSON API,
//! which Hyperion limits to 25 updates per second.

mod flatbuffer;

pub use flatbuffer::{
    DEFAULT_FLATBUFFER_PORT, Error as FlatbufferError, FlatbufferClient, FlatbufferConfig,
    MAX_FLATBUFFER_PRIORITY, MIN_FLATBUFFER_PRIORITY,
};

use std::io::Cursor;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{ImageBuffer, Rgb};
use reqwest::{Client as HttpClient, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;

/// Default input priority used for captured frames.
pub const DEFAULT_PRIORITY: u16 = 150;
/// Lowest priority accepted by Hyperion for image inputs.
pub const MIN_PRIORITY: u16 = 1;
/// Highest priority accepted by Hyperion for image inputs.
pub const MAX_PRIORITY: u16 = 253;

/// Configuration for the HTTP image [`HyperionClient`].
#[derive(Clone, Debug)]
pub struct ClientConfig {
    /// Hyperion web server base URL, usually `http://host:8090`.
    pub base_url: Url,
    /// Optional Hyperion bearer token.
    pub token: Option<String>,
    /// Hyperion input priority. Lower values win.
    pub priority: u16,
    /// Origin shown in Hyperion's source list.
    pub origin: String,
    /// JPEG quality in the inclusive range 1..=100.
    pub jpeg_quality: u8,
}

impl ClientConfig {
    /// Creates a configuration with conservative streaming defaults.
    #[must_use]
    pub fn new(base_url: Url) -> Self {
        Self {
            base_url,
            token: None,
            priority: DEFAULT_PRIORITY,
            origin: "hyperion-capture".to_owned(),
            jpeg_quality: 85,
        }
    }
}

/// An asynchronous client for Hyperion's HTTP JSON image endpoint.
#[derive(Clone, Debug)]
pub struct HyperionClient {
    http: HttpClient,
    endpoint: Url,
    config: ClientConfig,
}

impl HyperionClient {
    /// Builds a client from `config`.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON API endpoint cannot be derived from the base URL.
    pub fn new(config: ClientConfig) -> Result<Self, Error> {
        if !(1..=100).contains(&config.jpeg_quality) {
            return Err(Error::InvalidJpegQuality(config.jpeg_quality));
        }
        if !(MIN_PRIORITY..=MAX_PRIORITY).contains(&config.priority) {
            return Err(Error::InvalidPriority(config.priority));
        }

        let endpoint = config
            .base_url
            .join("json-rpc/input/image")
            .map_err(Error::InvalidEndpoint)?;

        Ok(Self {
            http: HttpClient::new(),
            endpoint,
            config,
        })
    }

    /// Encodes one packed RGB8 frame as JPEG and sends it to Hyperion.
    ///
    /// `duration_ms` should be slightly longer than the caller's frame interval,
    /// so Hyperion automatically expires the source if capture stops.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid dimensions, encoding failures, transport
    /// failures, non-success HTTP status codes, or a rejected Hyperion command.
    pub async fn send_rgb8(
        &self,
        width: u32,
        height: u32,
        pixels: &[u8],
        duration_ms: u32,
    ) -> Result<(), Error> {
        let image = ImageBuffer::<Rgb<u8>, _>::from_raw(width, height, pixels)
            .ok_or(Error::InvalidRgbBuffer { width, height })?;
        let mut encoded = Vec::new();
        let mut jpeg_writer = image::codecs::jpeg::JpegEncoder::new_with_quality(
            Cursor::new(&mut encoded),
            self.config.jpeg_quality,
        );
        jpeg_writer.encode_image(&image).map_err(Error::Encode)?;

        let request = ImageRequest {
            command: "image",
            image_data: STANDARD.encode(encoded),
            name: "screen",
            // Hyperion 2.2.x only accepts `auto` and determines JPEG from the payload.
            format: "auto",
            priority: self.config.priority,
            duration: duration_ms,
            origin: &self.config.origin,
        };

        let mut builder = self.http.post(self.endpoint.clone()).json(&request);
        if let Some(token) = &self.config.token {
            builder = builder.bearer_auth(token);
        }

        let response = builder.send().await.map_err(Error::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Http { status, body });
        }

        let response: ApiResponse = response.json().await.map_err(Error::Transport)?;
        if response.success {
            Ok(())
        } else {
            Err(Error::Rejected(response.error.unwrap_or_else(|| {
                "Hyperion rejected the image without an error message".to_owned()
            })))
        }
    }
}

#[derive(Debug, Serialize)]
struct ImageRequest<'a> {
    command: &'static str,
    #[serde(rename = "imagedata")]
    image_data: String,
    name: &'static str,
    format: &'static str,
    priority: u16,
    duration: u32,
    origin: &'a str,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    success: bool,
    #[serde(default)]
    error: Option<String>,
}

/// Errors produced by the Hyperion client.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The base URL could not be converted to a JSON API endpoint.
    #[error("invalid Hyperion endpoint: {0}")]
    InvalidEndpoint(url::ParseError),
    /// JPEG quality was outside its valid range.
    #[error("JPEG quality must be between 1 and 100, got {0}")]
    InvalidJpegQuality(u8),
    /// Input priority was outside Hyperion's accepted range.
    #[error("Hyperion priority must be between {MIN_PRIORITY} and {MAX_PRIORITY}, got {0}")]
    InvalidPriority(u16),
    /// The packed RGB buffer length did not match its dimensions.
    #[error("RGB buffer does not contain exactly {width}x{height} pixels")]
    InvalidRgbBuffer {
        /// Declared image width.
        width: u32,
        /// Declared image height.
        height: u32,
    },
    /// JPEG encoding failed.
    #[error("failed to encode captured image: {0}")]
    Encode(image::ImageError),
    /// The HTTP request failed.
    #[error("Hyperion request failed: {0}")]
    Transport(reqwest::Error),
    /// Hyperion returned a non-success HTTP status.
    #[error("Hyperion returned HTTP {status}: {body}")]
    Http {
        /// HTTP response status.
        status: StatusCode,
        /// Response body returned by Hyperion.
        body: String,
    },
    /// Hyperion accepted the request but rejected the command.
    #[error("Hyperion rejected the image: {0}")]
    Rejected(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_jpeg_quality() {
        let mut config = ClientConfig::new(
            Url::parse("http://127.0.0.1:8090/").unwrap_or_else(|error| panic!("{error}")),
        );
        config.jpeg_quality = 0;

        assert!(matches!(
            HyperionClient::new(config),
            Err(Error::InvalidJpegQuality(0))
        ));
    }

    #[test]
    fn derives_image_endpoint() {
        let config = ClientConfig::new(
            Url::parse("http://127.0.0.1:8090/").unwrap_or_else(|error| panic!("{error}")),
        );
        let client = HyperionClient::new(config).unwrap_or_else(|error| panic!("{error}"));

        assert_eq!(
            client.endpoint.as_str(),
            "http://127.0.0.1:8090/json-rpc/input/image"
        );
    }

    #[test]
    fn rejects_reserved_priority() {
        let mut config = ClientConfig::new(
            Url::parse("http://127.0.0.1:8090/").unwrap_or_else(|error| panic!("{error}")),
        );
        config.priority = 254;

        assert!(matches!(
            HyperionClient::new(config),
            Err(Error::InvalidPriority(254))
        ));
    }

    #[test]
    fn serializes_the_documented_image_request() {
        let request = ImageRequest {
            command: "image",
            image_data: "aW1hZ2U=".to_owned(),
            name: "screen",
            format: "auto",
            priority: DEFAULT_PRIORITY,
            duration: 1_000,
            origin: "hyperion-capture",
        };

        let value = serde_json::to_value(request)
            .unwrap_or_else(|error| panic!("failed to serialize image request: {error}"));

        assert_eq!(
            value,
            serde_json::json!({
                "command": "image",
                "imagedata": "aW1hZ2U=",
                "name": "screen",
                "format": "auto",
                "priority": 150,
                "duration": 1_000,
                "origin": "hyperion-capture"
            })
        );
    }
}
