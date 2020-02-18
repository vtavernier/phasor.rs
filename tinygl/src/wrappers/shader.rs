use crate::context::{Context, HasContext};

/// Common traits to binary and source shaders
pub trait ShaderCommon {
    fn kind() -> u32;
    fn name(&self) -> <glow::Context as HasContext>::Shader;
}

/// Build a shader name and try to compile it using the given callback
unsafe fn make_shader<F>(
    gl: &Context,
    kind: u32,
    mut compile_cb: F,
) -> Result<<glow::Context as HasContext>::Shader, String>
where
    F: FnMut(<glow::Context as HasContext>::Shader) -> (),
{
    // Create shader object
    let shader_name = gl.create_shader(kind)?;

    compile_cb(shader_name);

    // Check that the compile status is ok
    if !gl.get_shader_compile_status(shader_name) {
        let log = gl.get_shader_info_log(shader_name);
        gl.delete_shader(shader_name);
        return Err(log);
    }

    Ok(shader_name)
}

/// SPIR-V shader wrapper
#[cfg(not(target_arch = "wasm32"))]
pub trait BinaryShader<'a>: ShaderCommon {
    fn get_binary() -> &'a [u8];

    fn build(gl: &Context) -> Result<<glow::Context as HasContext>::Shader, String> {
        unsafe {
            make_shader(gl, Self::kind(), |shader_name| {
                use crate::gl;

                // Load the binary
                gl.shader_binary(
                    &[shader_name],
                    gl::SHADER_BINARY_FORMAT_SPIR_V,
                    Self::get_binary(),
                );

                // Specialize the binary
                gl.specialize_shader(shader_name, "main", &[], &[]);
            })
        }
    }
}

/// GLSL shader wrapper
pub trait SourceShader<'a>: ShaderCommon {
    fn get_source() -> &'a str;

    fn build(gl: &Context) -> Result<<glow::Context as HasContext>::Shader, String> {
        unsafe {
            make_shader(gl, Self::kind(), |shader_name| {
                // Load the binary
                gl.shader_source(shader_name, Self::get_source());

                // Specialize the binary
                gl.compile_shader(shader_name);
            })
        }
    }
}
