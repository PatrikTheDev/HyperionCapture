# Tart DRM/KMS test guest

This directory defines a reproducible Ubuntu 24.04 ARM64 guest for testing the
Linux-only portions of Hyperion Capture from an Apple Silicon development Mac.
It follows Tart's official image, directory-sharing, and guest-execution model.

## Lifecycle

The host helper defaults to the official
`ghcr.io/cirruslabs/ubuntu:24.04` image and creates a local
`hyperion-capture-ubuntu` VM with four CPUs, 8 GiB RAM, a 40 GiB disk, and a
1920x1080 virtual display:

```sh
vm/tart/manage.sh build
vm/tart/manage.sh start       # terminal 1; remains in the foreground
vm/tart/manage.sh provision   # terminal 2; first build only
vm/tart/manage.sh test        # terminal 2
vm/tart/manage.sh stop        # terminal 2, or close the VM normally
```

`tart run` owns the VM process, so `start` intentionally stays in the foreground
and opens Tart's normal display window on macOS. Run provisioning and tests from
a second terminal. Stopping or killing `start` stops the guest. Do not add
`--no-graphics`: headless mode is not representative of the active virtual KMS
display and caused guest processes to crash on the development host.

`build` downloads the base image on first use and expands its virtual disk to
40 GiB; run `provision` once after starting the new VM. Actual downloaded and
allocated space depends on Tart's OCI cache and APFS copy-on-write state. It
does not push an image. The source tree is shared
read-only through virtiofs at
`/mnt/shared/project`; Cargo output lives on the guest disk under `/var/tmp`,
so a guest build cannot populate or alter the host checkout. Set
`HYPERION_TART_VM` to keep multiple local variants or `HYPERION_TART_BASE` to
test another compatible Ubuntu image.

The provisioner is deliberately rerunnable. It installs Rust 1.85.0 (the
workspace minimum), Clippy, rustfmt, C build dependencies, libdrm/GBM/EGL
headers, `modetest`, `kmscube`, Mesa diagnostics, seatd, and Weston. The normal
test command verifies:

1. a DRM primary node and connected connector exist;
2. connectors, encoders, CRTCs, formats, and planes can be enumerated;
3. the kernel's current atomic DRM state can be inspected;
4. formatting, Clippy, unit tests, and the Linux build pass; and
5. the real KMS backend remains alive while capturing for eight seconds.

The Hyperion URL in the final probe is intentionally an unused local port.
Transport failures are expected and keep the probe independent of a Hyperion
deployment; an early capture-process exit is treated as failure. Only this
short hardware probe uses `sudo`, since exporting a framebuffer owned by the
active KMS client can require elevated DRM privileges. Compilation and ordinary
tests run as the unprivileged `admin` user. Production capability/device policy
must be tested separately on native Linux and should not run the full service
as root.

## Modeset and re-enumeration exercise

The official Ubuntu image normally leaves the virtual DRM card on its kernel
console. For a small, deterministic compositor session, open a second Tart
console and run:

```sh
sudo /mnt/shared/project/vm/tart/kms-session.sh
```

This stops a display manager if one was added to the guest, starts Weston
directly on KMS using its pixman (software) renderer, and restores the display
manager on exit. In another terminal, run `vm/tart/manage.sh test`. Starting
and stopping Weston supplies compositor teardown/startup and KMS modeset events
without introducing X11- or desktop-specific capture logic.

## What Tart proves—and what it cannot

Tart uses Apple's Virtualization.framework and presents a virtual display to
Linux. That makes the guest valuable for build coverage, DRM object discovery,
permissions, software-backed scanout formats, active-plane lookup, and
re-enumeration after a virtual modeset. It is not a substitute for the target
machine:

- the virtual display is not a physical AMD or Intel GPU;
- it does not reproduce SteamOS's Gamescope session handoff;
- it cannot cover vendor-specific modifiers, tiled/multi-plane framebuffers,
  HDR, rotation, or DMA-BUF import behavior; and
- successful virtual-card capture does not prove physical-card capture.

Gamescope is intentionally not installed. Gamescope composites with Vulkan and
expects a Vulkan-capable DRM device with format-modifier support. Tart does not
pass through an AMD/Intel GPU, so installing Gamescope would not provide a
credible Game Mode environment even if a software Vulkan implementation let a
subset of startup proceed.

Use the following test tiers before considering the capture backend complete:

1. ordinary macOS/CI unit tests for capture-independent logic, plus Linux unit
   tests for validated EGL DMA-BUF plane/modifier attributes;
2. this Tart guest for Linux compilation and virtual DRM integration;
3. native Linux with an AMD or Intel GPU for DMA-BUF/GBM/EGL import and format
   conversion; and
4. a SteamOS device for repeated Game Mode to KDE Plasma transitions while the
   capture service remains running.

If a shareable image is desired later, stop the provisioned VM and use Tart's
documented OCI workflow, for example `tart push <local-name>
ghcr.io/<owner>/<image>:<tag>`. Registry selection, authentication, and pushing
are intentionally left for explicit coordination.

References:

- <https://tart.run/quick-start/>
- <https://github.com/ValveSoftware/gamescope>
