use std::{
    error::Error as StdError,
    ffi::{CStr, c_char, c_int, c_uint, c_void},
    fmt,
    marker::PhantomData,
    mem::size_of,
    os::fd::{AsRawFd, BorrowedFd, OwnedFd},
    ptr,
    rc::Rc,
};

type EglBoolean = c_uint;
type EglDisplay = *mut c_void;
type EglConfig = *mut c_void;
type EglContext = *mut c_void;
type EglSurface = *mut c_void;
type EglImage = *mut c_void;
type EglClientBuffer = *mut c_void;
type EglInt = c_int;
type GbmDevice = c_void;

const EGL_FALSE: EglBoolean = 0;
const EGL_NONE: EglInt = 0x3038;
const EGL_EXTENSIONS: EglInt = 0x3055;
const EGL_SURFACE_TYPE: EglInt = 0x3033;
const EGL_PBUFFER_BIT: EglInt = 0x0001;
const EGL_RENDERABLE_TYPE: EglInt = 0x3040;
const EGL_OPENGL_ES2_BIT: EglInt = 0x0004;
const EGL_WIDTH: EglInt = 0x3057;
const EGL_HEIGHT: EglInt = 0x3056;
const EGL_CONTEXT_CLIENT_VERSION: EglInt = 0x3098;
const EGL_OPENGL_ES_API: c_uint = 0x30A0;
const EGL_LINUX_DMA_BUF_EXT: EglInt = 0x3270;
const EGL_LINUX_DRM_FOURCC_EXT: EglInt = 0x3271;
const EGL_DMA_BUF_PLANE0_FD_EXT: EglInt = 0x3272;
const EGL_DMA_BUF_PLANE0_OFFSET_EXT: EglInt = 0x3273;
const EGL_DMA_BUF_PLANE0_PITCH_EXT: EglInt = 0x3274;
const EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT: EglInt = 0x3443;
const EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT: EglInt = 0x3444;

const GL_TEXTURE_2D: c_uint = 0x0DE1;
const GL_TEXTURE0: c_uint = 0x84C0;
const GL_TEXTURE_MIN_FILTER: c_uint = 0x2801;
const GL_TEXTURE_MAG_FILTER: c_uint = 0x2800;
const GL_TEXTURE_WRAP_S: c_uint = 0x2802;
const GL_TEXTURE_WRAP_T: c_uint = 0x2803;
const GL_LINEAR: c_int = 0x2601;
const GL_CLAMP_TO_EDGE: c_int = 0x812F;
const GL_RGBA: c_uint = 0x1908;
const GL_RGBA_I: c_int = 0x1908;
const GL_UNSIGNED_BYTE: c_uint = 0x1401;
const GL_FLOAT: c_uint = 0x1406;
const GL_FALSE: u8 = 0;
const GL_NO_ERROR: c_uint = 0;
const GL_VERTEX_SHADER: c_uint = 0x8B31;
const GL_FRAGMENT_SHADER: c_uint = 0x8B30;
const GL_COMPILE_STATUS: c_uint = 0x8B81;
const GL_LINK_STATUS: c_uint = 0x8B82;
const GL_INFO_LOG_LENGTH: c_uint = 0x8B84;
const GL_FRAMEBUFFER: c_uint = 0x8D40;
const GL_COLOR_ATTACHMENT0: c_uint = 0x8CE0;
const GL_FRAMEBUFFER_COMPLETE: c_uint = 0x8CD5;
const GL_TRIANGLE_STRIP: c_uint = 0x0005;

type CreateImage = unsafe extern "C" fn(
    EglDisplay,
    EglContext,
    EglInt,
    EglClientBuffer,
    *const EglInt,
) -> EglImage;
type DestroyImage = unsafe extern "C" fn(EglDisplay, EglImage) -> EglBoolean;
type ImageTargetTexture = unsafe extern "C" fn(c_uint, *const c_void);

#[link(name = "gbm")]
unsafe extern "C" {
    fn gbm_create_device(fd: c_int) -> *mut GbmDevice;
    fn gbm_device_destroy(device: *mut GbmDevice);
}

#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetDisplay(native_display: *mut c_void) -> EglDisplay;
    fn eglInitialize(display: EglDisplay, major: *mut EglInt, minor: *mut EglInt) -> EglBoolean;
    fn eglTerminate(display: EglDisplay) -> EglBoolean;
    fn eglBindAPI(api: c_uint) -> EglBoolean;
    fn eglChooseConfig(
        display: EglDisplay,
        attributes: *const EglInt,
        configs: *mut EglConfig,
        config_size: EglInt,
        count: *mut EglInt,
    ) -> EglBoolean;
    fn eglCreatePbufferSurface(
        display: EglDisplay,
        config: EglConfig,
        attributes: *const EglInt,
    ) -> EglSurface;
    fn eglDestroySurface(display: EglDisplay, surface: EglSurface) -> EglBoolean;
    fn eglCreateContext(
        display: EglDisplay,
        config: EglConfig,
        share: EglContext,
        attributes: *const EglInt,
    ) -> EglContext;
    fn eglDestroyContext(display: EglDisplay, context: EglContext) -> EglBoolean;
    fn eglMakeCurrent(
        display: EglDisplay,
        draw: EglSurface,
        read: EglSurface,
        context: EglContext,
    ) -> EglBoolean;
    fn eglQueryString(display: EglDisplay, name: EglInt) -> *const c_char;
    fn eglGetProcAddress(name: *const c_char) -> *const c_void;
    fn eglGetError() -> EglInt;
}

