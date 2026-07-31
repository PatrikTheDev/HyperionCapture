use std::time::Duration;

use hyperion_client::{ClientConfig, HyperionClient};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{info, warn};

use crate::{
    capture::{CaptureSource, KmsCapture, TestPatternCapture},
    cli::{CaptureMethod, Cli},
};

pub async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ClientConfig::new(cli.hyperion_url);
    config.token = cli.hyperion_token;
    config.priority = cli.priority;
    config.jpeg_quality = cli.jpeg_quality;
    let client = HyperionClient::new(config)?;

    let mut source: Box<dyn CaptureSource> = match cli.capture {
        CaptureMethod::Kms => Box::new(KmsCapture::open(cli.drm_device)?),
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
                let frame = source.capture()?;
                if let Err(error) = client
                    .send_rgb8(frame.width(), frame.height(), frame.pixels(), duration_ms)
                    .await
                {
                    warn!(%error, "failed to publish frame");
                }
            }
        }
    }
}
