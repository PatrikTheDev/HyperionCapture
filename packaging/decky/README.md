# Decky plugin

This plugin is intentionally kept in the Hyperion Capture repository so that a
release can atomically version the Rust binary, Flatpak manifest, privileged
launcher, and Game Mode UI.

The backend has Decky's `root` flag. It installs the Flatpak bundle from `bin/`,
copies the trusted host `/usr/bin/bwrap` to the root-owned persistent directory
`/var/lib/decky-hyperion-capture`, makes that copy setuid, and selects it with
`FLATPAK_BWRAP` when starting the system Flatpak. This is the mechanism used by
Decky Sunshine to make KMS capture work from a Flatpak on SteamOS.

Auto-start waits for a running `gamescope` process, and manual launches also
require Gamescope. Stopping capture or unloading the plugin cancels a pending
auto-start. This process check orders startup but does not prove Gamescope has
acquired DRM master; the capture binary also immediately releases any master
ownership implicitly granted when opening the card. An existing capture
process still survives switches to Desktop Mode and Decky reloads.

The setuid copy is recreated from the OS-owned binary before every launch and
removed when the plugin is uninstalled. Never place it in the plugin directory:
Decky plugin files are writable by the `deck` user, which would turn replacement
of the copied executable into root code execution.

Build the frontend with:

```sh
bun install
bun run build
```

The release ZIP contains a single `HyperionCapture/` directory. That directory
must contain at least `dist/`, `main.py`, `py_modules/`, `package.json`,
`plugin.json`, and the generated Flatpak bundle under `bin/`. The packaging
script validates this Decky-compatible layout before publishing the archive.

The backend reports Flatpak process liveness separately from capture health.
While the payload is running, it watches the bounded tail of `capture.log` for
repeated, current capture or publication failures. The Game Mode UI refreshes
this status automatically and surfaces the latest useful error. A future local
Unix status socket can replace this log-based health signal with explicit frame
delivery acknowledgements.

The backend reads the installed payload version with SteamOS's supported
`flatpak list --columns=application,version` interface and compares it with the
Decky package version. A mismatch triggers a reinstall from the bundled Flatpak,
and a running old deployment is stopped before the replacement is launched.

After placing the generated Flatpak bundle in `bin/`, assemble that ZIP with:

```sh
./build-package.sh
```

Pushing a `v<version>` tag runs `.github/workflows/release.yml`. The tag must
match both the Cargo workspace version and `package.json`; the workflow builds
the Flatpak from the tagged tree, assembles `HyperionCapture.zip`, publishes the
Podman-compatible OCI image, generates the Podman setup archive and
`SHA256SUMS`, and attaches all downloadable artifacts to the GitHub release. The
tag must also match the Flatpak AppStream release version.
