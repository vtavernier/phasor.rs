#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    tinygl::boilerplate::desktop::run_boilerplate(phasor::Demo::default());
}
