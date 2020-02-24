#[cfg(not(target_arch = "wasm32"))]
pub mod desktop;
#[cfg(target_arch = "wasm32")]
pub mod web;

use crate::Context;
use std::rc::Rc;

pub trait Demo {
    type State;
    type Error;

    fn init(&mut self, gl: &Rc<Context>) -> Result<Self::State, Self::Error>;
    fn render(&mut self, gl: &Rc<Context>, state: &mut Self::State);

    fn title(&self) -> String {
        "tinygl demo".to_owned()
    }
}
