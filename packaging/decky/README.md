# Decky plugin

This plugin is intentionally kept in the Hyperion Capture repository so that a
release can atomically version the Rust binary, Flatpak manifest, privileged
launcher, and Game Mode UI.

The backend has Decky's `root` flag. It installs the Flatpak bundle from `bin/`,
copies the trusted host `/usr/bin/bwrap` to the root-owned persistent directory
`/var/lib/decky-hyperion-capture`, makes that copy setuid, and selects it with
`FLATPAK_BWRAP` when starting the system Flatpak. This is the mechanism used by
Decky Sunshine to make KMS capture work from a Flatpak on SteamOS.

The setuid copy is recreated from the OS-owned binary before every launch and
removed when the plugin is uninstalled. Never place it in the plugin directory:
Decky plugin files are writable by the `deck` user, which would turn replacement
of the copied executable into root code execution.

Build the frontend with:

```sh
bun install
bun run build
```

The release ZIP must contain at least `dist/`, `main.py`, `py_modules/`,
`plugin.json`, and the generated Flatpak bundle under `bin/`.

After placing the generated Flatpak bundle in `bin/`, assemble that ZIP with:

```sh
./build-package.sh
```
