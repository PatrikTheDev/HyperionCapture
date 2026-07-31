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
- `crates/hyperion-client`: reusable typed clients for Hyperion's FlatBuffers
  stream and HTTP JSON image APIs.
- `crates/hyperion-flatbuffer`: generated, low-level protocol bindings isolated
  from the safe client API.

The default transport keeps a TCP connection to Hyperion's FlatBuffers server
on port 19400 and sends packed RGB frames without JPEG or Base64 conversion.
The HTTP `/json-rpc/input/image` transport remains available as a compatibility
fallback; it encodes each frame as JPEG and is limited by Hyperion to 25 FPS.

## Develop on macOS

The Rust toolchain and CMake are the required macOS build dependencies. CMake
builds the pinned FlatBuffers schema compiler; a separate `flatc` installation
is not required.

```sh
brew install rust cmake
cargo --version
just check
```

If `just` is not installed, either run `brew install just` or execute the three
Cargo commands in the `check` recipe manually. macOS can build, lint, and test
the project, but cannot run the DRM/KMS backend.

## Run

Use the generated source to validate the FlatBuffers stream:

```sh
cargo run -- \
  --capture test-pattern \
  --hyperion-url http://hyperion.local:8090/
```

Configuration is available through flags and the `HYPERION_URL`,
`HYPERION_TRANSPORT`, and `HYPERION_PRIORITY` environment variables. The host
from `HYPERION_URL` is also used for FlatBuffers; its port can be changed with
`HYPERION_FLATBUFFER_PORT`.

To use the retained HTTP image transport:

```sh
export HYPERION_TOKEN='your-token'
cargo run -- --hyperion-transport json-image --capture test-pattern
```

Hyperion's FlatBuffers protocol does not carry API bearer tokens. Access to its
TCP port should therefore be restricted to trusted networks. Run
`cargo run -- --help` for the complete option list.

## Local Hyperion deployment

The repository includes a Docker Compose deployment built directly from
Hyperion's official, checksum-verified 2.2.1 Debian packages. It supports both
Apple Silicon and Intel Macs and binds its ports to localhost only.

```sh
just hyperion-up
open http://127.0.0.1:8090/
just integration
```

`just integration` sends real 2x2 RGB frames through both the persistent
FlatBuffers stream and the JPEG image endpoint. This validates both client
transports against a live daemon without requiring LED hardware. The deployment
persists configuration in a named volume. Use `just hyperion-down` to stop it
or `just hyperion-reset` to also erase that test configuration.

This environment validates the API side of the pipeline; it does not emulate a
DRM device and therefore cannot validate KMS capture.

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

Enable Hyperion's FlatBuffers server and allow TCP port 19400 from the capture
host. FlatBuffers streaming priorities must be between 100 and 199; the default
is 150. Lower values take precedence in Hyperion. Each frame has a short expiry,
and disconnecting the stream clears its registered input.

For the HTTP image fallback, create a non-admin API token in Hyperion and pass
it through `HYPERION_TOKEN`.

## Quality gates

`just check` verifies formatting, runs Clippy with warnings denied, and executes
all workspace tests (the Docker-backed test remains explicitly ignored). CI
runs the same checks on every push and pull request.

## License

MIT
