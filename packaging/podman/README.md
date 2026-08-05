# Podman packaging

This directory builds and installs the rootful OCI deployment for SteamOS and
other systemd-based Linux hosts. Published version tags produce the multi-arch
image `ghcr.io/patrikthedev/hyperion-capture` and attach this directory as a
small setup archive to the GitHub release.

After the workflow publishes the package for the first time, a repository owner
must set its GHCR visibility to public. Subsequent version tags update the same
package through `GITHUB_TOKEN`; no registry credential is required to pull a
public image.

## Install the released image

Extract `HyperionCapture-podman.tar.gz` from a release, then run the installer
with the DRM primary node to capture:

```sh
sudo ./install.sh /dev/dri/card0
sudoedit /etc/hyperion-capture/podman.env
sudo systemctl start hyperion-capture.service
sudo systemctl status hyperion-capture.service
```

The Quadlet contains `[Install] WantedBy=multi-user.target`, so its generated
service participates in normal boot after `systemctl daemon-reload`; Quadlet
services themselves are generated and should not be enabled directly. Follow
logs with:

```sh
sudo journalctl -u hyperion-capture.service -f
```

Before starting it, identify the active DRM card:

```sh
ls -l /dev/dri
for status in /sys/class/drm/card*-*/status; do
    printf '%s: %s\n' "$status" "$(cat "$status")"
done
```

If it is not `/dev/dri/card0`, pass the correct node to `install.sh`. The
installer also adds the node's numeric group to the container so its non-root
user can open it. To change cards later, rerun the installer with the new path,
update `HYPERION_DRM_DEVICE` in `/etc/hyperion-capture/podman.env`, and run
`sudo systemctl daemon-reload`.

## Security model

This cannot be a rootless container. Linux only returns existing framebuffer
GEM handles to DRM master or a process with `CAP_SYS_ADMIN` in the host user
namespace. The system Quadlet therefore uses `UserNS=host`, but still limits
the service in several ways:

- the image runs as the non-root UID/GID 65532;
- only the configured DRM primary node is exposed;
- only that node's owning group is added to the process;
- every container capability except `CAP_SYS_ADMIN` is dropped;
- the root filesystem is read-only and the process count is limited; and
- Hyperion Capture drops effective `CAP_SYS_ADMIN` between framebuffer queries.

The executable carries `cap_sys_admin=ep`, so `NoNewPrivileges=true` must not be
added: that option prevents file capabilities from taking effect. Do not replace
the selected device with `--privileged` or expose all host devices.

## Build locally

From the repository root:

```sh
sudo podman build \
    --file packaging/podman/Containerfile \
    --tag localhost/hyperion-capture:dev \
    .

sudo podman run --rm \
    --userns=host \
    --device=/dev/dri/card0:/dev/dri/card0:rw \
    --group-add="$(stat -c '%g' /dev/dri/card0)" \
    --cap-drop=all \
    --cap-add=SYS_ADMIN \
    --env-file=packaging/podman/podman.env.example \
    localhost/hyperion-capture:dev
```

The image contains Debian's Mesa GBM/EGL/GLES runtime. Validate real scanout
modifiers, HDR modes, external monitors, and Gamescope/desktop transitions on
the target hardware; ordinary container builds cannot emulate those paths.

On an SELinux-enforcing host, a device-label denial may require a local policy
allowing containers to use DRM devices. Keep label separation enabled where
possible; `SecurityLabelDisable=true` is not required on SteamOS and is not part
of the supplied Quadlet.
