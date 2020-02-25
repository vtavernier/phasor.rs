use crate::context::HasContext;

pub struct Texture {
    name: <glow::Context as HasContext>::Texture,
}

impl Texture {
    pub fn new(gl: &crate::Context) -> Result<Self, String> {
        Ok(Self {
            name: unsafe { gl.create_texture() }?,
        })
    }

    pub fn name(&self) -> <glow::Context as HasContext>::Texture {
        self.name
    }
}

impl super::GlDrop for Texture {
    fn drop(&mut self, gl: &crate::Context) {
        unsafe { gl.delete_texture(self.name) }
    }
}
