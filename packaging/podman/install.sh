#!/bin/sh
set -eu

if [ "$(id -u)" -ne 0 ]; then
    echo "run this installer as root" >&2
    exit 1
fi

for command in podman systemctl stat sed; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "required command is not installed: $command" >&2
        exit 1
    fi
done

cgroups_version=$(podman info --format '{{.Host.CgroupsVersion}}')
case "$cgroups_version" in
    2|v2) ;;
    *)
        echo "Podman Quadlet requires cgroup v2; found $cgroups_version" >&2
        exit 1
        ;;
esac

package_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
config_dir=/etc/hyperion-capture
quadlet_dir=/etc/containers/systemd
config_path="$config_dir/podman.env"
drm_device=${1:-/dev/dri/card0}

case "$drm_device" in
    /dev/dri/card*) drm_number=${drm_device#/dev/dri/card} ;;
    *) drm_number= ;;
esac
case "$drm_number" in
    ''|*[!0-9]*)
        echo "DRM device must look like /dev/dri/card0" >&2
        exit 1
        ;;
esac
if [ ! -c "$drm_device" ]; then
    echo "DRM device does not exist or is not a character device: $drm_device" >&2
    exit 1
fi

drm_gid=$(stat -c '%g' "$drm_device")
temporary=$(mktemp)
trap 'rm -f -- "$temporary"' EXIT INT TERM
sed \
    -e "s|^AddDevice=.*|AddDevice=$drm_device:$drm_device:rw|" \
    -e "s|^GroupAdd=.*|GroupAdd=$drm_gid|" \
    "$package_dir/hyperion-capture.container" > "$temporary"

install -d -m 0755 "$config_dir" "$quadlet_dir"
install -m 0644 \
    "$temporary" \
    "$quadlet_dir/hyperion-capture.container"

if [ ! -e "$config_path" ]; then
    install -m 0600 "$package_dir/podman.env.example" "$config_path"
    sed -i "s|^HYPERION_DRM_DEVICE=.*|HYPERION_DRM_DEVICE=$drm_device|" "$config_path"
    echo "created $config_path"
else
    echo "preserved existing $config_path"
fi

systemctl daemon-reload
if ! systemctl cat hyperion-capture.service >/dev/null 2>&1; then
    echo "Podman's Quadlet generator did not create hyperion-capture.service" >&2
    echo "verify that this Podman version includes system Quadlet support" >&2
    exit 1
fi

cat <<EOF
Installed the Hyperion Capture system Quadlet.

1. Edit $config_path.
2. The service is configured for $drm_device (supplementary GID $drm_gid).
3. Run: systemctl daemon-reload && systemctl start hyperion-capture.service
EOF
