use std::env;

fn main() {
    let mut compiler = tinygl_compiler::CompilerBuilder::new().build().unwrap();

    let mesh_vert = compiler.wrap_shader("shaders/mesh.frag").unwrap();
    let mesh_frag = compiler.wrap_shader("shaders/mesh.vert").unwrap();

    let mesh_prog = compiler
        .wrap_program(&[&mesh_vert, &mesh_frag], "mesh")
        .unwrap();

    compiler
        .write_root_include(
            env::var("OUT_DIR").unwrap(),
            &[&mesh_vert, &mesh_frag, &mesh_prog],
        )
        .unwrap();
}
