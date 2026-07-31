# Hyperion Capture

Hyperion Capture is a Linux screen-capture server that publishes the desktop as
an image input to [Hyperion](https://github.com/hyperion-project/hyperion.ng).
The capture boundary is designed around direct DRM/KMS access, the
compositor-independent mechanism used by Sunshine. That makes the eventual
backend suitable for Gamescope and avoids coupling the server to X11, GNOME,
KDE, or a particular Wayland compositor.

## Workspace

- `hyperion-capture`: the executable, capture backends, pacing, and lifecycle.
- `crates/hyperion-client`: reusable typed clients for Hyperion's FlatBuffers
  stream and HTTP JSON image APIs.

The default transport keeps a TCP connection to Hyperion's FlatBuffers server
on port 19400 and sends packed RGB frames without JPEG or Base64 conversion.
The HTTP `/json-rpc/input/image` transport remains available as a compatibility
fallback; it encodes each frame as JPEG and is limited by Hyperion to 25 FPS.

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

On Linux, direct KMS capture defaults to the first active output on
`/dev/dri/card0`. Select another stable connector name when needed:

```sh
cargo run --release -- \
  --capture kms \
  --drm-device /dev/dri/card1 \
  --drm-connector DP-1 \
  --hyperion-url http://hyperion.local:8090/
```

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

## Linux DRM/KMS capture

`capture::KmsCapture` follows Sunshine's compositor-independent path. It
enumerates connected DRM connectors and their CRTCs, selects the active primary
plane, reads the current framebuffer metadata with `GETFB2` (and the legacy
`GETFB` fallback), exports its GEM planes as DMA-BUFs, and imports them as EGL
images through a GBM-backed display. OpenGL performs modifier-aware GPU
readback and normalizes the framebuffer to packed RGB8 at the Hyperion boundary.

All connector, encoder, CRTC, plane, and framebuffer IDs are rediscovered for
each frame. DRM IDs can change during a modeset, so this is what allows a
long-running process to follow compositor replacement such as SteamOS Game
Mode switching to or from KDE Plasma. There is no X11, portal, or
compositor-specific fallback.

Reading framebuffer GEM handles requires `CAP_SYS_ADMIN` on current kernels.
The backend drops it from the effective set after initialization, raises it
only around `GETFB2`/`GETFB`, and drops it immediately afterwards. Grant the
capability through a hardened systemd unit's permitted/bounding set and limit
device access to the selected `/dev/dri/card*`; do not run the service as root.
Never expose a capability-bearing development binary from a writable build
directory as a production service.

Unsafe GBM/EGL/OpenGL FFI is isolated in the `kms-egl` crate, whose safe API
owns the duplicated DRM descriptor, validates plane metadata, checks allocation
sizes, and keeps EGL/GL state thread-affine. The main capture crate remains
`unsafe_code = "forbid"`. Drivers must expose EGL DMA-BUF import and, for tiled
scanout, the modifier extension; otherwise capture returns an explicit error.
Validate the target AMD/Intel hardware, including its actual modifiers and any
HDR mode, before deployment.

## SteamOS packaging

The initial SteamOS distribution is a system Flatpak controlled by a companion
root Decky plugin. The Flatpak provides an immutable, updateable payload and a
matching graphics runtime. The Decky backend supplies the host-side privileged
`bwrap` required for KMS framebuffer export, supervises the process, and exposes
configuration in Game Mode. Both packages live in this repository so releases
cannot accidentally pair incompatible versions.

See [`packaging/flatpak`](packaging/flatpak/README.md) for the Flatpak build and
[`packaging/decky`](packaging/decky/README.md) for the launcher security model
and plugin bundle layout.

## Tart Linux testing

An Ubuntu 24.04 ARM64 Tart image exercises Linux compilation, virtual KMS
enumeration, permissions, active-plane capture, and re-enumeration around a
direct-KMS Weston session:

```sh
just tart-vm-build  # first run downloads and configures the large base image
just tart-vm-up     # terminal 1; stays open and owns the VM

# In terminal 2. Run provision once after creating a new image.
just tart-vm-provision
just tart-vm-test
just tart-vm-down
```

The checkout is mounted read-only and guest build artifacts stay on the VM.
Tart's virtual GPU is useful for KMS integration but cannot reproduce physical
AMD/Intel modifiers, GPU passthrough, or the real Gamescope↔Plasma handoff.
Those remain native SteamOS acceptance tests. The complete tiered strategy,
optional modeset fixture, image lifecycle, and future OCI push handoff are in
[`vm/tart/TESTING.md`](vm/tart/TESTING.md).

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
