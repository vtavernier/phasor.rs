#[cfg(not(target_arch = "wasm32"))]
pub mod desktop;
#[cfg(target_arch = "wasm32")]
pub mod web;

use crate::Context;

pub trait Demo<'a> {
    fn init(&mut self, gl: &Context);
    fn render(&mut self, gl: &Context);

    fn title(&self) -> &'a str {
        "tinygl demo"
    }
}
