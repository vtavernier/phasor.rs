#[cfg(not(target_arch = "wasm32"))]
pub mod desktop;
#[cfg(target_arch = "wasm32")]
pub mod web;

use crate::Context;
use std::rc::Rc;

pub mod prelude {
    #[cfg(not(target_arch = "wasm32"))]
    pub use glutin;
}

use prelude::*;

pub trait Demo {
    type State;
    type Error;

    fn init(&mut self, gl: &Rc<Context>) -> Result<Self::State, Self::Error>;
    fn render(&mut self, gl: &Rc<Context>, state: &mut Self::State);

    #[cfg(not(target_arch = "wasm32"))]
    fn run_loop(
        &mut self,
        windowed_context: &glutin::WindowedContext<glutin::PossiblyCurrent>,
        gl: &Rc<crate::Context>,
        state: &mut Self::State,
        event: glutin::event::Event<()>,
        _target: &glutin::event_loop::EventLoopWindowTarget<()>,
        control_flow: &mut glutin::event_loop::ControlFlow,
    ) {
        use glutin::event::{Event, WindowEvent};
        use glutin::event_loop::ControlFlow;

        *control_flow = ControlFlow::Wait;

        match event {
            Event::LoopDestroyed => return,
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(physical_size) => windowed_context.resize(physical_size),
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => (),
            },
            Event::RedrawRequested(_) => {
                // Render demo
                self.render(&gl, state);
                windowed_context.swap_buffers().unwrap();
            }
            _ => (),
        }
    }

    fn title(&self) -> String {
        "tinygl demo".to_owned()
    }
}
