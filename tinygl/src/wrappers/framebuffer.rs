use crate::context::HasContext;

pub struct Framebuffer {
    name: <glow::Context as HasContext>::Framebuffer,
}

impl Framebuffer {
    pub fn new(gl: &crate::Context) -> Result<Self, String> {
        Ok(Self {
            name: unsafe { gl.create_framebuffer() }?,
        })
    }

    pub fn name(&self) -> <glow::Context as HasContext>::Framebuffer {
        self.name
    }

    pub fn bind(&self, gl: &crate::Context, target: u32) {
        unsafe { gl.bind_framebuffer(target, Some(self.name)) }
    }

    pub fn renderbuffer(
        &self,
        gl: &crate::Context,
        target: u32,
        attachment: u32,
        renderbuffer: Option<impl AsRef<super::Renderbuffer>>,
    ) {
        unsafe {
            gl.framebuffer_renderbuffer(
                target,
                attachment,
                crate::gl::RENDERBUFFER,
                renderbuffer.map(|rb| rb.as_ref().name()),
            );
        }
    }

    pub fn texture(
        &self,
        gl: &crate::Context,
        target: u32,
        attachment: u32,
        texture: Option<impl AsRef<super::Texture>>,
        level: i32,
    ) {
        unsafe {
            gl.framebuffer_texture(
                target,
                attachment,
                texture.map(|rb| rb.as_ref().name()),
                level,
            );
        }
    }
}

impl super::GlDrop for Framebuffer {
    fn drop(&mut self, gl: &crate::Context) {
        unsafe { gl.delete_framebuffer(self.name) }
    }
}
