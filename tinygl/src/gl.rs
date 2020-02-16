//! Exposed OpenGL bindings

#[cfg(not(target_arch = "wasm32"))]
pub use crate::glowx::gl::SHADER_BINARY_FORMAT_SPIR_V;
pub use ::glow::*;
