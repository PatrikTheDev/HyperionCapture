#!/usr/bin/env bash
set -euo pipefail

# This script runs as root inside the official Ubuntu 24.04 Tart guest.
# Keep versions explicit where the project depends on them.
readonly rust_toolchain="1.85.0"
readonly guest_user="${SUDO_USER:-admin}"

if ! command -v apt-get >/dev/null 2>&1; then
    echo "error: this provisioner requires an Ubuntu/Debian guest" >&2
    exit 1
fi

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install --yes --no-install-recommends \
    build-essential \
    ca-certificates \
    clang \
    cmake \
    curl \
    drm-info \
    git \
    jq \
    kmscube \
    libdrm-dev \
    libdrm-tests \
    libegl1-mesa-dev \
    libgbm-dev \
    libgl1-mesa-dri \
    libgl1-mesa-dev \
    libseat1 \
    mesa-utils \
    mesa-vulkan-drivers \
    pkg-config \
    seatd \
    systemd \
    udev \
    vulkan-tools \
    weston

if ! id "${guest_user}" >/dev/null 2>&1; then
    echo "error: expected Tart guest user ${guest_user} does not exist" >&2
    exit 1
fi

# Device access applies on the user's next login/tart exec session.
usermod --append --groups video,render "${guest_user}"
systemctl enable --now seatd.service

if [[ ! -x "/home/${guest_user}/.cargo/bin/rustup" ]]; then
    sudo -u "${guest_user}" --set-home \
        sh -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none'
fi

sudo -u "${guest_user}" --set-home \
    "/home/${guest_user}/.cargo/bin/rustup" toolchain install \
    "${rust_toolchain}" --profile minimal --component clippy --component rustfmt
sudo -u "${guest_user}" --set-home \
    "/home/${guest_user}/.cargo/bin/rustup" default "${rust_toolchain}"

install -d -o "${guest_user}" -g "${guest_user}" /var/tmp/hyperion-capture-target

cat >/etc/profile.d/hyperion-capture-rust.sh <<'EOF'
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR=/var/tmp/hyperion-capture-target
EOF
chmod 0644 /etc/profile.d/hyperion-capture-rust.sh

echo "Provisioned Ubuntu Tart guest with Rust ${rust_toolchain}."
echo "Gamescope was intentionally omitted; see vm/tart/TESTING.md."
