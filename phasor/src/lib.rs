use tinygl::prelude::*;

#[cfg(target_arch = "wasm32")]
mod web;

pub mod shaders {
    // Compiled by tinygl-compiler
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

#[derive(Default)]
pub struct Demo {}

pub struct State {
    // TODO: Handle cleanup using GlHandle
    program: shaders::DemoProgram,
}

impl tinygl::boilerplate::Demo for Demo {
    type State = State;
    type Error = String;

    fn init(&mut self, gl: &tinygl::Context) -> Result<State, String> {
        // Build and bind an empty VAO
        let _vao = unsafe {
            let vao_name = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(vao_name));
            vao_name
        };

        // Build the main program
        let program = shaders::DemoProgram::build(gl)?;

        // Use the main program
        unsafe { gl.use_program(Some(program.name())) };

        Ok(State { program })
    }

    fn render(&mut self, gl: &tinygl::Context, state: &mut State) {
        unsafe {
            // Clear framebuffer
            gl.clear_color(1.0, 0.0, 1.0, 1.0);
            gl.clear(tinygl::gl::COLOR_BUFFER_BIT);

            // Set uniforms
            state.program.set_u_stuff(&gl, cgmath::vec3(1, 1, 1));

            // Draw current program
            gl.draw_arrays(tinygl::gl::TRIANGLES, 0, 3);
        }
    }

    fn title(&self) -> String {
        "phasor.rs".to_owned()
    }
}
