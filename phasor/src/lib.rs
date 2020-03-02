#[macro_use]
extern crate log as log_crate;

use std::rc::Rc;

use tinygl::prelude::*;
use tinygl::wrappers::GlHandle;

pub mod api;
pub mod log;
mod optimization_mode;
pub use optimization_mode::*;
mod params;
pub use params::*;
pub mod shaders;
pub mod shared;
mod texture_render_target;
use texture_render_target::*;

pub struct State {
    display_program: GlHandle<shaders::DisplayProgram>,
    init_program: GlHandle<shaders::InitProgram>,
    opt_program: GlHandle<shaders::OptProgram>,
    kernels: GlHandle<tinygl::wrappers::Buffer>,
    kernel_texture: GlHandle<tinygl::wrappers::Texture>,
    allocated_size: usize,
    texture_render_target: Option<TextureRenderTarget>,
}

impl State {
    pub fn new(gl: &Rc<tinygl::Context>) -> Result<Self, String> {
        // Build demo state
        let mut state = Self {
            display_program: GlHandle::new(gl, shaders::DisplayProgram::build(&gl)?),
            init_program: GlHandle::new(gl, shaders::InitProgram::build(&gl)?),
            opt_program: GlHandle::new(gl, shaders::OptProgram::build(&gl)?),
            kernels: GlHandle::new(gl, tinygl::wrappers::Buffer::new(&gl)?),
            kernel_texture: GlHandle::new(gl, tinygl::wrappers::Texture::new(&gl)?),
            allocated_size: 0,
            texture_render_target: None,
        };

        // Initialize grid
        state
            .check_grid(gl, &Params::default())
            .map_err(|err| format!("OpenGL error: {}", err))?;

        // Setup texture for buffer storage
        unsafe {
            gl.bind_texture(
                tinygl::gl::TEXTURE_BUFFER,
                Some(state.kernel_texture.name()),
            );
            gl.tex_buffer(
                tinygl::gl::TEXTURE_BUFFER,
                tinygl::gl::R32F,
                state.kernels.name(),
            );
            gl.bind_texture(tinygl::gl::TEXTURE_BUFFER, None);
        }

        Ok(state)
    }

    pub fn run_init(&mut self, gl: &Rc<tinygl::Context>, params: &Params) {
        // Check grid status
        self.check_grid(gl, params)
            .expect("failed to allocate grid");

        // Set params
        self.init_program.use_program(gl);
        params.apply_shared(gl, self.init_program.as_ref());

        unsafe {
            // Bind kernel data
            gl.bind_image_texture(
                self.init_program.get_u_kernels_binding(),
                self.kernel_texture.name(),
                0,
                false,
                0,
                tinygl::gl::READ_WRITE,
                tinygl::gl::R32F,
            );

            // Dispatch program
            gl.dispatch_compute(
                params.grid_size.x as u32,
                params.grid_size.y as u32,
                params.grid_size.z as u32,
            );

            gl.memory_barrier(tinygl::gl::TEXTURE_FETCH_BARRIER_BIT);
        }
    }

    pub fn run_optimize(
        &mut self,
        gl: &Rc<tinygl::Context>,
        mode: OptimizationMode,
        steps: u32,
        params: &Params,
    ) {
        if !mode.is_active() {
            warn!("invalid optimization mode: {:?}", mode);
            return;
        }

        if steps < 1 {
            warn!("invalid optimization step count: {:?}", steps);
            return;
        }

        // Check grid status
        self.check_grid(gl, params)
            .expect("failed to allocate grid");

        // Run one optimization pass
        self.opt_program.use_program(gl);
        params.apply_global(gl, self.opt_program.as_ref());
        self.opt_program
            .set_u_noise_bandwidth(gl, params.noise_bandwidth);
        self.opt_program.set_u_opt_method(gl, mode.as_mode());
        self.opt_program.set_u_opt_steps(gl, steps);

        unsafe {
            // Bind kernel data
            gl.bind_image_texture(
                self.opt_program.get_u_kernels_binding(),
                self.kernel_texture.name(),
                0,
                false,
                0,
                tinygl::gl::READ_WRITE,
                tinygl::gl::R32F,
            );

            gl.dispatch_compute(
                (params.grid_size.x * params.grid_size.y * params.grid_size.z) as u32,
                1,
                1,
            );

            gl.memory_barrier(tinygl::gl::TEXTURE_FETCH_BARRIER_BIT);
        }
    }

