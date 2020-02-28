#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod glowx;

mod context;
pub use context::*;

pub mod gl;
pub mod wrappers;

pub use cgmath;
pub use glow;

pub mod prelude {
    pub use super::glow::HasContext;

    pub use super::wrappers::ProgramCommon;
    pub use super::wrappers::ShaderCommon;

    #[cfg(not(target_arch = "wasm32"))]
    pub use super::wrappers::BinaryShader;
    pub use super::wrappers::SourceShader;

    pub use cgmath;
}
