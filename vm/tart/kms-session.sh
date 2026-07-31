#!/usr/bin/env bash
set -euo pipefail

# Optional destructive-to-the-session test: replace the guest desktop with a
# tiny pixman-rendered Weston DRM session, then restore the display manager.
# Run as root from a Tart console, not over an SSH session you need to preserve.
runtime_dir="$(mktemp -d /run/hyperion-weston.XXXXXX)"
display_manager_was_active=0

cleanup() {
    rm -rf "${runtime_dir}"
    if (( display_manager_was_active == 1 )); then
        systemctl start display-manager.service
    fi
}
trap cleanup EXIT

if systemctl is-active --quiet display-manager.service; then
    display_manager_was_active=1
    systemctl stop display-manager.service
fi

chmod 0700 "${runtime_dir}"
export XDG_RUNTIME_DIR="${runtime_dir}"
export LIBSEAT_BACKEND=seatd

echo "Starting Weston on the DRM backend with software rendering."
echo "In another terminal, run vm/tart/smoke.sh to exercise capture."
weston \
    --backend=drm-backend.so \
    --renderer=pixman \
    --continue-without-input \
    --idle-time=0
