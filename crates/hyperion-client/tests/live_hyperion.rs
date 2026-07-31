//! Live API contract tests for the Docker-backed Hyperion deployment.

use hyperion_client::{
    ClientConfig, FlatbufferClient, FlatbufferConfig, HyperionClient, MAX_FLATBUFFER_PRIORITY,
    MAX_PRIORITY,
};
use url::Url;

#[tokio::test]
#[ignore = "requires the Docker Hyperion deployment"]
async fn sends_an_rgb_frame_to_hyperion() {
    let base_url = std::env::var("HYPERION_INTEGRATION_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8090/".to_owned());
    let url = Url::parse(&base_url).unwrap_or_else(|error| panic!("invalid test URL: {error}"));
    let mut config = ClientConfig::new(url);
    // Exercise the documented upper priority boundary against the real daemon.
    config.priority = MAX_PRIORITY;
    let client = HyperionClient::new(config)
        .unwrap_or_else(|error| panic!("failed to create client: {error}"));

    // Four distinct pixels exercise dimensions, RGB packing, JPEG encoding,
    // transport, and Hyperion's image-command handling.
    let pixels = [
        255, 0, 0, 0, 255, 0, // red, green
        0, 0, 255, 255, 255, 255, // blue, white
    ];
    client
        .send_rgb8(2, 2, &pixels, 1_000)
        .await
        .unwrap_or_else(|error| panic!("Hyperion rejected the test frame: {error}"));
}

#[tokio::test]
#[ignore = "requires the Docker Hyperion deployment"]
async fn streams_an_rgb_frame_to_hyperion_over_flatbuffers() {
    let port = std::env::var("HYPERION_INTEGRATION_FLATBUFFER_PORT")
        .unwrap_or_else(|_| "19400".to_owned())
        .parse::<u16>()
        .unwrap_or_else(|error| panic!("invalid FlatBuffers test port: {error}"));
    let mut config = FlatbufferConfig::new("127.0.0.1");
    config.port = port;
    config.priority = MAX_FLATBUFFER_PRIORITY;
    config.origin = "hyperion-capture-integration-test".to_owned();
    let mut client = FlatbufferClient::new(config)
        .unwrap_or_else(|error| panic!("failed to create FlatBuffers client: {error}"));

    let pixels = [
        255, 0, 0, 0, 255, 0, // red, green
        0, 0, 255, 255, 255, 255, // blue, white
    ];
    client
        .send_rgb8(2, 2, &pixels, 1_000)
        .await
        .unwrap_or_else(|error| panic!("Hyperion rejected the FlatBuffers frame: {error}"));
    assert!(client.is_connected());
}
