fn main() {
    let mut compiler = tinygl_compiler::CompilerBuilder::default().build().unwrap();

    compiler.wrap_shader("shaders/display.frag").unwrap();
    compiler.wrap_shader("shaders/display.vert").unwrap();
    compiler.wrap_shader("shaders/init.comp").unwrap();
    compiler.wrap_shader("shaders/opt.comp").unwrap();
    compiler
        .wrap_program(&["shaders/display.vert", "shaders/display.frag"], "display")
        .unwrap();
    compiler
        .wrap_program(&["shaders/init.comp"], "init")
        .unwrap();
    compiler.wrap_program(&["shaders/opt.comp"], "opt").unwrap();
    compiler.wrap_uniforms(&["init", "display"], "shared").unwrap();
    compiler.write_root_include().unwrap();
}
