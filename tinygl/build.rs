use gl_generator::{Api, Fallbacks, Profile, Registry, StructGenerator};
use std::env;
use std::fs::File;
use std::path::Path;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let dest = env::var("OUT_DIR").unwrap();

        // Build the OpenGL 4.6 bindings
        let mut file = File::create(&Path::new(&dest).join("bindings.rs")).unwrap();

        Registry::new(Api::Gl, (4, 6), Profile::Core, Fallbacks::All, [])
            .write_bindings(StructGenerator, &mut file)
            .unwrap();
    }
}