    pub fn run_display(&mut self, gl: &Rc<tinygl::Context>, params: &Params, display_mode: i32) {
        // Check grid status
        self.check_grid(gl, params)
            .expect("failed to allocate grid");

        self.display_program.use_program(gl);
        params.apply_shared(gl, self.display_program.as_ref());
        self.display_program
            .set_u_filter_modulation(gl, params.filter_modulation);
        self.display_program
            .set_u_filter_mod_power(gl, params.filter_mod_power);
        self.display_program
            .set_u_isotropy_modulation(gl, params.isotropy_modulation);
        self.display_program
            .set_u_noise_bandwidth(gl, params.noise_bandwidth);
        self.display_program
            .set_u_filter_bandwidth(gl, params.filter_bandwidth);
        self.display_program.set_u_display_mode(gl, display_mode);

        unsafe {
            // Bind kernel data
            gl.bind_image_texture(
                self.display_program.get_u_kernels_binding(),
                self.kernel_texture.name(),
                0,
                false,
                0,
                tinygl::gl::READ_WRITE,
                tinygl::gl::R32F,
            );

            // Draw current program
            gl.draw_arrays(tinygl::gl::TRIANGLES, 0, 3);
        }
    }

    pub fn render_to_texture(
        &mut self,
        gl: &Rc<tinygl::Context>,
        width: u32,
        height: u32,
        display_mode: i32,
        params: &Params,
        buffer_main: &mut Vec<f32>,
        buffer_extra: &mut Vec<f32>,
    ) {
        // Prepare render target
        let trt = {
            if self.texture_render_target.is_none() {
                self.texture_render_target = Some(
                    TextureRenderTarget::new(gl, width, height)
                        .expect("failed to create render target"),
                );
            }

            self.texture_render_target.as_mut().unwrap()
        };

        trt.alloc(gl, width, height);

        // Set target framebuffer
        trt.framebuffer.bind(gl, tinygl::gl::FRAMEBUFFER);

        unsafe {
            // Set viewport
            gl.viewport(0, 0, width as i32, height as i32);

            // Render
            self.run_display(gl, params, display_mode);

            // Render target
            let trt = self.texture_render_target.as_mut().unwrap();

            // Get images
            trt.texture_main.bind(gl, tinygl::gl::TEXTURE_2D);
            buffer_main.resize(
                width as usize * height as usize * std::mem::size_of::<f32>() * 4,
                0.0,
            );
            gl.get_tex_image_u8_slice(
                tinygl::gl::TEXTURE_2D,
                0,
                tinygl::gl::RGBA,
                tinygl::gl::FLOAT,
                Some(std::mem::transmute(&buffer_main[..])),
            );

            trt.texture_extra.bind(gl, tinygl::gl::TEXTURE_2D);
            buffer_extra.resize(
                width as usize * height as usize * std::mem::size_of::<f32>() * 4,
                0.0,
            );
            gl.get_tex_image_u8_slice(
                tinygl::gl::TEXTURE_2D,
                0,
                tinygl::gl::RGBA,
                tinygl::gl::FLOAT,
                Some(std::mem::transmute(&buffer_extra[..])),
            );
        }

        // Cleanup
        unsafe {
            gl.bind_framebuffer(tinygl::gl::FRAMEBUFFER, None);
        }
    }

    fn check_grid(&mut self, gl: &Rc<tinygl::Context>, params: &Params) -> Result<(), u32> {
        let new_alloc_size = shared::NFLOATS as usize
            * std::mem::size_of::<f32>()
            * (params.grid_size.x * params.grid_size.y * params.grid_size.z) as usize
            * params.kernel_count as usize;

        if new_alloc_size > self.allocated_size {
            info!(
                "reallocating for grid_size: {:?}, kernel_count: {}, bytes: {}",
                params.grid_size,
                params.kernel_count,
                bytesize::ByteSize(new_alloc_size as u64)
            );

            // Setupinitialize buffer storage
            unsafe {
                gl.bind_buffer(tinygl::gl::TEXTURE_BUFFER, Some(self.kernels.name()));
                gl.buffer_data_size(
                    tinygl::gl::TEXTURE_BUFFER,
                    new_alloc_size as i32,
                    tinygl::gl::DYNAMIC_DRAW,
                );

                // Check allocation errors
                let error = gl.get_error();

                gl.bind_buffer(tinygl::gl::TEXTURE_BUFFER, None);

                if error != tinygl::gl::NO_ERROR {
                    return Err(error);
                }
            }

            // Updated allocated size
            self.allocated_size = new_alloc_size;
        }

        Ok(())
    }
}
