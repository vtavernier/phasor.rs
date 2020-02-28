#[macro_use]
extern crate log;

use std::rc::Rc;

use tinygl::prelude::*;
use tinygl::wrappers::GlHandle;

pub mod shaders;
pub mod shared;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OptimizationMode {
    None,
    Optimize,
    Average,
}

impl OptimizationMode {
    pub fn as_mode(&self) -> i32 {
        match self {
            Self::None => -1,
            Self::Optimize => shared::OM_OPTIMIZE as i32,
            Self::Average => shared::OM_AVERAGE as i32,
        }
    }

    pub fn toggle_and_switch(
        &mut self,
        active_mode: &mut OptimizationMode,
        target_mode: OptimizationMode,
    ) {
        *active_mode = target_mode;
        if *self == target_mode {
            *self = Self::None;
        } else {
            *self = target_mode;
        }
    }

    pub fn toggle(&mut self, active_mode: &mut OptimizationMode) {
        match self {
            Self::None => *self = *active_mode,
            Self::Optimize | Self::Average => {
                *active_mode = *self;
                *self = Self::None;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        match self {
            Self::None => false,
            _ => true,
        }
    }
}

impl Default for OptimizationMode {
    fn default() -> Self {
        Self::None
    }
}

pub struct State {
    display_program: GlHandle<shaders::DisplayProgram>,
    init_program: GlHandle<shaders::InitProgram>,
    opt_program: GlHandle<shaders::OptProgram>,
    kernels: GlHandle<tinygl::wrappers::Buffer>,
    kernel_texture: GlHandle<tinygl::wrappers::Texture>,
    grid_size: cgmath::Vector3<i32>,
}

impl State {
    pub fn new(gl: &Rc<tinygl::Context>) -> Result<Self, String> {
        // Build demo state
        let state = Self {
            display_program: GlHandle::new(gl, shaders::DisplayProgram::build(&gl)?),
            init_program: GlHandle::new(gl, shaders::InitProgram::build(&gl)?),
            opt_program: GlHandle::new(gl, shaders::OptProgram::build(&gl)?),
            kernels: GlHandle::new(gl, tinygl::wrappers::Buffer::new(&gl)?),
            kernel_texture: GlHandle::new(gl, tinygl::wrappers::Texture::new(&gl)?),
            grid_size: cgmath::vec3(16, 16, 1),
        };

        // Setup buffer storage
        unsafe {
            gl.bind_buffer(tinygl::gl::TEXTURE_BUFFER, Some(state.kernels.name()));
            // NFLOATS * sizeof(float) * GridX * GridY * K
            gl.buffer_storage(
                tinygl::gl::TEXTURE_BUFFER,
                (shared::NFLOATS as usize
                    * std::mem::size_of::<f32>()
                    * (state.grid_size.x * state.grid_size.y * state.grid_size.z) as usize
                    * shared::CURRENT_K as usize) as i32,
                None,
                tinygl::gl::MAP_READ_BIT,
            );
            gl.bind_buffer(tinygl::gl::TEXTURE_BUFFER, None);
        }

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
            gl.bind_image_texture(
                0, // TODO get_u_kernels_binding
                state.kernel_texture.name(),
                0,
                false,
                0,
                tinygl::gl::READ_WRITE,
                tinygl::gl::R32F,
            );
        }

        // Initialize kernels
        state.run_init(gl);

        Ok(state)
    }

    pub fn run_init(&self, gl: &Rc<tinygl::Context>) {
        self.set_params(gl, self.init_program.as_ref());

        unsafe {
            // Dispatch program
            gl.dispatch_compute(
                (self.grid_size.x * self.grid_size.y * self.grid_size.z) as u32,
                1,
                1,
            );
            gl.memory_barrier(tinygl::gl::ALL_BARRIER_BITS);
        }
    }

    pub fn run_optimize(&self, gl: &Rc<tinygl::Context>, mode: OptimizationMode) {
        if !mode.is_active() {
            warn!("invalid optimization mode: {:?}", mode);
            return;
        }

        // Run one optimization pass
        self.opt_program.use_program(gl);
        self.opt_program
            .set_u_noise_bandwidth(gl, 3.0 / std::f32::consts::PI);
        self.opt_program.set_u_cell_mode(gl, 0);
        self.opt_program.set_u_grid(gl, self.grid_size);
        self.opt_program.set_u_opt_method(gl, mode.as_mode());

        unsafe {
            gl.dispatch_compute(
                (self.grid_size.x * self.grid_size.y * self.grid_size.z) as u32,
                1,
                1,
            );
            gl.memory_barrier(tinygl::gl::TEXTURE_FETCH_BARRIER_BIT);
        }
    }

    pub fn run_display(&self, gl: &Rc<tinygl::Context>) {
        self.set_params(gl, self.display_program.as_ref());
        self.display_program.set_u_filter_modulation(gl, 2.0);
        self.display_program.set_u_filter_mod_power(gl, 2.0);
        self.display_program.set_u_isotropy_modulation(gl, 2.0);
        self.display_program
            .set_u_noise_bandwidth(gl, 3.0 / std::f32::consts::PI);
        self.display_program
            .set_u_filter_bandwidth(gl, 0.0 / std::f32::consts::PI);

        unsafe {
            // Draw current program
            gl.draw_arrays(tinygl::gl::TRIANGLES, 0, 3);
        }
    }

    fn set_params(
        &self,
        gl: &Rc<tinygl::Context>,
        program: &(impl shaders::SharedUniformSet + ProgramCommon),
    ) {
        program.use_program(gl);

        program.set_u_grid(&gl, self.grid_size);
        program.set_u_angle_bandwidth(&gl, 0.1);
        program.set_u_angle_mode(&gl, shared::AM_GAUSS as i32);
        program.set_u_angle_offset(&gl, 0.0);
        program.set_u_angle_range(&gl, std::f32::consts::PI);
        program.set_u_cell_mode(&gl, shared::CM_CLAMP as i32);
        program.set_u_frequency_bandwidth(&gl, 0.1);
        program.set_u_frequency_mode(&gl, shared::FM_STATIC as i32);
        program.set_u_global_seed(&gl, 171);
        program.set_u_isotropy_bandwidth(&gl, 0.1);
        program.set_u_isotropy_power(&gl, 1.0);
        program.set_u_max_frequency(&gl, 4.0);
        program.set_u_max_isotropy(&gl, 1.0);
        program.set_u_min_frequency(&gl, 2.0);
        program.set_u_min_isotropy(&gl, 0.0);
    }
}
