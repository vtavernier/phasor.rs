use std::rc::Rc;

use tinygl::prelude::*;
use tinygl::wrappers::GlHandle;

pub struct TextureRenderTarget {
    pub framebuffer: GlHandle<tinygl::wrappers::Framebuffer>,
    pub depthbuffer: GlHandle<tinygl::wrappers::Renderbuffer>,
    pub texture_main: GlHandle<tinygl::wrappers::Texture>,
    pub texture_extra: GlHandle<tinygl::wrappers::Texture>,
    current_size: Option<cgmath::Vector2<i32>>,
}

impl TextureRenderTarget {
    pub fn new(
        gl: &Rc<tinygl::Context>,
        width: u32,
        height: u32,
    ) -> tinygl::Result<TextureRenderTarget> {
        // Create objects
        let mut this = Self {
            framebuffer: GlHandle::new(gl, tinygl::wrappers::Framebuffer::new(gl)?),
            depthbuffer: GlHandle::new(gl, tinygl::wrappers::Renderbuffer::new(gl)?),
            texture_main: GlHandle::new(gl, tinygl::wrappers::Texture::new(gl)?),
            texture_extra: GlHandle::new(gl, tinygl::wrappers::Texture::new(gl)?),
            current_size: None,
        };

        // Initial allocation
        this.alloc(gl, width, height);

        // Don't use mipmaps
        unsafe {
            for tex in [&this.texture_main, &this.texture_extra].iter() {
                tex.bind(gl, tinygl::gl::TEXTURE_2D);
                gl.tex_parameter_i32(
                    tinygl::gl::TEXTURE_2D,
                    tinygl::gl::TEXTURE_MIN_FILTER,
                    tinygl::gl::NEAREST as i32,
                );
                gl.tex_parameter_i32(
                    tinygl::gl::TEXTURE_2D,
                    tinygl::gl::TEXTURE_MAG_FILTER,
                    tinygl::gl::NEAREST as i32,
                );
            }

            gl.bind_texture(tinygl::gl::TEXTURE_2D, None);
        }

        // Setup bindings
        unsafe {
            this.framebuffer.bind(gl, tinygl::gl::FRAMEBUFFER);
            this.framebuffer.renderbuffer(
                gl,
                tinygl::gl::FRAMEBUFFER,
                tinygl::gl::DEPTH_ATTACHMENT,
                Some(&this.depthbuffer),
            );
            this.framebuffer.texture(
                gl,
                tinygl::gl::FRAMEBUFFER,
                tinygl::gl::COLOR_ATTACHMENT0,
                Some(&this.texture_main),
                0,
            );
            this.framebuffer.texture(
                gl,
                tinygl::gl::FRAMEBUFFER,
                tinygl::gl::COLOR_ATTACHMENT1,
                Some(&this.texture_extra),
                0,
            );
            gl.draw_buffers(&[tinygl::gl::COLOR_ATTACHMENT0, tinygl::gl::COLOR_ATTACHMENT1]);
            gl.bind_framebuffer(tinygl::gl::FRAMEBUFFER, None);
        }

        Ok(this)
    }

    pub fn alloc(&mut self, gl: &Rc<tinygl::Context>, width: u32, height: u32) {
        let new_size = cgmath::vec2(width as i32, height as i32);

        if !self.current_size.map(|cs| cs == new_size).unwrap_or(false) {
            // Setup storage
            unsafe {
                // Depth buffer
                self.depthbuffer.bind(gl);
                gl.renderbuffer_storage(
                    tinygl::gl::RENDERBUFFER,
                    tinygl::gl::DEPTH_COMPONENT,
                    new_size.x,
                    new_size.y,
                );
                gl.bind_renderbuffer(tinygl::gl::RENDERBUFFER, None);

                // Textures
                for tex in [&self.texture_main, &self.texture_extra].iter() {
                    tex.bind(gl, tinygl::gl::TEXTURE_2D);
                    gl.tex_image_2d(
                        tinygl::gl::TEXTURE_2D,
                        0,
                        tinygl::gl::RGBA32F as i32,
                        new_size.x,
                        new_size.y,
                        0,
                        tinygl::gl::RGBA,
                        tinygl::gl::FLOAT,
                        None,
                    );
                }

                gl.bind_texture(tinygl::gl::TEXTURE_2D, None);
            }

            // Update size
            self.current_size = Some(new_size);
        }
    }
}