#[link(name = "GLESv2")]
unsafe extern "C" {
    fn glGenTextures(count: c_int, textures: *mut c_uint);
    fn glDeleteTextures(count: c_int, textures: *const c_uint);
    fn glBindTexture(target: c_uint, texture: c_uint);
    fn glTexParameteri(target: c_uint, name: c_uint, value: c_int);
    fn glTexImage2D(
        target: c_uint,
        level: c_int,
        internal_format: c_int,
        width: c_int,
        height: c_int,
        border: c_int,
        format: c_uint,
        kind: c_uint,
        pixels: *const c_void,
    );
    fn glActiveTexture(texture: c_uint);
    fn glCreateShader(kind: c_uint) -> c_uint;
    fn glShaderSource(
        shader: c_uint,
        count: c_int,
        strings: *const *const c_char,
        lengths: *const c_int,
    );
    fn glCompileShader(shader: c_uint);
    fn glGetShaderiv(shader: c_uint, name: c_uint, value: *mut c_int);
    fn glGetShaderInfoLog(shader: c_uint, capacity: c_int, length: *mut c_int, log: *mut c_char);
    fn glDeleteShader(shader: c_uint);
    fn glCreateProgram() -> c_uint;
    fn glAttachShader(program: c_uint, shader: c_uint);
    fn glBindAttribLocation(program: c_uint, index: c_uint, name: *const c_char);
    fn glLinkProgram(program: c_uint);
    fn glGetProgramiv(program: c_uint, name: c_uint, value: *mut c_int);
    fn glGetProgramInfoLog(program: c_uint, capacity: c_int, length: *mut c_int, log: *mut c_char);
    fn glDeleteProgram(program: c_uint);
    fn glUseProgram(program: c_uint);
    fn glGetUniformLocation(program: c_uint, name: *const c_char) -> c_int;
    fn glUniform1i(location: c_int, value: c_int);
    fn glUniform1f(location: c_int, value: f32);
    fn glUniform2f(location: c_int, x: f32, y: f32);
    fn glEnableVertexAttribArray(index: c_uint);
    fn glDisableVertexAttribArray(index: c_uint);
    fn glVertexAttribPointer(
        index: c_uint,
        size: c_int,
        kind: c_uint,
        normalized: u8,
        stride: c_int,
        pointer: *const c_void,
    );
    fn glGenFramebuffers(count: c_int, framebuffers: *mut c_uint);
    fn glDeleteFramebuffers(count: c_int, framebuffers: *const c_uint);
    fn glBindFramebuffer(target: c_uint, framebuffer: c_uint);
    fn glFramebufferTexture2D(
        target: c_uint,
        attachment: c_uint,
        texture_target: c_uint,
        texture: c_uint,
        level: c_int,
    );
    fn glCheckFramebufferStatus(target: c_uint) -> c_uint;
    fn glViewport(x: c_int, y: c_int, width: c_int, height: c_int);
    fn glDrawArrays(mode: c_uint, first: c_int, count: c_int);
    fn glReadPixels(
        x: c_int,
        y: c_int,
        width: c_int,
        height: c_int,
        format: c_uint,
        kind: c_uint,
        pixels: *mut c_void,
    );
    fn glGetError() -> c_uint;
}

const POSITION_ATTRIBUTE: c_uint = 0;
const TEX_COORD_ATTRIBUTE: c_uint = 1;

const VERTEX_SHADER: &[u8] = br"
attribute vec2 a_position;
attribute vec2 a_tex_coord;
varying vec2 v_tex_coord;

void main() {
    gl_Position = vec4(a_position, 0.0, 1.0);
    v_tex_coord = a_tex_coord;
}
";

const FRAGMENT_SHADER: &[u8] = br"
precision highp float;

uniform sampler2D u_source;
uniform vec2 u_sample_offset;
uniform float u_transfer;
varying vec2 v_tex_coord;

vec3 srgb_to_linear(vec3 value) {
    vec3 low = value / 12.92;
    vec3 high = pow((value + 0.055) / 1.055, vec3(2.4));
    return mix(low, high, step(vec3(0.04045), value));
}

vec3 linear_to_srgb(vec3 value) {
    value = max(value, vec3(0.0));
    vec3 low = value * 12.92;
    vec3 high = 1.055 * pow(value, vec3(1.0 / 2.4)) - 0.055;
    return mix(low, high, step(vec3(0.0031308), value));
}

vec3 pq_to_nits(vec3 value) {
    const float m1 = 2610.0 / 16384.0;
    const float m2 = 2523.0 / 32.0;
    const float c1 = 3424.0 / 4096.0;
    const float c2 = 2413.0 / 128.0;
    const float c3 = 2392.0 / 128.0;
    vec3 power = pow(max(value, vec3(0.0)), vec3(1.0 / m2));
    vec3 linear = pow(max(power - c1, vec3(0.0)) / max(c2 - c3 * power, vec3(0.000001)), vec3(1.0 / m1));
    return linear * 10000.0;
}

vec3 hlg_to_linear(vec3 value) {
    const float a = 0.17883277;
    const float b = 0.28466892;
    const float c = 0.55991073;
    vec3 low = value * value / 3.0;
    vec3 high = (exp((value - c) / a) + b) / 12.0;
    return mix(low, high, step(vec3(0.5), value));
}

vec3 bt2020_to_bt709(vec3 value) {
    return mat3(
         1.660491, -0.124550, -0.018151,
        -0.587641,  1.132900, -0.100579,
        -0.072850, -0.008349,  1.118730
    ) * value;
}

vec3 aces_fitted(vec3 value) {
    const float a = 2.51;
    const float b = 0.03;
    const float c = 2.43;
    const float d = 0.59;
    const float e = 0.14;
    return clamp((value * (a * value + b)) / (value * (c * value + d) + e), 0.0, 1.0);
}

vec3 decode(vec3 value) {
    if (u_transfer < 0.5) {
        return srgb_to_linear(value);
    }
    if (u_transfer < 1.5) {
        return bt2020_to_bt709(pq_to_nits(value) / 203.0);
    }
    return bt2020_to_bt709(hlg_to_linear(value) * (1000.0 / 203.0));
}

