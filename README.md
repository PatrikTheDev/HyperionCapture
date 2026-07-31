# Hyperion Capture

Hyperion Capture is a Linux screen-capture server that publishes the desktop as
an image input to [Hyperion](https://github.com/hyperion-project/hyperion.ng).
The capture boundary is designed around direct DRM/KMS access, the
compositor-independent mechanism used by Sunshine. That makes the eventual
backend suitable for Gamescope and avoids coupling the server to X11, GNOME,
KDE, or a particular Wayland compositor.

> [!IMPORTANT]
> This repository is a bootstrap. The executable, pipeline, test-pattern source,
> and Hyperion API client work; DRM framebuffer/DMA-BUF capture is deliberately
> isolated but is the next implementation milestone.

## Workspace

- `hyperion-capture`: the executable, capture backends, pacing, and lifecycle.
- `crates/hyperion-client`: reusable typed client for Hyperion's HTTP JSON API.

Frames are encoded as JPEG and sent to `/json-rpc/input/image`. Hyperion limits
JSON API images to 25 updates per second, so the executable defaults to 20 FPS
and rejects values above 25. A future FlatBuffers transport can be added to the
client crate if lower overhead or raw image streaming is required.

## Develop on macOS

The Rust toolchain is the only required macOS dependency:

```sh
brew install rust
cargo --version
just check
```

If `just` is not installed, either run `brew install just` or execute the three
Cargo commands in the `check` recipe manually. macOS can build, lint, and test
the project, but cannot run the DRM/KMS backend.

## Run

Use the generated source to validate credentials and the Hyperion connection:

```sh
export HYPERION_TOKEN='your-token'
cargo run -- \
  --capture test-pattern \
  --hyperion-url http://hyperion.local:8090/
```

Configuration is available through flags and the `HYPERION_URL`,
`HYPERION_TOKEN`, and `HYPERION_PRIORITY` environment variables. Run
`cargo run -- --help` for the complete list.

## Linux capture plan

The `capture::KmsCapture` module is the integration point for the direct KMS
path. It should:

1. Enumerate DRM cards, connectors, CRTCs, and active planes.
2. Select an output by connector name rather than desktop-environment APIs.
3. Import the active framebuffer's DMA-BUF planes through GBM/EGL.
4. Convert or map the image into packed RGB while respecting modifiers,
   rotation, pitch, and multi-plane formats.
5. Re-enumerate after modesets so Gamescope and display changes are handled.

Like Sunshine's KMS service, production access must be granted narrowly. Prefer
a hardened systemd service with access to the selected `/dev/dri/card*` device;
do not run the process as root. The exact capability and device policy will be
added alongside the backend, once its system calls are known.

## Hyperion setup

Create a non-admin API token in Hyperion and pass it through
`HYPERION_TOKEN`. The default priority is 150; lower values take precedence in
Hyperion. Each frame has a short expiry, so lighting falls back to another input
when capture stops unexpectedly.

## Quality gates

`just check` verifies formatting, runs Clippy with warnings denied, and executes
all workspace tests. CI runs the same checks on every push and pull request.

## License

MIT

