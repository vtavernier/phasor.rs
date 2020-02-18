//! OpenGL object wrappers

mod shader;
pub use shader::*;

mod program;
pub use program::*;

/// Trait for GL objects that can be dropped
pub trait GlDrop {
    fn drop(&mut self, gl: &crate::Context);
}

pub struct GlHandle<'gl, T: GlDrop> {
    gl: &'gl crate::Context,
    res: T,
}

impl<'gl, T: GlDrop> GlHandle<'gl, T> {
    pub fn new(gl: &'gl crate::Context, res: T) -> Self {
        Self { gl, res }
    }
}

impl<'gl, T: GlDrop> Drop for GlHandle<'gl, T> {
    fn drop(&mut self) {
        self.res.drop(self.gl);
    }
}

impl<'gl, T: GlDrop> std::convert::AsRef<T> for GlHandle<'gl, T> {
    fn as_ref(&self) -> &T {
        &self.res
    }
}

impl<'gl, T: GlDrop> std::convert::AsMut<T> for GlHandle<'gl, T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.res
    }
}
