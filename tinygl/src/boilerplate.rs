#[cfg(not(target_arch = "wasm32"))]
pub mod desktop;
#[cfg(target_arch = "wasm32")]
pub mod web;

use crate::Context;

pub trait Demo {
    type State;
    type Error;

    fn init(&mut self, gl: &Context) -> Result<Self::State, Self::Error>;
    fn render(&mut self, gl: &Context, state: &mut Self::State);

    fn title(&self) -> String {
        "tinygl demo".to_owned()
    }
}
