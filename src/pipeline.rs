use std::time::Duration;

use hyperion_client::{ClientConfig, FlatbufferClient, FlatbufferConfig, HyperionClient};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{info, warn};

use crate::{
    capture::{CaptureSource, KmsCapture, TestPatternCapture},
    cli::{CaptureMethod, Cli, HyperionTransport},
};

enum Publisher {
    Flatbuffers(FlatbufferClient),
    JsonImage(HyperionClient),
}

impl Publisher {
    async fn send_rgb8(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u8],
        duration_ms: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Flatbuffers(client) => client
                .send_rgb8(width, height, pixels, duration_ms)
                .await
                .map_err(Into::into),
            Self::JsonImage(client) => client
                .send_rgb8(width, height, pixels, duration_ms)
                .await
                .map_err(Into::into),
        }
    }
}

pub async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let mut publisher = match cli.hyperion_transport {
        HyperionTransport::Flatbuffers => {
            let host = cli.hyperion_url.host_str().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Hyperion URL must contain a host for FlatBuffers streaming",
                )
            })?;
            let mut config = FlatbufferConfig::new(host);
            config.port = cli.hyperion_flatbuffer_port;
            config.priority = cli.priority;
            Publisher::Flatbuffers(FlatbufferClient::new(config)?)
        }
        HyperionTransport::JsonImage => {
            if cli.fps > 25 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Hyperion's JSON image transport supports at most 25 FPS",
                )
                .into());
            }
            let mut config = ClientConfig::new(cli.hyperion_url);
            config.token = cli.hyperion_token;
            config.priority = cli.priority;
            config.jpeg_quality = cli.jpeg_quality;
            Publisher::JsonImage(HyperionClient::new(config)?)
        }
    };

    let mut source: Box<dyn CaptureSource> = match cli.capture {
        CaptureMethod::Kms => Box::new(match cli.drm_connector.as_deref() {
            Some(connector) => KmsCapture::open_connector(cli.drm_device, Some(connector))?,
            None => KmsCapture::open(cli.drm_device)?,
        }),
        CaptureMethod::TestPattern => Box::new(TestPatternCapture::new(cli.width, cli.height)),
    };

    let frame_interval = Duration::from_secs_f64(1.0 / f64::from(cli.fps));
    let duration_ms =
        u32::try_from(frame_interval.as_millis().saturating_mul(3)).unwrap_or(u32::MAX);
    let mut ticker = interval(frame_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    info!(fps = cli.fps, "capture pipeline started");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("shutdown requested");
                return Ok(());
            }
            _ = ticker.tick() => {
                let frame = match source.capture() {
                    Ok(frame) => frame,
                    Err(error) => {
                        // KMS briefly has no active plane while one compositor
                        // releases the display and the next modesets it. Retry
                        // after the next tick so session switching cannot tear
                        // down the long-running service.
                        warn!(%error, "failed to capture frame; will re-enumerate and retry");
                        continue;
                    }
                };
                if let Err(error) = publisher
                    .send_rgb8(frame.width(), frame.height(), frame.pixels(), duration_ms)
                    .await
                {
                    warn!(%error, "failed to publish frame");
                }
            }
        }
    }
}