void main() {
    vec2 offset = u_sample_offset;
    vec3 linear = decode(texture2D(u_source, v_tex_coord + vec2(-offset.x, -offset.y)).rgb);
    linear += decode(texture2D(u_source, v_tex_coord + vec2( offset.x, -offset.y)).rgb);
    linear += decode(texture2D(u_source, v_tex_coord + vec2(-offset.x,  offset.y)).rgb);
    linear += decode(texture2D(u_source, v_tex_coord + vec2( offset.x,  offset.y)).rgb);
    linear *= 0.25;
    if (u_transfer >= 0.5) {
        linear = aces_fitted(max(linear, vec3(0.0)));
    }
    gl_FragColor = vec4(linear_to_srgb(linear), 1.0);
}
";

/// One DMA-BUF plane belonging to a DRM framebuffer.
#[derive(Debug)]
pub struct DmaBufPlane<'a> {
    /// Live DMA-BUF file descriptor.
    pub fd: BorrowedFd<'a>,
    /// Byte offset of this plane in the DMA-BUF.
    pub offset: u32,
    /// Number of bytes between adjacent rows.
    pub pitch: u32,
    /// DRM format modifier, or `None` when the kernel did not report one.
    pub modifier: Option<u64>,
}

/// DMA-BUF metadata required by `EGL_EXT_image_dma_buf_import`.
#[derive(Debug)]
pub struct DmaBufFrame<'a> {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// DRM `FourCC` value.
    pub fourcc: u32,
    /// One to four DMA-BUF planes.
    pub planes: &'a [DmaBufPlane<'a>],
}

/// Transfer function used by the scanout framebuffer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TransferFunction {
    /// Standard dynamic range encoded with the sRGB transfer function.
    #[default]
    Srgb,
    /// SMPTE ST 2084 (PQ), normally with BT.2020 primaries.
    Pq,
    /// BT.2100 Hybrid Log-Gamma, normally with BT.2020 primaries.
    Hlg,
}

impl TransferFunction {
    const fn shader_value(self) -> f32 {
        match self {
            Self::Srgb => 0.0,
            Self::Pq => 1.0,
            Self::Hlg => 2.0,
        }
    }
}

/// GPU conversion requested for one framebuffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadbackOptions {
    /// Maximum output height. Images smaller than this are not enlarged.
    pub max_height: u32,
    /// Transfer function advertised by the active KMS connector.
    pub transfer_function: TransferFunction,
}

/// Packed RGB8 pixels produced by the GPU conversion pass.
#[derive(Debug)]
pub struct RgbFrame {
    /// Output width after aspect-preserving scaling.
    pub width: u32,
    /// Output height after aspect-preserving scaling.
    pub height: u32,
    /// Top-down, packed RGB8 pixels.
    pub pixels: Vec<u8>,
}

/// EGL/OpenGL initialization or readback failure.
#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl StdError for Error {}

/// Thread-affine EGL/OpenGL reader associated with one DRM device.
pub struct Reader {
    _drm_fd: OwnedFd,
    gbm: *mut GbmDevice,
    display: EglDisplay,
    context: EglContext,
    surface: EglSurface,
    create_image: CreateImage,
    destroy_image: DestroyImage,
    image_target_texture: ImageTargetTexture,
    supports_modifiers: bool,
    program: c_uint,
    source_uniform: c_int,
    sample_offset_uniform: c_int,
    transfer_uniform: c_int,
    output: Option<OutputTarget>,
    _thread_affine: PhantomData<Rc<()>>,
}

struct OutputTarget {
    width: u32,
    height: u32,
    framebuffer: c_uint,
    texture: c_uint,
    rgba: Vec<u8>,
}

