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
    pub fn from_loader_function<F>(loader_function: F) -> Self
    where
        F: FnMut(&str) -> *const std::os::raw::c_void + Clone,
    {
        Self {
            ctx: glow::Context::from_loader_function(loader_function.clone()),
            glx: gl::Gl::load_with(loader_function),
        }
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

    /// Set up a callback for debug messages from the OpenGL driver
    pub unsafe fn debug_message_callback<F>(&self, callback: F)
    where
        F: FnMut(u32, u32, u32, u32, &std::ffi::CStr) + 'static,
    {
        self.glx.DebugMessageCallback(
            Some(tinygl_debug_message_callback::<F>),
            Box::into_raw(Box::new(callback)) as *const std::ffi::c_void,
        );
    }
}

extern "system" fn tinygl_debug_message_callback<F>(
    source: u32,
    message_type: u32,
    id: u32,
    severity: u32,
    length: i32,
    message: *const i8,
    user_param: *mut std::ffi::c_void,
) where
    F: FnMut(u32, u32, u32, u32, &std::ffi::CStr),
{
    unsafe {
        let callback_ptr = user_param as *mut F;
        let callback = &mut *callback_ptr;

        let message = &std::ffi::CStr::from_bytes_with_nul_unchecked(std::slice::from_raw_parts(
            message as *const _,
            length as usize,
        ));

        callback(source, message_type, id, severity, message);
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
