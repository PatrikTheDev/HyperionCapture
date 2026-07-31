# kms-egl

This is the workspace's deliberately isolated low-level graphics crate. It is
the only crate allowed to contain unsafe Rust. Its safe API imports DRM
framebuffer DMA-BUF planes with `EGL_EXT_image_dma_buf_import`, binds the image
to an OpenGL texture, and reads normalized RGBA pixels back through OpenGL.

Safety invariants:

- the crate owns the duplicated DRM file descriptor for at least as long as
  its GBM and EGL objects;
- every imported plane borrows a live file descriptor for the complete EGL
  image lifetime;
- EGL/GL handles are checked before use and destroyed in reverse order;
- attribute arrays are terminated with `EGL_NONE` and contain at most four
  planes;
- output allocations are overflow-checked before native code writes to them;
- the reader is deliberately neither `Send` nor `Sync`, keeping its EGL context
  and OpenGL state on the creating thread.

No capture policy, connector selection, capability handling, or transport code
belongs here.
