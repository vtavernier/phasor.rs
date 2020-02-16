#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use tinygl::impl_desktop_demo;
    impl_desktop_demo!(phasor::Demo);
}
