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
    display_program: shaders::DisplayProgram,
    init_program: shaders::InitProgram,
    opt_program: shaders::OptProgram,
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

        Ok(State {
            display_program: shaders::DisplayProgram::build(&gl)?,
            init_program: shaders::InitProgram::build(&gl)?,
            opt_program: shaders::OptProgram::build(&gl)?,
        })
    }

    fn render(&mut self, gl: &tinygl::Context, state: &mut State) {
        unsafe {
            // Clear framebuffer
            gl.clear_color(1.0, 0.0, 1.0, 1.0);
            gl.clear(tinygl::gl::COLOR_BUFFER_BIT);

            // Use the main program
            state.display_program.use_program(gl);

            // Draw current program
            gl.draw_arrays(tinygl::gl::TRIANGLES, 0, 3);
        }
    }

    fn title(&self) -> String {
        "phasor.rs".to_owned()
    }
}
