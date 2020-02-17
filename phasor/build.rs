fn main() {
    let mut compiler = tinygl_compiler::CompilerBuilder::default().build().unwrap();

    compiler.wrap_shader("shaders/display.frag").unwrap();
    compiler.wrap_shader("shaders/quad.vert").unwrap();
    compiler.write_root_include().unwrap();
}
