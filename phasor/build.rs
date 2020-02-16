use std::env;

fn main() {
    let dest = env::var("OUT_DIR").unwrap();
    let mut compiler = tinygl_compiler::CompilerBuilder::default().build();

    compiler
        .wrap_shader(&dest, "shaders/display.frag", false)
        .unwrap();
    compiler
        .wrap_shader(&dest, "shaders/quad.vert", false)
        .unwrap();
    compiler.write_root_include(&dest).unwrap();
}
