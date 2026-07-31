#!/usr/bin/env bash
set -euo pipefail

# Host-side lifecycle helper for the reproducible Tart DRM test guest.
# Override these task-specific variables when maintaining more than one image.
vm_name="${HYPERION_TART_VM:-hyperion-capture-ubuntu}"
base_image="${HYPERION_TART_BASE:-ghcr.io/cirruslabs/ubuntu:24.04}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

usage() {
    cat <<EOF
Usage: $0 <build|start|provision|test|shell|stop|delete|status>

Environment:
  HYPERION_TART_VM    local VM name (default: ${vm_name})
  HYPERION_TART_BASE  source image (default: ${base_image})

The project is mounted read-only at /mnt/shared/project. Guest build output is
kept under /var/tmp so tests never modify the host checkout.
EOF
}

require_tart() {
    if ! command -v tart >/dev/null 2>&1; then
        echo "error: Tart is not installed; see https://tart.run/quick-start/" >&2
        exit 1
    fi
}

vm_exists() {
    tart get "${vm_name}" >/dev/null 2>&1
}

vm_running() {
    tart exec "${vm_name}" true >/dev/null 2>&1
}

require_running() {
    if ! vm_exists; then
        echo "error: VM ${vm_name} does not exist; run '$0 build' first" >&2
        exit 1
    fi
    if ! vm_running; then
        echo "error: VM ${vm_name} is not running; keep '$0 start' open in another terminal" >&2
        exit 1
    fi
}

provision_vm() {
    require_running
    tart exec -i "${vm_name}" sudo bash -s \
        <"${repo_root}/vm/tart/provision-ubuntu.sh"
}

require_tart
action="${1:-}"

case "${action}" in
build)
    if vm_exists; then
        echo "error: VM ${vm_name} already exists; delete or choose another name" >&2
        exit 1
    fi
    tart clone "${base_image}" "${vm_name}"
    tart set "${vm_name}" \
        --cpu 4 \
        --memory 8192 \
        --disk-size 40 \
        --display 1920x1080px \
        --no-display-refit
    echo "Created ${vm_name}. Keep '$0 start' running, then run '$0 provision' in another terminal."
    ;;
start)
    if ! vm_exists; then
        echo "error: VM ${vm_name} does not exist; run '$0 build' first" >&2
        exit 1
    fi
    if vm_running; then
        echo "error: VM ${vm_name} is already running" >&2
        exit 1
    fi
    # `tart run` owns the VM. Deliberately remain in the foreground: exiting or
    # killing this process stops the VM. The normal window is part of the KMS
    # test environment, so do not use --no-graphics.
    exec tart run \
        --dir="project:${repo_root}:ro" \
        "${vm_name}"
    ;;
provision)
    provision_vm
    ;;
test)
    require_running
    # Stream the harness because Linux does not mount virtiofs until the
    # harness requests it, so its checked-in path is not initially visible.
    tart exec -i "${vm_name}" bash -s /mnt/shared/project \
        <"${repo_root}/vm/tart/smoke.sh"
    ;;
shell)
    require_running
    tart exec -t "${vm_name}" bash -l
    ;;
stop)
    tart stop "${vm_name}"
    ;;
delete)
    tart delete "${vm_name}"
    ;;
status)
    tart get "${vm_name}"
    ;;
*)
    usage >&2
    exit 2
    ;;
esac
