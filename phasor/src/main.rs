use tinygl::prelude::*;

use std::rc::Rc;

use glutin::event::{ElementState, Event, VirtualKeyCode, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::window::{Fullscreen, WindowBuilder};
use glutin::ContextBuilder;

use phasor::*;

fn main() -> Result<(), String> {
    phasor::log::init();

    let el = EventLoop::new();

    let wb = WindowBuilder::new()
        .with_title("phasor.rs")
        .with_inner_size(glutin::dpi::LogicalSize::new(768.0, 768.0));

    let windowed_context = ContextBuilder::new()
        .with_gl(glutin::GlRequest::Specific(glutin::Api::OpenGl, (4, 6)))
        .with_gl_profile(glutin::GlProfile::Core)
        .with_gl_debug_flag(true)
        .with_vsync(true)
        .build_windowed(wb, &el)
        .unwrap();

    let (gl, windowed_context) = unsafe {
        let current = windowed_context
            .make_current()
            .expect("failed to make window context current");
        (
            Rc::new(tinygl::Context::from_loader_function(|s| {
                current.get_proc_address(s) as *const _
            })),
            current,
        )
    };

    // Build and bind an empty VAO
    let _vao = unsafe {
        let vao_name = gl.create_vertex_array()?;
        gl.bind_vertex_array(Some(vao_name));
        vao_name
    };

    // Initialize demo
    let mut state = State::new(&gl).expect("failed to initialize state");
    let mut params = Params::default();
    params.min_frequency = 1.0;
    params.max_frequency = 4.0;
    params.frequency_mode = phasor::shared::FM_GAUSS as i32;
    params.filter_bandwidth = 3.0 / std::f32::consts::PI.sqrt();
    state.run_init(&gl, &params);

    // Optimization modes
    let mut optimizing = OptimizationMode::None;
    let mut active_mode = OptimizationMode::Optimize;

    // Monitors
    let fullscreen = Some(Fullscreen::Borderless(
        el.available_monitors()
            .nth(0)
            .expect("no avilable monitors"),
    ));

    el.run(move |event, _target, control_flow| {
        // Default behavior: wait for events
        if optimizing.is_active() {
            *control_flow = ControlFlow::Poll;
        } else {
            *control_flow = ControlFlow::Wait;
        }

        match event {
            Event::LoopDestroyed => return,
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput { input, .. } => {
                    input.virtual_keycode.map(|key| {
                        if let ElementState::Pressed = input.state {
                            match key {
                                VirtualKeyCode::Space => {
                                    optimizing.toggle(&mut active_mode);
                                }
                                VirtualKeyCode::A => {
                                    optimizing.toggle_and_switch(
                                        &mut active_mode,
                                        OptimizationMode::Average,
                                    );
                                }
                                VirtualKeyCode::O => {
                                    optimizing.toggle_and_switch(
                                        &mut active_mode,
                                        OptimizationMode::Optimize,
                                    );
                                }
                                VirtualKeyCode::Escape => {
                                    *control_flow = ControlFlow::Exit;
                                }
                                VirtualKeyCode::F11 => {
                                    if windowed_context.window().fullscreen().is_some() {
                                        windowed_context.window().set_fullscreen(None);
                                    } else {
                                        windowed_context
                                            .window()
                                            .set_fullscreen(fullscreen.clone());
                                    }
                                }
                                _ => {}
                            }
                        }
                    });
                }
                WindowEvent::Resized(physical_size) => {
                    windowed_context.resize(physical_size);
                    unsafe {
                        gl.viewport(
                            0,
                            0,
                            physical_size.width as i32,
                            physical_size.height as i32,
                        );
                    }
                }
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => {}
            },
            Event::RedrawRequested(_) => {
                // Render demo
                unsafe {
                    // Clear framebuffer
                    gl.clear_color(1.0, 0.0, 1.0, 1.0);
                    gl.clear(tinygl::gl::COLOR_BUFFER_BIT);

                    if optimizing.is_active() {
                        state.run_optimize(&gl, optimizing, 1, &params);
                    }

                    state.run_display(&gl, &params, shared::DM_NOISE as i32);
                }

                windowed_context.swap_buffers().unwrap();
            }
            Event::RedrawEventsCleared => {
                if optimizing.is_active() {
                    windowed_context.window().request_redraw();
                }
            }
            _ => {}
        }
    });
}
