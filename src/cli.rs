use clap::{Parser, ValueEnum};
use url::Url;

/// Capture the Linux display and publish frames to Hyperion.
#[derive(Clone, Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    /// Hyperion web server base URL.
    #[arg(long, env = "HYPERION_URL", default_value = "http://127.0.0.1:8090/")]
    pub hyperion_url: Url,

    /// Hyperion API bearer token.
    #[arg(long, env = "HYPERION_TOKEN", hide_env_values = true)]
    pub hyperion_token: Option<String>,

    /// Hyperion input priority; lower values win.
    #[arg(long, env = "HYPERION_PRIORITY", default_value_t = 150)]
    pub priority: u16,

    /// Capture frames per second (Hyperion JSON API supports at most 25).
    #[arg(long, default_value = "20", value_parser = clap::value_parser!(u8).range(1..=25))]
    pub fps: u8,

    /// JPEG quality used to send frames.
    #[arg(long, default_value = "85", value_parser = clap::value_parser!(u8).range(1..=100))]
    pub jpeg_quality: u8,

    /// Capture source.
    #[arg(long, value_enum, default_value_t = CaptureMethod::Kms)]
    pub capture: CaptureMethod,

    /// DRM card to capture, such as /dev/dri/card0.
    #[arg(long, default_value = "/dev/dri/card0")]
    pub drm_device: String,

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
