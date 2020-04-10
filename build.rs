use std::env;
use std::path::PathBuf;

fn main() {
    let mut compiler = tinygl_compiler::CompilerBuilder::new().build().unwrap();

    let display_frag = compiler.wrap_shader("shaders/display.frag").unwrap();
    let display_vert = compiler.wrap_shader("shaders/display.vert").unwrap();
    let init_comp = compiler.wrap_shader("shaders/init.comp").unwrap();
    let opt_comp = compiler.wrap_shader("shaders/opt.comp").unwrap();

    let display_prog = compiler
        .wrap_program(&[&display_vert, &display_frag], "display")
        .unwrap();
    let init_prog = compiler.wrap_program(&[&init_comp], "init").unwrap();
    let opt_prog = compiler.wrap_program(&[&opt_comp], "opt").unwrap();

    let shared_uniforms = compiler
        .wrap_uniforms(&[&init_prog, &display_prog], "shared")
        .unwrap();
    let global_uniforms = compiler
        .wrap_uniforms(&[&init_prog, &opt_prog, &display_prog], "global")
        .unwrap();

    compiler
        .write_root_include(
            env::var("OUT_DIR").unwrap(),
            &[
                &display_frag,
                &display_vert,
                &init_comp,
                &opt_comp,
                &display_prog,
                &init_prog,
                &opt_prog,
                &shared_uniforms,
                &global_uniforms,
            ],
        )
        .unwrap();

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
        .with_config(cbindgen::Config {
            cpp_compat: true,
            language: cbindgen::Language::C,
            includes: vec!["shaders/shared.h".to_owned()],
            ..Default::default()
        })
        .with_crate(env::var("CARGO_MANIFEST_DIR").unwrap())
        .exclude_item("Kernel")
        .generate()
        .expect("unable to generate C bindings")
        .write_to_file(PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("phasoropt.h"));
}
