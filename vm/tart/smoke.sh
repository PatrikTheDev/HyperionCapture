#!/usr/bin/env bash
set -euo pipefail

project_dir="${1:-/mnt/shared/project}"
readonly target_dir="/var/tmp/hyperion-capture-target"

if [[ ! -d "${project_dir}" ]]; then
    sudo install -d /mnt/shared
    sudo mount -t virtiofs com.apple.virtio-fs.automount /mnt/shared
fi
if [[ ! -f "${project_dir}/Cargo.toml" ]]; then
    echo "error: project is not mounted at ${project_dir}" >&2
    exit 1
fi

export PATH="/home/admin/.cargo/bin:${PATH}"
export CARGO_TARGET_DIR="${target_dir}"

echo "== guest =="
uname -a
rustc --version
cargo --version

echo "== DRM nodes =="
if [[ ! -d /dev/dri ]]; then
    echo "error: Tart guest has no /dev/dri; start it with a configured display" >&2
    exit 1
fi
ls -la /dev/dri

mapfile -t cards < <(find /dev/dri -maxdepth 1 -type c -name 'card*' | sort)
if (( ${#cards[@]} == 0 )); then
    echo "error: no DRM primary node found" >&2
    exit 1
fi

connected=0
for status_file in /sys/class/drm/card*-*/status; do
    [[ -e "${status_file}" ]] || continue
    printf '%s: ' "${status_file%/status}"
    cat "${status_file}"
    if grep -qx connected "${status_file}"; then
        connected=1
    fi
done
if (( connected == 0 )); then
    echo "error: no connected DRM connector; Tart's virtual display is inactive" >&2
    exit 1
fi

echo "== DRM resources =="
sudo drm_info
# Tart exposes a single DRM card. Let modetest use libdrm's default-device
# discovery: `-D /dev/dri/card0` is treated as a bus ID by Ubuntu's build, and
# the PCI module name (virtio_pci) differs from the DRM driver (virtio_gpu).
sudo modetest -c -e -p

echo "== kernel atomic state =="
mountpoint -q /sys/kernel/debug || sudo mount -t debugfs debugfs /sys/kernel/debug
state_files=(/sys/kernel/debug/dri/*/state)
if [[ -e "${state_files[0]}" ]]; then
    for state_file in "${state_files[@]}"; do
        echo "-- ${state_file} --"
        sudo cat "${state_file}"
    done
else
    echo "warning: kernel does not expose DRM atomic state through debugfs" >&2
fi

echo "== Rust quality gates =="
cd "${project_dir}"
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features

echo "== live KMS capture probe =="
capture_log="$(mktemp /var/tmp/hyperion-kms-smoke.XXXXXX.log)"
trap 'rm -f "${capture_log}"' EXIT
# Root is confined to the hardware probe; project compilation above runs as
# the unprivileged Tart user.
set +e
# The application installs a Ctrl-C handler for interactive use. SIGTERM keeps
# this bounded non-interactive probe independent of terminal signal delivery.
sudo timeout --signal=TERM --kill-after=2 8 \
    "${target_dir}/debug/hyperion-capture" \
    --capture kms \
    --drm-device "${cards[0]}" \
    --hyperion-url http://127.0.0.1:9/ \
    --fps 5 >"${capture_log}" 2>&1
capture_status=$?
set -e
cat "${capture_log}"

if (( capture_status != 124 && capture_status != 130 )); then
    echo "error: KMS backend exited during the live capture probe (status ${capture_status})" >&2
    exit 1
fi

if ! grep -Eq 'width.*854.*height.*480' "${capture_log}"; then
    echo "error: live KMS probe did not report the expected 854x480 GPU-scaled output" >&2
    exit 1
fi

echo "KMS backend produced 854x480 frames for the 8-second capture window."
