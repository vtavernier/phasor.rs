use std::rc::Rc;

use tinygl::prelude::*;
use tinygl::wrappers::GlHandle;

#[cfg(target_arch = "wasm32")]
mod web;

pub mod shaders;
pub mod shared;

#[derive(Default)]
pub struct Demo {}

pub struct State {
    display_program: GlHandle<shaders::DisplayProgram>,
    init_program: GlHandle<shaders::InitProgram>,
    opt_program: GlHandle<shaders::OptProgram>,
    kernels: GlHandle<tinygl::wrappers::Buffer>,
    kernel_texture: GlHandle<tinygl::wrappers::Texture>,
    grid_size: cgmath::Vector3<i32>,
}

impl State {
    fn set_params(
        &self,
        gl: &Rc<tinygl::Context>,
        program: &(impl shaders::SharedUniformSet + ProgramCommon),
    ) {
        program.use_program(gl);
        program.set_u_angle_bandwidth(&gl, 0.1);
        program.set_u_angle_mode(&gl, shared::AM_GAUSS as i32);
        program.set_u_angle_range(&gl, 2.0f32 * std::f32::consts::PI);
        program.set_u_frequency_bandwidth(&gl, 0.1);
        program.set_u_frequency_mode(&gl, shared::FM_STATIC as i32);
        program.set_u_global_seed(&gl, 0);
        program.set_u_grid(&gl, self.grid_size);
        program.set_u_isotropy_bandwidth(&gl, 0.1);
        program.set_u_isotropy_mode(&gl, 0);
        program.set_u_isotropy_power(&gl, 2.0);
        program.set_u_max_frequency(&gl, 60.0 / 32.0);
        program.set_u_min_frequency(&gl, 60.0 / 32.0);
        program.set_u_max_isotropy(&gl, 1.0);
        program.set_u_min_isotropy(&gl, 0.0);
    }
}

impl tinygl::boilerplate::Demo for Demo {
    type State = State;
    type Error = String;

    fn init(&mut self, gl: &Rc<tinygl::Context>) -> Result<State, String> {
        // Build and bind an empty VAO
        let _vao = unsafe {
            let vao_name = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(vao_name));
            vao_name
        };

        // Build demo state
        let state = State {
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
        state.set_params(gl, state.init_program.as_ref());

        unsafe {
            // Dispatch program
            gl.dispatch_compute(
                (state.grid_size.x * state.grid_size.y * state.grid_size.z) as u32,
                1,
                1,
            );
            gl.memory_barrier(tinygl::gl::ALL_BARRIER_BITS);
        }

        Ok(state)
    }

    fn render(&mut self, gl: &Rc<tinygl::Context>, state: &mut State) {
        unsafe {
            // Clear framebuffer
            gl.clear_color(1.0, 0.0, 1.0, 1.0);
            gl.clear(tinygl::gl::COLOR_BUFFER_BIT);

            // Run one optimization pass
            state.opt_program.use_program(gl);
            state
                .opt_program
                .set_u_noise_bandwidth(gl, 3.0 / std::f32::consts::PI);
            state.opt_program.set_u_cell_mode(gl, 0);
            state.opt_program.set_u_grid(gl, state.grid_size);
            state.opt_program.set_u_opt_method(gl, shared::OM_AVERAGE as i32);
            gl.dispatch_compute(
                (state.grid_size.x * state.grid_size.y * state.grid_size.z) as u32,
                1,
                1,
            );
            gl.memory_barrier(tinygl::gl::TEXTURE_FETCH_BARRIER_BIT);

            // Use the main program
            state.set_params(gl, state.display_program.as_ref());
            state.display_program.set_u_filter_modulation(gl, 2.0);
            state.display_program.set_u_filter_mod_power(gl, 2.0);
            state.display_program.set_u_isotropy_modulation(gl, 2.0);
            state
                .display_program
                .set_u_noise_bandwidth(gl, 3.0 / std::f32::consts::PI);
            state
                .display_program
                .set_u_filter_bandwidth(gl, 0.0 / std::f32::consts::PI);

            // Draw current program
            gl.draw_arrays(tinygl::gl::TRIANGLES, 0, 3);
        }
    }

    fn title(&self) -> String {
        "phasor.rs".to_owned()
    }
}
