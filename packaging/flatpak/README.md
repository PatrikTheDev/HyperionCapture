# Flatpak packaging

The Flatpak contains the capture binary and its userspace graphics stack. Its
static permissions expose DRM and the network, but they do not grant the
`CAP_SYS_ADMIN` required by KMS framebuffer export. On SteamOS, the companion
Decky plugin launches the system installation through a trusted setuid copy of
the host's `bwrap`, following Decky Sunshine's working model.

The payload uses GBM, EGL, and OpenGL ES 2 from the Freedesktop runtime. GPU
downscaling and HDR-to-SDR tone mapping happen before the 480p RGB8 readback, so
the Flatpak does not transport native-resolution scanout frames to Hyperion.

Generate the Cargo source list with the upstream `flatpak-cargo-generator.py`:

```sh
python3 /path/to/flatpak-builder-tools/cargo/flatpak-cargo-generator.py \
  Cargo.lock -o packaging/flatpak/cargo-sources.json
```

Build and install locally on Linux:

```sh
flatpak install --user flathub \
  org.freedesktop.Platform//25.08 \
  org.freedesktop.Sdk//25.08 \
  org.freedesktop.Sdk.Extension.rust-stable//25.08

flatpak-builder --force-clean --user --install build-flatpak \
  packaging/flatpak/io.github.PatrikTheDev.HyperionCapture.yml
```

Create the single-file bundle embedded in a Decky release:

```sh
flatpak-builder --force-clean --repo=flatpak-repo build-flatpak \
  packaging/flatpak/io.github.PatrikTheDev.HyperionCapture.yml
flatpak build-bundle \
  --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo \
  flatpak-repo \
  packaging/decky/bin/io.github.PatrikTheDev.HyperionCapture.flatpak \
  io.github.PatrikTheDev.HyperionCapture
```

The bundle intentionally is not committed. Release automation should build it
from the tagged source and place it in the Decky plugin ZIP.
