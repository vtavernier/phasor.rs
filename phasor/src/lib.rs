#[macro_use]
extern crate log;

use std::rc::Rc;

use tinygl::boilerplate::prelude::*;
use tinygl::prelude::*;

#[cfg(target_arch = "wasm32")]
mod web;

pub mod phasor;
pub mod shaders;
pub mod shared;

use phasor::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OptimizationMode {
    None,
    Optimize,
    Average,
}

impl OptimizationMode {
    pub fn as_mode(&self) -> i32 {
        match self {
            Self::None => -1,
            Self::Optimize => shared::OM_OPTIMIZE as i32,
            Self::Average => shared::OM_AVERAGE as i32,
        }
    }

    pub fn toggle_and_switch(
        &mut self,
        active_mode: &mut OptimizationMode,
        target_mode: OptimizationMode,
    ) {
        *active_mode = target_mode;
        if *self == target_mode {
            *self = Self::None;
        } else {
            *self = target_mode;
        }
    }

    pub fn toggle(&mut self, active_mode: &mut OptimizationMode) {
        match self {
            Self::None => *self = *active_mode,
            Self::Optimize | Self::Average => {
                *active_mode = *self;
                *self = Self::None;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        match self {
            Self::None => false,
            _ => true,
        }
    }
}

impl Default for OptimizationMode {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Default)]
pub struct Demo {
    optimizing: OptimizationMode,
    active_mode: OptimizationMode,
}

impl tinygl::boilerplate::Demo for Demo {
    type State = State;
    type Error = String;

    fn init(&mut self, gl: &Rc<tinygl::Context>) -> Result<State, String> {
        // Build and bind an empty VAO
        let _vao = unsafe {
            let vao_name = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(vao_name));
            vao_name
        };

        State::new(gl)
    }

    fn render(&mut self, gl: &Rc<tinygl::Context>, state: &mut State) {
        unsafe {
            // Clear framebuffer
            gl.clear_color(1.0, 0.0, 1.0, 1.0);
            gl.clear(tinygl::gl::COLOR_BUFFER_BIT);

            if self.optimizing.is_active() {
                state.run_optimize(gl, self.optimizing);
            }

            state.run_display(gl);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn run_loop(
        &mut self,
        windowed_context: &glutin::WindowedContext<glutin::PossiblyCurrent>,
        gl: &Rc<tinygl::Context>,
        state: &mut Self::State,
        event: glutin::event::Event<()>,
        _target: &glutin::event_loop::EventLoopWindowTarget<()>,
        control_flow: &mut glutin::event_loop::ControlFlow,
    ) {
        use glutin::event::{Event, WindowEvent};
        use glutin::event_loop::ControlFlow;

        // Default behavior: wait for events
        if self.optimizing.is_active() {
            *control_flow = ControlFlow::Poll;
        } else {
            *control_flow = ControlFlow::Wait;
        }

        match event {
            Event::LoopDestroyed => return,
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(physical_size) => windowed_context.resize(physical_size),
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::KeyboardInput { input, .. } => {
                    input.virtual_keycode.map(|key| {
                        if let glutin::event::ElementState::Pressed = input.state {
                            match key {
                                glutin::event::VirtualKeyCode::Space => {
                                    self.optimizing.toggle(&mut self.active_mode);
                                }
                                glutin::event::VirtualKeyCode::A => {
                                    self.optimizing.toggle_and_switch(
                                        &mut self.active_mode,
                                        OptimizationMode::Average,
                                    );
                                }
                                glutin::event::VirtualKeyCode::O => {
                                    self.optimizing.toggle_and_switch(
                                        &mut self.active_mode,
                                        OptimizationMode::Optimize,
                                    );
                                }
                                _ => {}
                            }
                        }
                    });
                }
                _ => (),
            },
            Event::RedrawRequested(_) => {
                // Render demo
                self.render(&gl, state);
                windowed_context.swap_buffers().unwrap();
            }
            Event::RedrawEventsCleared => {
                if self.optimizing.is_active() {
                    windowed_context.window().request_redraw();
                }
            }
            _ => (),
        }
    }

    fn title(&self) -> String {
        "phasor.rs".to_owned()
    }
}
