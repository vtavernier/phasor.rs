#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod glowx;

pub mod boilerplate;

mod context;
pub use context::*;

pub mod gl;

mod wrappers;
pub use wrappers::*;

pub mod prelude {
    pub use super::HasContext;

    #[cfg(not(target_arch = "wasm32"))]
    pub use super::BinaryShader;
    pub use super::SourceShader;
}
