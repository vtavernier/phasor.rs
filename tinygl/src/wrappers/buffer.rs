use crate::context::HasContext;

pub struct Buffer {
    name: <glow::Context as HasContext>::Buffer,
}

impl Buffer {
    pub fn new(gl: &crate::Context) -> Result<Self, String> {
        Ok(Self { name: unsafe { gl.create_buffer() }? })
    }

    pub fn name(&self) -> <glow::Context as HasContext>::Buffer {
        self.name
    }
}

impl super::GlDrop for Buffer {
    fn drop(&mut self, gl: &crate::Context) {
        unsafe { gl.delete_buffer(self.name) }
    }
}
