use std::env;
use std::path::PathBuf;

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
    compiler
        .wrap_uniforms(&["init", "display"], "shared")
        .unwrap();
    compiler
        .wrap_uniforms(&["init", "opt", "display"], "global")
        .unwrap();
    compiler.write_root_include().unwrap();

    // Generate wrapper for constants
    println!("cargo:rerun-if-changed=shaders/shared.h");
    let bindings = bindgen::Builder::default()
        .header("shaders/shared.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("unable to generate bindings");

    bindings
        .write_to_file(PathBuf::from(env::var("OUT_DIR").unwrap()).join("shared.rs"))
        .expect("couldn't write bindings");

    // Generate C header for library clients
    cbindgen::Builder::new()
        .with_crate(env::var("CARGO_MANIFEST_DIR").unwrap())
        .generate()
        .expect("unable to generate C bindings")
        .write_to_file(PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("phasoropt.h"));
}
