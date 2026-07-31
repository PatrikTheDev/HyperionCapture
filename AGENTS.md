# Contributor guidance

## Scope

This is a Rust 2024 workspace. Keep the executable focused on orchestration and
put reusable Hyperion protocol behavior in `crates/hyperion-client`.

## Architecture

- Keep capture backends behind `capture::CaptureSource`.
- The production Linux path is direct DRM/KMS. Do not add X11, portal, or
  compositor-specific behavior as a silent fallback.
- Re-enumeration after DRM modesets is required; Gamescope and monitor changes
  must not require restarting the service.
- Avoid unnecessary full-frame copies. Document any unavoidable format
  conversion at the boundary where it occurs.
- Never log API tokens or include them in error messages.

## Workflow

Run `just check` before handing off changes. Use `just format` to apply Rust
formatting. Add unit tests for protocol serialization and capture-independent
logic; gate hardware tests so ordinary CI does not require `/dev/dri`.

Unsafe Rust is forbidden at the workspace level. If a future DRM/GBM binding
requires unsafe code, isolate it in a dedicated low-level crate, document every
safety invariant, and change the lint only for that crate after review.

