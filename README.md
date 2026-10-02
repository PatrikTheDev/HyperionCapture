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

Configuration is available through flags and environment variables. The
deployment-oriented variables are `HYPERION_URL`, `HYPERION_TOKEN`,
`HYPERION_TRANSPORT`, `HYPERION_FLATBUFFER_PORT`, `HYPERION_PRIORITY`,
`HYPERION_FPS`, `HYPERION_JPEG_QUALITY`, `HYPERION_CAPTURE`,
`HYPERION_DRM_DEVICE`, `HYPERION_DRM_CONNECTOR`, and
`HYPERION_OUTPUT_HEIGHT`. The host from `HYPERION_URL` is also used for
FlatBuffers.

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
  --output-height 480 \
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
images through a GBM-backed display. An OpenGL ES 2 shader downsamples in
linear light to a maximum height of 480 pixels by default, preserving aspect
ratio and never enlarging smaller inputs. Only the small result is read back
and normalized to packed RGB8 at the Hyperion boundary. A 1920x1080 scanout
therefore becomes 854x480 instead of consuming roughly 1 Gbit/s at 20 FPS.

All connector, encoder, CRTC, plane, and framebuffer IDs are rediscovered for
each frame. DRM IDs can change during a modeset, so this is what allows a
long-running process to follow compositor replacement such as SteamOS Game
Mode switching to or from KDE Plasma. There is no X11, portal, or
compositor-specific fallback.

Opening a DRM primary node can implicitly grant master ownership when the
compositor has not acquired it yet. Capture immediately issues `DROP_MASTER`
before initializing KMS or EGL, and fails initialization if it cannot release
ownership. It never requests master ownership. Launch it after the compositor;
the Decky plugin waits for a Gamescope process before auto-starting.

Reading framebuffer GEM handles requires `CAP_SYS_ADMIN` on current kernels.
The backend drops it from the effective set after initialization, raises it
only around `GETFB2`/`GETFB`, and drops it immediately afterwards. Grant the
capability through a hardened systemd unit's permitted/bounding set and limit
device access to the selected `/dev/dri/card*`; do not run the service as root.
Never expose a capability-bearing development binary from a writable build
directory as a production service.

Unsafe GBM/EGL/OpenGL ES FFI is isolated in the `kms-egl` crate, whose safe API
owns the duplicated DRM descriptor, validates plane metadata, checks allocation
sizes, and keeps EGL/GL state thread-affine. The main capture crate remains
`unsafe_code = "forbid"`. Drivers must expose EGL DMA-BUF import and, for tiled
scanout, the modifier extension; otherwise capture returns an explicit error.
Validate the target AMD/Intel hardware, including its actual modifiers and any
HDR mode, before deployment.

### HDR and scanout formats

The connector's `HDR_OUTPUT_METADATA` property is checked for every frame, so
HDR changes are followed without restarting capture. Single-plane XRGB/ARGB and
XBGR/ABGR scanout in 8-bit and 10-bit variants is supported. PQ and HLG BT.2020
are decoded in the scaling shader, converted to BT.709, and tone-mapped into the
SDR RGB8 values consumed by Hyperion. This is HDR-aware ambient-light capture;
Hyperion is not sent an HDR signal. The tone mapper uses a 203-nit SDR reference
white and an ACES fitted curve.

Unknown HDR EOTFs and non-RGB or multi-plane DMA-BUF formats fail explicitly.
Multi-plane YUV requires `GL_TEXTURE_EXTERNAL_OES` plus KMS color range/matrix
metadata and is left as a documented extension in `kms-egl`; silently treating
it as RGB would produce incorrect colors. The capture currently follows the
logical framebuffer orientation. KMS panel rotation is deliberately not
reapplied, while unusual primary-plane source crops remain future shader-UV
work. Hardware cursor and overlay planes are not included, which is appropriate
for ambient lighting but should be revisited for general-purpose screenshots.

## Distribution

Tagged releases provide two supported deployments and one reusable payload:

