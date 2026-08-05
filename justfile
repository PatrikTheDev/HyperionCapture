set dotenv-load := true

default: check

check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --all-features
    python3 -m unittest discover -s packaging/decky/tests

format:
    cargo fmt --all

# Build the Decky Game Mode frontend after installing its JS dependencies.
decky-build:
    bun run --cwd packaging/decky build

# Assemble a Decky ZIP after the Flatpak bundle has been placed in bin/.
decky-package:
    packaging/decky/build-package.sh

# Build the production OCI image used by the rootful Podman deployment.
podman-build:
    podman build --file packaging/podman/Containerfile --tag localhost/hyperion-capture:dev .

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

# Clone and configure the Ubuntu DRM/KMS test VM (downloads a large Tart image).
tart-vm-build:
    vm/tart/manage.sh build

# Run the Tart VM in the foreground with its normal display; keep this open.
tart-vm-up:
    vm/tart/manage.sh start

# Run Linux checks, inspect KMS state, and exercise live framebuffer capture.
tart-vm-test:
    vm/tart/manage.sh test

# Stop the Tart VM while preserving its provisioned disk.
tart-vm-down:
    vm/tart/manage.sh stop

# Provision a running guest, or re-run the idempotent provisioner after changes.
tart-vm-provision:
    vm/tart/manage.sh provision

# Open an interactive shell in the running Tart VM.
tart-vm-shell:
    vm/tart/manage.sh shell
