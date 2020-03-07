//! Extensions to glow for desktop-only features

// TODO: For now this is an ugly hack. glow::Context has its own loader generated using
// gl_generator, and we add ours for extra functions we still want to use on desktop. This means
// loaders for most GL functions are duplicated but actually unused. There's probably a way to do
// better using custom generators for gl_generator.

pub mod gl {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub struct ContextEx {
    ctx: glow::Context,
    glx: gl::Gl,
}

impl ContextEx {
    pub unsafe fn from_loader_function<F>(loader_function: F) -> Self
    where
        F: FnMut(&str) -> *const std::os::raw::c_void + Clone,
    {
        use glow::HasContext;

        let gl = Self {
            ctx: glow::Context::from_loader_function(loader_function.clone()),
            glx: gl::Gl::load_with(loader_function),
        };

        // Setup logging on the context
        gl.ctx.debug_message_callback(|source, message_type, id, severity, message| {
            use crate::gl as Gl;
            let source = match source {
                Gl::DEBUG_SOURCE_API => "opengl::api",
                Gl::DEBUG_SOURCE_WINDOW_SYSTEM => "opengl::window_system",
                Gl::DEBUG_SOURCE_SHADER_COMPILER => "opengl::shader_compiler",
                Gl::DEBUG_SOURCE_THIRD_PARTY => "opengl::third_party",
                Gl::DEBUG_SOURCE_APPLICATION => "opengl::application",
                Gl::DEBUG_SOURCE_OTHER => "opengl::other",
                _ => "opengl::unknown",
            };

            let level = match severity {
                Gl::DEBUG_SEVERITY_HIGH => log::Level::Error,
                Gl::DEBUG_SEVERITY_MEDIUM => log::Level::Warn,
                Gl::DEBUG_SEVERITY_LOW => log::Level::Info,
                Gl::DEBUG_SEVERITY_NOTIFICATION => log::Level::Debug,
                _ => log::Level::Trace,
            };

            let message_type = match message_type {
                Gl::DEBUG_TYPE_ERROR => "error",
                Gl::DEBUG_TYPE_DEPRECATED_BEHAVIOR => "deprecated behavior",
                Gl::DEBUG_TYPE_UNDEFINED_BEHAVIOR => "undefined behavior",
                Gl::DEBUG_TYPE_PORTABILITY => "portability",
                Gl::DEBUG_TYPE_PERFORMANCE => "performance",
                Gl::DEBUG_TYPE_MARKER => "marker",
                Gl::DEBUG_TYPE_PUSH_GROUP => "push group",
                Gl::DEBUG_TYPE_POP_GROUP => "pop group",
                Gl::DEBUG_TYPE_OTHER => "other",
                _ => "unknown",
            };

            // Create record manually so we can override the module path
            log::logger().log(
                &log::Record::builder()
                    .args(format_args!(
                        "{} ({}): {}{}",
                        message_type,
                        id,
                        message,
                        if level == log::Level::Warn || level == log::Level::Error {
                            format!(", stack backtrace:\n{:?}", backtrace::Backtrace::new())
                        } else {
                            "".to_owned()
                        }
                    ))
                    .level(level)
                    .target("opengl")
                    .module_path_static(Some(source))
                    .build(),
            );
        });

        gl
    }

    pub unsafe fn shader_binary(
        &self,
        shaders: &[<glow::Context as glow::HasContext>::Shader],
        binary_format: u32,
        binary: &[u8],
    ) {
        self.glx.ShaderBinary(
            shaders.len() as i32,
            shaders.as_ptr() as *const _,
            binary_format,
            binary.as_ptr() as *const _,
            binary.len() as i32,
        )
    }

    pub unsafe fn specialize_shader(
        &self,
        shader: <glow::Context as glow::HasContext>::Shader,
        entry_point: &str,
        constant_indices: &[u32],
        constant_values: &[u32],
    ) {
        assert!(constant_indices.len() == constant_values.len());

        let entry_point = std::ffi::CString::new(entry_point).unwrap();
        self.glx.SpecializeShader(
            shader,
            entry_point.as_bytes_with_nul().as_ptr() as *const i8,
            constant_indices.len() as u32,
            constant_indices.as_ptr(),
            constant_values.as_ptr(),
        );
    }

    pub unsafe fn tex_buffer(
        &self,
        target: u32,
        internal_format: u32,
        buffer: <glow::Context as glow::HasContext>::Buffer,
    ) {
        self.glx.TexBuffer(target, internal_format, buffer)
    }

    pub unsafe fn memory_barrier(&self, barriers: u32) {
        self.glx.MemoryBarrier(barriers)
    }

    pub unsafe fn bind_image_texture(
        &self,
        unit: u32,
        texture: <glow::Context as glow::HasContext>::Texture,
        level: i32,
        layered: bool,
        layer: i32,
        access: u32,
        format: u32,
    ) {
        self.glx.BindImageTexture(
            unit,
            texture,
            level,
            if layered {
                super::gl::TRUE
            } else {
                super::gl::FALSE
            },
            layer,
            access,
            format,
        );
    }
}

impl std::ops::Deref for ContextEx {
    type Target = glow::Context;

    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}

impl std::convert::AsRef<glow::Context> for ContextEx {
    fn as_ref(&self) -> &glow::Context {
        &self.ctx
    }
}
