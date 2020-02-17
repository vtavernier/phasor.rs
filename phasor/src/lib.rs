use tinygl::prelude::*;

#[cfg(target_arch = "wasm32")]
mod web;

pub mod shaders {
    // Compiled by tinygl-compiler
    include!(concat!(env!("OUT_DIR"), "/shaders.rs"));
}

#[derive(Default)]
pub struct Demo {
    prog_uniforms: shaders::DisplayFragUniforms,
}

impl<'a> tinygl::boilerplate::Demo<'a> for Demo {
    fn init(&mut self, gl: &tinygl::Context) {
        // Build and bind an empty VAO
        let _vao = unsafe {
            let vao_name = gl.create_vertex_array().unwrap();
            gl.bind_vertex_array(Some(vao_name));
            vao_name
        };

        // Build the main program
        let vert_shader = shaders::QuadVertShader::build(&gl).unwrap();
        let frag_shader = shaders::DisplayFragShader::build(&gl).unwrap();

        let prog = unsafe {
            let program_name = gl.create_program().unwrap();

            gl.attach_shader(program_name, vert_shader);
            gl.attach_shader(program_name, frag_shader);

            gl.link_program(program_name);

            assert!(gl.get_program_link_status(program_name));

            gl.detach_shader(program_name, vert_shader);
            gl.detach_shader(program_name, frag_shader);

            gl.delete_shader(vert_shader);
            gl.delete_shader(frag_shader);

            program_name
        };

        self.prog_uniforms = shaders::DisplayFragUniforms::new(gl, prog);

        // Use the main program
        unsafe { gl.use_program(Some(prog)) };
    }

    fn render(&mut self, gl: &tinygl::Context) {
        unsafe {
            // Clear framebuffer
            gl.clear_color(1.0, 0.0, 1.0, 1.0);
            gl.clear(tinygl::gl::COLOR_BUFFER_BIT);

            // Set uniforms
            self.prog_uniforms.set_u_stuff(&gl, cgmath::vec3(1, 1, 1));

            // Draw current program
            gl.draw_arrays(tinygl::gl::TRIANGLES, 0, 3);
        }
    }

    fn title(&self) -> &'a str {
        "phasor.rs"
    }
}
