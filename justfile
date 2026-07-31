set dotenv-load := true

default: check

check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --all-features

format:
    cargo fmt --all

run *args:
    cargo run -- {{args}}

# Start a persistent local Hyperion 2.2.1 deployment.
hyperion-up:
    docker compose up --build --detach --wait hyperion

# Stop Hyperion while preserving its configuration volume.
hyperion-down:
    docker compose down

# Remove Hyperion and its persisted test configuration.
hyperion-reset:
    docker compose down --volumes

# Validate HTTP image and FlatBuffers streaming against the local daemon.
integration: hyperion-up
    HYPERION_INTEGRATION_URL=http://127.0.0.1:8090/ HYPERION_INTEGRATION_FLATBUFFER_PORT=19400 cargo test -p hyperion-client --test live_hyperion -- --ignored