| Artifact | Intended use | Privileged boundary |
| --- | --- | --- |
| `HyperionCapture.zip` | Recommended SteamOS installation through Decky | Root Decky backend and its trusted `bwrap` copy |
| `ghcr.io/patrikthedev/hyperion-capture` plus `HyperionCapture-podman.tar.gz` | SteamOS or another systemd Linux host using rootful Podman | A narrowly configured system Quadlet |
| `io.github.PatrikTheDev.HyperionCapture.flatpak` | Payload for Decky or another trusted host launcher | The Flatpak alone cannot obtain `CAP_SYS_ADMIN` |

`SHA256SUMS` covers every downloadable release artifact. Cargo, Decky, and
Flatpak versions must all match the release tag. The OCI image is published for
`linux/amd64` and `linux/arm64` with full-version tags, a moving major/minor tag,
and `latest`.

### Decky and Flatpak

This is the integrated SteamOS distribution. The system Flatpak provides an
immutable payload and matching Freedesktop graphics runtime. Its companion root
Decky plugin installs that exact bundle, supplies the host-side privileged
`bwrap` needed for framebuffer export, supervises the process, and exposes its
configuration in Game Mode. Keeping the payload and launcher in one release
prevents incompatible versions from being paired accidentally.

Install `HyperionCapture.zip` through Decky's plugin installer. The embedded
Flatpak is installed and updated by the plugin; installing the standalone
`.flatpak` release asset is only useful for development or a custom privileged
launcher. An ordinary `flatpak run` can see `/dev/dri` but cannot obtain the
host `CAP_SYS_ADMIN` required by `GETFB2`.

See [`packaging/flatpak`](packaging/flatpak/README.md) for the Flatpak build and
[`packaging/decky`](packaging/decky/README.md) for the launcher security model,
plugin lifecycle, and archive layout.

### Podman

The Podman distribution is a non-root OCI image plus a rootful system Quadlet.
It is suitable when Decky integration is unnecessary or the service should be
managed directly through systemd. The image ships a Mesa GBM/EGL/GLES runtime
and gives only the immutable capture executable the `cap_sys_admin` file
capability. The Quadlet exposes one DRM primary node, drops all other container
capabilities, uses a read-only root filesystem, and runs as UID/GID 65532.

Download and extract `HyperionCapture-podman.tar.gz`, then install it:

```sh
tar -xzf HyperionCapture-podman.tar.gz
sudo ./install.sh /dev/dri/card0
sudoedit /etc/hyperion-capture/podman.env
sudo systemctl start hyperion-capture.service
sudo journalctl -u hyperion-capture.service -f
```

The host must use cgroup v2 and a Podman version with Quadlet support. This is
intentionally a rootful service: rootless Podman's namespaced capability does
not satisfy the kernel's framebuffer-handle check. Do not add `--privileged`.

SteamOS may assign AMDGPU a card other than `/dev/dri/card0`. Inspect
`/dev/dri` and `/sys/class/drm/card*-*/status`, then pass the card that owns the
active connector to `install.sh`. The installer exposes only that node and adds
its numeric owning group to the non-root container process. If Hyperion runs on
the same host, remember that `127.0.0.1` inside the default container network is
the container itself; use a host-reachable address.

To build the image locally instead of pulling it:

```sh
just podman-build
```

Then replace the Quadlet's `Image=` with
`localhost/hyperion-capture:dev` and set `Pull=never`. Complete build, direct
`podman run`, security, logging, and troubleshooting instructions are in
[`packaging/podman`](packaging/podman/README.md).

### Health and readiness ownership

The Rust capture process owns health and readiness because it alone can tell
whether DRM capture and Hyperion publication are succeeding. Process existence
is liveness only. Readiness should mean that a frame was captured recently and
Hyperion accepted a frame recently; a disconnected Hyperion server or a KMS
handoff is temporarily not ready even while the process remains healthy. A
future local Unix status socket should expose those timestamps and the current
connector/output dimensions. The Decky Python backend should proxy that status
to Game Mode rather than inventing readiness from `flatpak ps`. No public HTTP
health endpoint is planned.

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
optional modeset fixture, image lifecycle, and published OCI image are in
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
