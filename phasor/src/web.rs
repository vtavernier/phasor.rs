// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// This is like the `main` function, except for JavaScript.
#[wasm_bindgen(start)]
pub fn wasm_main() -> Result<(), JsValue> {
    #[cfg(debug_assertions)]
    {
        // This provides better error messages in debug mode.
        console_error_panic_hook::set_once();

        // Set logger
        console_log::init_with_level(log::Level::Debug).unwrap()
    }

    Ok(())
}

use tinygl::impl_web_demo;
impl_web_demo!(super::Demo);
