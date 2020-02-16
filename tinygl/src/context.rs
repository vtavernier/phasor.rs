/// OpenGL function context
#[cfg(not(target_arch = "wasm32"))]
pub type Context = crate::glowx::ContextEx;

/// OpenGL function context
#[cfg(target_arch = "wasm32")]
pub type Context = glow::Context;

pub use glow::HasContext;