impl fmt::Debug for Reader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Reader")
            .field("display", &self.display)
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl Reader {
    /// Creates GBM, EGL, and OpenGL ES 2 state for a duplicated DRM FD.
    ///
    /// # Errors
    ///
    /// Returns an error when GBM/EGL initialization or a required DMA-BUF
    /// import extension is unavailable.
    pub fn new(drm_fd: OwnedFd) -> Result<Self, Error> {
        // SAFETY: `drm_fd` remains owned by the returned Reader. Each handle is
        // checked before use and partial initialization is unwound explicitly.
        unsafe {
            let gbm = gbm_create_device(drm_fd.as_raw_fd());
            if gbm.is_null() {
                return Err(Error("gbm_create_device failed".to_owned()));
            }
            let display = eglGetDisplay(gbm.cast());
            if display.is_null() {
                gbm_device_destroy(gbm);
                return Err(last_egl_error("eglGetDisplay failed"));
            }
            if eglInitialize(display, ptr::null_mut(), ptr::null_mut()) == EGL_FALSE {
                gbm_device_destroy(gbm);
                return Err(last_egl_error("eglInitialize failed"));
            }
            if eglBindAPI(EGL_OPENGL_ES_API) == EGL_FALSE {
                eglTerminate(display);
                gbm_device_destroy(gbm);
                return Err(last_egl_error("EGL does not expose OpenGL ES"));
            }

            let (context, surface) = match create_context(display) {
                Ok(state) => state,
                Err(error) => {
                    eglTerminate(display);
                    gbm_device_destroy(gbm);
                    return Err(error);
                }
            };

            let extensions = required_extensions(display);
            let (create_image, destroy_image, image_target_texture, supports_modifiers) =
                match extensions {
                    Ok(extensions) => extensions,
                    Err(error) => {
                        eglMakeCurrent(display, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
                        eglDestroyContext(display, context);
                        if !surface.is_null() {
                            eglDestroySurface(display, surface);
                        }
                        eglTerminate(display);
                        gbm_device_destroy(gbm);
                        return Err(error);
                    }
                };

            let program = match create_conversion_program() {
                Ok(program) => program,
                Err(error) => {
                    eglMakeCurrent(display, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
                    eglDestroyContext(display, context);
                    if !surface.is_null() {
                        eglDestroySurface(display, surface);
                    }
                    eglTerminate(display);
                    gbm_device_destroy(gbm);
                    return Err(error);
                }
            };
            let source_uniform = glGetUniformLocation(program, c"u_source".as_ptr());
            let sample_offset_uniform = glGetUniformLocation(program, c"u_sample_offset".as_ptr());
            let transfer_uniform = glGetUniformLocation(program, c"u_transfer".as_ptr());
            if source_uniform < 0 || sample_offset_uniform < 0 || transfer_uniform < 0 {
                glDeleteProgram(program);
                eglMakeCurrent(display, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
                eglDestroyContext(display, context);
                if !surface.is_null() {
                    eglDestroySurface(display, surface);
                }
                eglTerminate(display);
                gbm_device_destroy(gbm);
                return Err(Error(
                    "GPU conversion shader does not expose its required uniforms".to_owned(),
                ));
            }

            Ok(Self {
                _drm_fd: drm_fd,
                gbm,
                display,
                context,
                surface,
                create_image,
                destroy_image,
                image_target_texture,
                supports_modifiers,
                program,
                source_uniform,
                sample_offset_uniform,
                transfer_uniform,
                output: None,
                _thread_affine: PhantomData,
            })
        }
    }

    /// Imports, scales, tone-maps, and reads a framebuffer as packed RGB8.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata, unsupported modifiers, EGL
    /// import failures, shader failures, or OpenGL ES readback failures.
    pub fn read_rgb(
        &mut self,
        frame: &DmaBufFrame<'_>,
        options: ReadbackOptions,
    ) -> Result<RgbFrame, Error> {
        let attributes = build_attributes(frame, self.supports_modifiers)?;
        validate_rgb_texture(frame)?;
        let (output_width, output_height) =
            scaled_dimensions(frame.width, frame.height, options.max_height)?;

        // SAFETY: the context belongs to this thread-affine Reader. The plane
        // FDs and attribute array remain live through import, render, and
        // cleanup; helpers validate every created handle.
        unsafe {
            if eglMakeCurrent(self.display, self.surface, self.surface, self.context) == EGL_FALSE {
                return Err(last_egl_error("eglMakeCurrent failed before readback"));
            }
            let (image, texture) = self.import_source(&attributes)?;
            let result = self.render_source(texture, frame, options, output_width, output_height);
            glBindFramebuffer(GL_FRAMEBUFFER, 0);
            glUseProgram(0);
            glBindTexture(GL_TEXTURE_2D, 0);
            glDeleteTextures(1, &raw const texture);
            (self.destroy_image)(self.display, image);
            result
        }
    }

    unsafe fn import_source(&self, attributes: &[EglInt]) -> Result<(EglImage, c_uint), Error> {
        let image = unsafe {
            (self.create_image)(
                self.display,
                ptr::null_mut(),
                EGL_LINUX_DMA_BUF_EXT,
                ptr::null_mut(),
                attributes.as_ptr(),
            )
        };
        if image.is_null() {
            return Err(last_egl_error("EGL DMA-BUF import failed"));
        }

        let mut texture = 0;
        unsafe {
            glGenTextures(1, &raw mut texture);
            glBindTexture(GL_TEXTURE_2D, texture);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            (self.image_target_texture)(GL_TEXTURE_2D, image.cast_const());
        }
        let gl_error = unsafe { glGetError() };
        if texture == 0 || gl_error != GL_NO_ERROR {
            if texture != 0 {
                unsafe { glDeleteTextures(1, &raw const texture) };
            }
            unsafe { (self.destroy_image)(self.display, image) };
            return Err(Error(format!(
                "glEGLImageTargetTexture2DOES failed (OpenGL ES error 0x{gl_error:04x})"
            )));
        }
        Ok((image, texture))
    }

    unsafe fn render_source(
        &mut self,
        source_texture: c_uint,
        frame: &DmaBufFrame<'_>,
        options: ReadbackOptions,
        output_width: u32,
        output_height: u32,
    ) -> Result<RgbFrame, Error> {
        unsafe { self.ensure_output(output_width, output_height) }?;
        let program = self.program;
        let source_uniform = self.source_uniform;
        let sample_offset_uniform = self.sample_offset_uniform;
        let transfer_uniform = self.transfer_uniform;
        let output = self
            .output
            .as_mut()
            .ok_or_else(|| Error("GPU output target disappeared after allocation".to_owned()))?;
        let vertices: [f32; 16] = [
            -1.0, -1.0, 0.0, 0.0, 1.0, -1.0, 1.0, 0.0, -1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0,
        ];
        let stride = c_int::try_from(4 * size_of::<f32>())
            .map_err(|error| Error(format!("vertex stride is too large: {error}")))?;
        let texture_coordinates = unsafe { vertices.as_ptr().add(2) };

        unsafe {
            glBindFramebuffer(GL_FRAMEBUFFER, output.framebuffer);
            glViewport(
                0,
                0,
                to_gl_size(output_width, "output width")?,
                to_gl_size(output_height, "output height")?,
            );
            glUseProgram(program);
            glActiveTexture(GL_TEXTURE0);
            glBindTexture(GL_TEXTURE_2D, source_texture);
            glUniform1i(source_uniform, 0);
            glUniform2f(
                sample_offset_uniform,
                sample_offset(frame.width, output_width),
                sample_offset(frame.height, output_height),
            );
            glUniform1f(transfer_uniform, options.transfer_function.shader_value());
            glEnableVertexAttribArray(POSITION_ATTRIBUTE);
            glEnableVertexAttribArray(TEX_COORD_ATTRIBUTE);
            glVertexAttribPointer(
                POSITION_ATTRIBUTE,
                2,
                GL_FLOAT,
                GL_FALSE,
                stride,
                vertices.as_ptr().cast(),
            );
            glVertexAttribPointer(
                TEX_COORD_ATTRIBUTE,
                2,
                GL_FLOAT,
                GL_FALSE,
                stride,
                texture_coordinates.cast(),
            );
            glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
            glDisableVertexAttribArray(POSITION_ATTRIBUTE);
            glDisableVertexAttribArray(TEX_COORD_ATTRIBUTE);
            glReadPixels(
                0,
                0,
                to_gl_size(output_width, "output width")?,
                to_gl_size(output_height, "output height")?,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                output.rgba.as_mut_ptr().cast(),
            );
        }
        let gl_error = unsafe { glGetError() };
        if gl_error != GL_NO_ERROR {
            return Err(Error(format!(
                "OpenGL ES conversion/readback failed: 0x{gl_error:04x}"
            )));
        }

        let output_len = pixel_count(output_width, output_height)?
            .checked_mul(3)
            .ok_or_else(|| Error("RGB frame size overflow".to_owned()))?;
        let mut rgb = Vec::with_capacity(output_len);
        for pixel in output.rgba.chunks_exact(4) {
            rgb.extend_from_slice(&pixel[..3]);
        }
        Ok(RgbFrame {
            width: output_width,
            height: output_height,
            pixels: rgb,
        })
    }

    unsafe fn ensure_output(&mut self, width: u32, height: u32) -> Result<(), Error> {
        if self
            .output
            .as_ref()
            .is_some_and(|output| output.width == width && output.height == height)
        {
            return Ok(());
        }
        if let Some(output) = self.output.take() {
            // SAFETY: the handles belong to the current Reader context.
            unsafe { destroy_output(&output) };
        }
        // SAFETY: the Reader context is current and dimensions were validated.
        self.output = Some(unsafe { create_output(width, height) }?);
        Ok(())
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        // SAFETY: all handles were created by this Reader and are destroyed
        // once, in reverse dependency order, while the DRM FD is still owned.
        unsafe {
            if let Some(output) = self.output.take() {
                destroy_output(&output);
            }
            glDeleteProgram(self.program);
            let _ = eglMakeCurrent(
                self.display,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let _ = eglDestroyContext(self.display, self.context);
            if !self.surface.is_null() {
                let _ = eglDestroySurface(self.display, self.surface);
            }
            let _ = eglTerminate(self.display);
            gbm_device_destroy(self.gbm);
        }
    }
}

fn to_egl_int(value: u32, label: &str) -> Result<EglInt, Error> {
    EglInt::try_from(value).map_err(|error| Error(format!("{label} is too large: {error}")))
}

fn to_gl_size(value: u32, label: &str) -> Result<c_int, Error> {
    c_int::try_from(value).map_err(|error| Error(format!("{label} is too large: {error}")))
}

fn pixel_count(width: u32, height: u32) -> Result<usize, Error> {
    usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| Error("frame dimensions overflow addressable memory".to_owned()))
}

#[allow(clippy::cast_precision_loss)]
fn sample_offset(source: u32, output: u32) -> f32 {
    // GL dimensions are already constrained to signed 32-bit values. f32 has
    // ample precision for a sub-pixel normalized texture coordinate here.
    if output < source {
        0.25 / output as f32
    } else {
        0.0
    }
}

fn scaled_dimensions(width: u32, height: u32, max_height: u32) -> Result<(u32, u32), Error> {
    if width == 0 || height == 0 || max_height == 0 {
        return Err(Error(
            "source dimensions and maximum output height must be non-zero".to_owned(),
        ));
    }
    if height <= max_height {
        return Ok((width, height));
    }

    let numerator = u64::from(width)
        .checked_mul(u64::from(max_height))
        .ok_or_else(|| Error("scaled width calculation overflowed".to_owned()))?;
    let rounded = numerator
        .checked_add(u64::from(height) / 2)
        .ok_or_else(|| Error("scaled width calculation overflowed".to_owned()))?
        / u64::from(height);
    let mut output_width = u32::try_from(rounded.max(1))
        .map_err(|error| Error(format!("scaled width is too large: {error}")))?;
    // Keep the conventional 480p result at 854x480 and produce dimensions
    // friendly to downstream image processing without materially changing the
    // aspect ratio. A one-pixel portrait output remains one pixel wide.
    if output_width > 1 && output_width % 2 != 0 {
        output_width = output_width
            .checked_add(1)
            .ok_or_else(|| Error("scaled width overflowed while aligning it".to_owned()))?;
    }
    Ok((output_width, max_height))
}

fn validate_rgb_texture(frame: &DmaBufFrame<'_>) -> Result<(), Error> {
    // The values are the DRM FourCC encodings of XR24, AR24, XB24, AB24,
    // XR30, AR30, XB30, and AB30. These are RGB textures that EGL can expose
    // as GL_TEXTURE_2D. Multi-plane YUV needs GL_TEXTURE_EXTERNAL_OES plus a
    // color-range/matrix description, so accepting it here would silently
    // produce incorrect colors. The API keeps the FourCC and plane list intact
    // so that path can be added without changing the KMS boundary.
    const SUPPORTED: [u32; 8] = [
        0x3432_5258,
        0x3432_5241,
        0x3432_4258,
        0x3432_4241,
        0x3033_5258,
        0x3033_5241,
        0x3033_4258,
        0x3033_4241,
    ];
    if frame.planes.len() != 1 {
        return Err(Error(format!(
            "multi-plane DMA-BUF format 0x{:08x} is not supported yet; RGB scanout must contain exactly one plane",
            frame.fourcc
        )));
    }
    if !SUPPORTED.contains(&frame.fourcc) {
        return Err(Error(format!(
            "DRM format 0x{:08x} is not a supported RGB8/RGB10 scanout format",
            frame.fourcc
        )));
    }
    Ok(())
}

unsafe fn create_conversion_program() -> Result<c_uint, Error> {
    // SAFETY: shader sources are stable byte arrays and every partial GL object
    // is deleted before an error is returned.
    let vertex = unsafe { compile_shader(GL_VERTEX_SHADER, VERTEX_SHADER) }?;
    // SAFETY: same lifetime and cleanup invariants as the vertex shader.
    let fragment = match unsafe { compile_shader(GL_FRAGMENT_SHADER, FRAGMENT_SHADER) } {
        Ok(shader) => shader,
        Err(error) => {
            // SAFETY: `vertex` is the live shader created above.
            unsafe { glDeleteShader(vertex) };
            return Err(error);
        }
    };
    // SAFETY: the current context owns both compiled shaders.
    let program = unsafe { glCreateProgram() };
    if program == 0 {
        // SAFETY: both shaders are live and owned by this context.
        unsafe {
            glDeleteShader(vertex);
            glDeleteShader(fragment);
        }
        return Err(last_gl_error("glCreateProgram failed"));
    }
    // SAFETY: handles are live; attribute names are NUL-terminated.
    unsafe {
        glAttachShader(program, vertex);
        glAttachShader(program, fragment);
        glBindAttribLocation(program, POSITION_ATTRIBUTE, c"a_position".as_ptr());
        glBindAttribLocation(program, TEX_COORD_ATTRIBUTE, c"a_tex_coord".as_ptr());
        glLinkProgram(program);
        glDeleteShader(vertex);
        glDeleteShader(fragment);
    }
    let mut linked = 0;
    // SAFETY: `program` is live and `linked` is writable.
    unsafe { glGetProgramiv(program, GL_LINK_STATUS, &raw mut linked) };
    if linked == 0 {
        let log = unsafe { program_log(program) };
        // SAFETY: `program` is live and no longer needed.
        unsafe { glDeleteProgram(program) };
        return Err(Error(format!(
            "GPU conversion shader failed to link: {log}"
        )));
    }
    Ok(program)
}

unsafe fn compile_shader(kind: c_uint, source: &[u8]) -> Result<c_uint, Error> {
    // SAFETY: caller has a current GLES2 context.
    let shader = unsafe { glCreateShader(kind) };
    if shader == 0 {
        return Err(last_gl_error("glCreateShader failed"));
    }
    let source_pointer = source.as_ptr().cast::<c_char>();
    let source_length = c_int::try_from(source.len())
        .map_err(|error| Error(format!("shader source is too large: {error}")))?;
    // SAFETY: the byte slice remains live for the call and an explicit length
    // means it does not need a trailing NUL.
    unsafe {
        glShaderSource(
            shader,
            1,
            &raw const source_pointer,
            &raw const source_length,
        );
        glCompileShader(shader);
    }
    let mut compiled = 0;
    // SAFETY: `shader` is live and `compiled` is writable.
    unsafe { glGetShaderiv(shader, GL_COMPILE_STATUS, &raw mut compiled) };
    if compiled == 0 {
        let log = unsafe { shader_log(shader) };
        // SAFETY: the failed shader is live and no longer needed.
        unsafe { glDeleteShader(shader) };
        return Err(Error(format!(
            "GPU conversion shader failed to compile: {log}"
        )));
    }
    Ok(shader)
}

unsafe fn shader_log(shader: c_uint) -> String {
    let mut length = 0;
    // SAFETY: `shader` is live and `length` is writable.
    unsafe { glGetShaderiv(shader, GL_INFO_LOG_LENGTH, &raw mut length) };
    gl_log(length, |capacity, written, buffer| unsafe {
        glGetShaderInfoLog(shader, capacity, written, buffer);
    })
}

unsafe fn program_log(program: c_uint) -> String {
    let mut length = 0;
    // SAFETY: `program` is live and `length` is writable.
    unsafe { glGetProgramiv(program, GL_INFO_LOG_LENGTH, &raw mut length) };
    gl_log(length, |capacity, written, buffer| unsafe {
        glGetProgramInfoLog(program, capacity, written, buffer);
    })
}

fn gl_log(mut length: c_int, read: impl FnOnce(c_int, *mut c_int, *mut c_char)) -> String {
    length = length.max(1);
    let mut bytes = vec![0_u8; usize::try_from(length).unwrap_or(1)];
    let mut written = 0;
    read(length, &raw mut written, bytes.as_mut_ptr().cast());
    let used = usize::try_from(written.max(0))
        .unwrap_or(0)
        .min(bytes.len());
    String::from_utf8_lossy(&bytes[..used]).trim().to_owned()
}

unsafe fn create_output(width: u32, height: u32) -> Result<OutputTarget, Error> {
    let width_gl = to_gl_size(width, "output width")?;
    let height_gl = to_gl_size(height, "output height")?;
    let rgba_len = pixel_count(width, height)?
        .checked_mul(4)
        .ok_or_else(|| Error("RGBA frame size overflow".to_owned()))?;
    let mut texture = 0;
    let mut framebuffer = 0;
    // SAFETY: caller has a current context and output pointers are writable.
    unsafe {
        glGenTextures(1, &raw mut texture);
        glBindTexture(GL_TEXTURE_2D, texture);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        glTexImage2D(
            GL_TEXTURE_2D,
            0,
            GL_RGBA_I,
            width_gl,
            height_gl,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            ptr::null(),
        );
        glGenFramebuffers(1, &raw mut framebuffer);
        glBindFramebuffer(GL_FRAMEBUFFER, framebuffer);
        glFramebufferTexture2D(
            GL_FRAMEBUFFER,
            GL_COLOR_ATTACHMENT0,
            GL_TEXTURE_2D,
            texture,
            0,
        );
    }
    let complete = unsafe { glCheckFramebufferStatus(GL_FRAMEBUFFER) };
    let gl_error = unsafe { glGetError() };
    // SAFETY: restore neutral bindings before returning.
    unsafe {
        glBindFramebuffer(GL_FRAMEBUFFER, 0);
        glBindTexture(GL_TEXTURE_2D, 0);
    }
    if texture == 0 || framebuffer == 0 || complete != GL_FRAMEBUFFER_COMPLETE || gl_error != 0 {
        if framebuffer != 0 {
            unsafe { glDeleteFramebuffers(1, &raw const framebuffer) };
        }
        if texture != 0 {
            unsafe { glDeleteTextures(1, &raw const texture) };
        }
        return Err(Error(format!(
            "could not create {width}x{height} GPU output target (framebuffer status 0x{complete:04x}, GL error 0x{gl_error:04x})"
        )));
    }
    Ok(OutputTarget {
        width,
        height,
        framebuffer,
        texture,
        rgba: vec![0; rgba_len],
    })
}

unsafe fn destroy_output(output: &OutputTarget) {
    // SAFETY: both handles were created together by `create_output` and are
    // deleted exactly once while their owning context is current.
    unsafe {
        glDeleteFramebuffers(1, &raw const output.framebuffer);
        glDeleteTextures(1, &raw const output.texture);
    }
}

unsafe fn create_context(display: EglDisplay) -> Result<(EglContext, EglSurface), Error> {
    // SAFETY: `display` is initialized; the helper releases every partial
    // surface/context before returning an error.
    let surfaceless = unsafe { has_egl_extension(display, b"EGL_KHR_surfaceless_context") }?;
    let config_attributes = if surfaceless {
        [
            EGL_RENDERABLE_TYPE,
            EGL_OPENGL_ES2_BIT,
            EGL_NONE,
            EGL_NONE,
            EGL_NONE,
        ]
    } else {
        [
            EGL_SURFACE_TYPE,
            EGL_PBUFFER_BIT,
            EGL_RENDERABLE_TYPE,
            EGL_OPENGL_ES2_BIT,
            EGL_NONE,
        ]
    };
    let mut config = ptr::null_mut();
    let mut config_count = 0;
    // SAFETY: output pointers and the terminated attribute array are valid.
    if unsafe {
        eglChooseConfig(
            display,
            config_attributes.as_ptr(),
            &raw mut config,
            1,
            &raw mut config_count,
        )
    } == EGL_FALSE
        || config_count != 1
    {
        return Err(last_egl_error(
            "no compatible EGL OpenGL ES 2 config is available",
        ));
    }

    let surface = if surfaceless {
        ptr::null_mut()
    } else {
        let attributes = [EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE];
        // SAFETY: `config` was selected for pbuffer support.
        let surface = unsafe { eglCreatePbufferSurface(display, config, attributes.as_ptr()) };
        if surface.is_null() {
            return Err(last_egl_error("eglCreatePbufferSurface failed"));
        }
        surface
    };
    // SAFETY: the attribute array is terminated and `config` belongs to the
    // initialized display.
    let context_attributes = [EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE];
    let context = unsafe {
        eglCreateContext(
            display,
            config,
            ptr::null_mut(),
            context_attributes.as_ptr(),
        )
    };
    if context.is_null() {
        if !surface.is_null() {
            // SAFETY: the pbuffer was created above and has no dependants.
            let _ = unsafe { eglDestroySurface(display, surface) };
        }
        return Err(last_egl_error("eglCreateContext failed"));
    }
    // SAFETY: the context and optional surface belong to this display;
    // EGL_KHR_surfaceless_context permits null draw/read surfaces.
    if unsafe { eglMakeCurrent(display, surface, surface, context) } == EGL_FALSE {
        // SAFETY: these handles were created above and are released once.
        let _ = unsafe { eglDestroyContext(display, context) };
        if !surface.is_null() {
            // SAFETY: this is the live pbuffer created above.
            let _ = unsafe { eglDestroySurface(display, surface) };
        }
        return Err(last_egl_error("eglMakeCurrent failed"));
    }
    Ok((context, surface))
}

unsafe fn required_extensions(
    display: EglDisplay,
) -> Result<(CreateImage, DestroyImage, ImageTargetTexture, bool), Error> {
    // SAFETY: `display` is initialized and current for the calling Reader.
    let extensions = unsafe { eglQueryString(display, EGL_EXTENSIONS) };
    if extensions.is_null() {
        return Err(Error("EGL extension query failed".to_owned()));
    }
    // SAFETY: EGL guarantees a NUL-terminated string for a successful query;
    // its storage remains owned by EGL for the display lifetime.
    let extension_bytes = unsafe { CStr::from_ptr(extensions) }.to_bytes();
    let has_extension = |expected: &[u8]| {
        extension_bytes
            .split(|byte| *byte == b' ')
            .any(|extension| extension == expected)
    };
    if !has_extension(b"EGL_EXT_image_dma_buf_import") {
        return Err(Error(
            "EGL_EXT_image_dma_buf_import is not supported".to_owned(),
        ));
    }
    let supports_modifiers = has_extension(b"EGL_EXT_image_dma_buf_import_modifiers");
    // SAFETY: each symbol is paired with its published Khronos ABI type.
    let create_image = unsafe { load_proc::<CreateImage>(c"eglCreateImageKHR") }?;
    // SAFETY: each symbol is paired with its published Khronos ABI type.
    let destroy_image = unsafe { load_proc::<DestroyImage>(c"eglDestroyImageKHR") }?;
    // SAFETY: each symbol is paired with its published Khronos ABI type.
    let image_target = unsafe { load_proc::<ImageTargetTexture>(c"glEGLImageTargetTexture2DOES") }?;
    Ok((
        create_image,
        destroy_image,
        image_target,
        supports_modifiers,
    ))
}

unsafe fn has_egl_extension(display: EglDisplay, expected: &[u8]) -> Result<bool, Error> {
    // SAFETY: `display` was returned by EGL and initialized by the caller.
    let extensions = unsafe { eglQueryString(display, EGL_EXTENSIONS) };
    if extensions.is_null() {
        return Err(Error("EGL extension query failed".to_owned()));
    }
    // SAFETY: a successful EGL string query returns stable NUL-terminated
    // storage owned by the display.
    Ok(unsafe { CStr::from_ptr(extensions) }
        .to_bytes()
        .split(|byte| *byte == b' ')
        .any(|extension| extension == expected))
}

fn build_attributes(
    frame: &DmaBufFrame<'_>,
    supports_modifiers: bool,
) -> Result<Vec<EglInt>, Error> {
    if frame.width == 0 || frame.height == 0 {
        return Err(Error("DMA-BUF dimensions must be non-zero".to_owned()));
    }
    if frame.planes.is_empty() || frame.planes.len() > 4 {
        return Err(Error(
            "DMA-BUF must contain between one and four planes".to_owned(),
        ));
    }
    if !supports_modifiers && frame.planes.iter().any(|plane| plane.modifier.is_some()) {
        return Err(Error(
            "EGL_EXT_image_dma_buf_import_modifiers is not supported".to_owned(),
        ));
    }
    let mut attributes = vec![
        EGL_WIDTH,
        to_egl_int(frame.width, "frame width")?,
        EGL_HEIGHT,
        to_egl_int(frame.height, "frame height")?,
        EGL_LINUX_DRM_FOURCC_EXT,
        EglInt::from_ne_bytes(frame.fourcc.to_ne_bytes()),
    ];
    for (index, plane) in frame.planes.iter().enumerate() {
        let index = EglInt::try_from(index)
            .map_err(|error| Error(format!("invalid plane index: {error}")))?;
        attributes.extend_from_slice(&[
            EGL_DMA_BUF_PLANE0_FD_EXT + index * 3,
            plane.fd.as_raw_fd(),
            EGL_DMA_BUF_PLANE0_OFFSET_EXT + index * 3,
            to_egl_int(plane.offset, "plane offset")?,
            EGL_DMA_BUF_PLANE0_PITCH_EXT + index * 3,
            to_egl_int(plane.pitch, "plane pitch")?,
        ]);
        if let Some(modifier) = plane.modifier {
            let low = u32::try_from(modifier & u64::from(u32::MAX))
                .map_err(|error| Error(format!("invalid modifier low bits: {error}")))?;
            let high = u32::try_from(modifier >> 32)
                .map_err(|error| Error(format!("invalid modifier high bits: {error}")))?;
            attributes.extend_from_slice(&[
                EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT + index * 2,
                EglInt::from_ne_bytes(low.to_ne_bytes()),
                EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT + index * 2,
                EglInt::from_ne_bytes(high.to_ne_bytes()),
            ]);
        }
    }
    attributes.push(EGL_NONE);
    Ok(attributes)
}

unsafe fn load_proc<T: Copy>(name: &CStr) -> Result<T, Error> {
    // SAFETY: EGL returns a function pointer for the exact extension name. The
    // caller supplies the matching ABI type and checks for null first.
    let pointer = unsafe { eglGetProcAddress(name.as_ptr()) };
    if pointer.is_null() {
        return Err(Error(format!(
            "required EGL/GL symbol {} is unavailable",
            name.to_string_lossy()
        )));
    }
    // SAFETY: guarded by the ABI/name invariant documented above; function
    // pointers and data pointers have the same representation on supported
    // Linux EGL platforms.
    Ok(unsafe { std::mem::transmute_copy(&pointer) })
}

fn last_egl_error(message: &str) -> Error {
    // SAFETY: eglGetError has no arguments and is valid after any EGL failure.
    let code = unsafe { eglGetError() };
    Error(format!("{message} (EGL error 0x{code:04x})"))
}

fn last_gl_error(message: &str) -> Error {
    // SAFETY: callers have made this Reader's context current.
    let code = unsafe { glGetError() };
    Error(format!("{message} (OpenGL error 0x{code:04x})"))
}

#[cfg(test)]
mod tests {
    use std::{fs::File, os::fd::AsFd};

    use super::{
        DmaBufFrame, DmaBufPlane, EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT,
        EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT, EGL_NONE, build_attributes, scaled_dimensions,
        validate_rgb_texture,
    };

    #[test]
    fn builds_terminated_modifier_attributes() -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open("/dev/null")?;
        let planes = [DmaBufPlane {
            fd: file.as_fd(),
            offset: 64,
            pitch: 4096,
            modifier: Some(0x0123_4567_89ab_cdef),
        }];
        let attributes = build_attributes(
            &DmaBufFrame {
                width: 1920,
                height: 1080,
                fourcc: 0x3432_5258,
                planes: &planes,
            },
            true,
        )?;

        assert_eq!(attributes.last(), Some(&EGL_NONE));
        assert!(attributes.contains(&EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT));
        assert!(attributes.contains(&EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT));
        Ok(())
    }

    #[test]
    fn rejects_modifier_without_egl_extension() -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open("/dev/null")?;
        let planes = [DmaBufPlane {
            fd: file.as_fd(),
            offset: 0,
            pitch: 4,
            modifier: Some(0),
        }];
        let result = build_attributes(
            &DmaBufFrame {
                width: 1,
                height: 1,
                fourcc: 0x3432_5258,
                planes: &planes,
            },
            false,
        );
        assert!(result.is_err_and(|error| error.to_string().contains("modifiers")));
        Ok(())
    }

    #[test]
    fn rejects_a_frame_without_planes() {
        let result = build_attributes(
            &DmaBufFrame {
                width: 1,
                height: 1,
                fourcc: 0x3432_5258,
                planes: &[],
            },
            true,
        );
        assert!(result.is_err_and(|error| error.to_string().contains("one and four")));
    }

    #[test]
    fn scales_common_aspect_ratios_to_480p() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(scaled_dimensions(1920, 1080, 480)?, (854, 480));
        assert_eq!(scaled_dimensions(1280, 800, 480)?, (768, 480));
        assert_eq!(scaled_dimensions(320, 180, 480)?, (320, 180));
        Ok(())
    }

    #[test]
    fn rejects_multi_plane_formats_before_egl_import() -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open("/dev/null")?;
        let planes = [
            DmaBufPlane {
                fd: file.as_fd(),
                offset: 0,
                pitch: 4,
                modifier: None,
            },
            DmaBufPlane {
                fd: file.as_fd(),
                offset: 4,
                pitch: 4,
                modifier: None,
            },
        ];
        let result = validate_rgb_texture(&DmaBufFrame {
            width: 1,
            height: 1,
            fourcc: 0x3432_5258,
            planes: &planes,
        });
        assert!(result.is_err_and(|error| error.to_string().contains("multi-plane")));
        Ok(())
    }
}
