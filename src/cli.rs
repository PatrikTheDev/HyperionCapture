use clap::{Parser, ValueEnum};
use url::Url;

/// Capture the Linux display and publish frames to Hyperion.
#[derive(Clone, Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    /// Hyperion server URL; its host is also used for flat-buffer streaming.
    #[arg(long, env = "HYPERION_URL", default_value = "http://127.0.0.1:8090/")]
    pub hyperion_url: Url,

    /// Hyperion API bearer token.
    ///
    /// Used only by the JSON image transport; Hyperion's flat-buffer protocol
    /// does not define token authentication.
    #[arg(long, env = "HYPERION_TOKEN", hide_env_values = true)]
    pub hyperion_token: Option<String>,

    /// Transport used to publish captured frames.
    #[arg(
        long,
        env = "HYPERION_TRANSPORT",
        value_enum,
        default_value_t = HyperionTransport::Flatbuffers
    )]
    pub hyperion_transport: HyperionTransport,

    /// Hyperion flat-buffer TCP port; the host comes from --hyperion-url.
    #[arg(long, env = "HYPERION_FLATBUFFER_PORT", default_value_t = 19_400)]
    pub hyperion_flatbuffer_port: u16,

    /// Input priority; use 100-199 for flat buffers or 1-253 for JSON images.
    #[arg(
        long,
        env = "HYPERION_PRIORITY",
        default_value_t = 150,
        value_parser = clap::value_parser!(u16).range(1..=253)
    )]
    pub priority: u16,

    /// Capture frames per second. The JSON image fallback supports at most 25.
    #[arg(long, default_value = "20", value_parser = clap::value_parser!(u16).range(1..=120))]
    pub fps: u16,

    /// JPEG quality used only by the JSON image transport.
    #[arg(long, default_value = "85", value_parser = clap::value_parser!(u8).range(1..=100))]
    pub jpeg_quality: u8,

    /// Capture source.
    #[arg(long, value_enum, default_value_t = CaptureMethod::Kms)]
    pub capture: CaptureMethod,

    /// DRM card to capture, such as /dev/dri/card0.
    #[arg(long, default_value = "/dev/dri/card0")]
    pub drm_device: String,

    /// DRM connector to capture, such as DP-1; defaults to the first active output.
    #[arg(long)]
    pub drm_connector: Option<String>,

    /// Test-pattern width; only used with `--capture test-pattern`.
    #[arg(long, default_value = "320", value_parser = clap::value_parser!(u32).range(1..))]
    pub width: u32,

    /// Test-pattern height; only used with `--capture test-pattern`.
    #[arg(long, default_value = "180", value_parser = clap::value_parser!(u32).range(1..))]
    pub height: u32,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CaptureMethod {
    /// Direct DRM/KMS capture, independent of the desktop compositor.
    Kms,
    /// Generated RGB frames for development and API testing.
    TestPattern,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum HyperionTransport {
    /// Persistent raw RGB streaming over Hyperion's flat-buffer TCP protocol.
    Flatbuffers,
    /// Independent JPEG images over Hyperion's HTTP JSON API.
    JsonImage,
}
