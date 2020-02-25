use std::rc::Rc;

use glutin::event::{Event, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::window::WindowBuilder;
use glutin::ContextBuilder;

pub fn run_boilerplate<T>(mut demo: T)
where
    T: super::Demo + 'static,
    T::Error: std::fmt::Debug,
    T::State: 'static,
{
    env_logger::init();

    let el = EventLoop::new();

    let wb = WindowBuilder::new()
        .with_title(demo.title())
        .with_inner_size(glutin::dpi::LogicalSize::new(768.0, 768.0));

    let windowed_context = ContextBuilder::new()
        .with_gl(glutin::GlRequest::Specific(glutin::Api::OpenGl, (4, 6)))
        .with_gl_profile(glutin::GlProfile::Core)
        .with_gl_debug_flag(true)
        .build_windowed(wb, &el)
        .unwrap();

    let (gl, windowed_context) = unsafe {
        let current = windowed_context
            .make_current()
            .expect("failed to make window context current");
        (
            Rc::new(crate::Context::from_loader_function(|s| {
                current.get_proc_address(s) as *const _
            })),
            current,
        )
    };

    // Setup logging on the context
    unsafe {
        gl.debug_message_callback(|source, message_type, id, severity, message| {
            use crate::gl as Gl;
            let source = match source {
                Gl::DEBUG_SOURCE_API => "opengl::api",
                Gl::DEBUG_SOURCE_WINDOW_SYSTEM => "opengl::window_system",
                Gl::DEBUG_SOURCE_SHADER_COMPILER => "opengl::shader_compiler",
                Gl::DEBUG_SOURCE_THIRD_PARTY => "opengl::third_party",
                Gl::DEBUG_SOURCE_APPLICATION => "opengl::application",
                Gl::DEBUG_SOURCE_OTHER => "opengl::other",
                _ => "opengl::unknown",
            };

            let level = match severity {
                Gl::DEBUG_SEVERITY_HIGH => log::Level::Error,
                Gl::DEBUG_SEVERITY_MEDIUM => log::Level::Warn,
                Gl::DEBUG_SEVERITY_LOW => log::Level::Info,
                Gl::DEBUG_SEVERITY_NOTIFICATION => log::Level::Debug,
                _ => log::Level::Trace,
            };

            let message_type = match message_type {
                Gl::DEBUG_TYPE_ERROR => "error",
                Gl::DEBUG_TYPE_DEPRECATED_BEHAVIOR => "deprecated behavior",
                Gl::DEBUG_TYPE_UNDEFINED_BEHAVIOR => "undefined behavior",
                Gl::DEBUG_TYPE_PORTABILITY => "portability",
                Gl::DEBUG_TYPE_PERFORMANCE => "performance",
                Gl::DEBUG_TYPE_MARKER => "marker",
                Gl::DEBUG_TYPE_PUSH_GROUP => "push group",
                Gl::DEBUG_TYPE_POP_GROUP => "pop group",
                Gl::DEBUG_TYPE_OTHER => "other",
                _ => "unknown",
            };

            // Create record manually so we can override the module path
            log::logger().log(
                &log::Record::builder()
                    .args(format_args!(
                        "{} ({}): {}",
                        message_type,
                        id,
                        message.to_string_lossy()
                    ))
                    .level(level)
                    .target("opengl")
                    .module_path_static(Some(source))
                    .build(),
            );
        });
    }

    // Initialize demo
    let mut state = demo.init(&gl).expect("failed to initialize demo");

    el.run(move |event, _, control_flow| {
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
                demo.render(&gl, &mut state);
                windowed_context.window().request_redraw();
                windowed_context.swap_buffers().unwrap();
            }
            _ => (),
        }
    });
}
