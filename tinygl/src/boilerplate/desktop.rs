pub fn run_boilerplate<'a, T>(mut demo: T)
where
    T: super::Demo<'a> + 'static,
    T::Error: std::fmt::Debug,
    T::State: 'static
{
    use glutin::event::{Event, WindowEvent};
    use glutin::event_loop::{ControlFlow, EventLoop};
    use glutin::window::WindowBuilder;
    use glutin::ContextBuilder;

    let el = EventLoop::new();

    let wb = WindowBuilder::new()
        .with_title(demo.title())
        .with_inner_size(glutin::dpi::LogicalSize::new(768.0, 768.0));

    let windowed_context = ContextBuilder::new().build_windowed(wb, &el).unwrap();

    let (gl, windowed_context) = unsafe {
        let current = windowed_context
            .make_current()
            .expect("failed to make window context current");
        (
            crate::Context::from_loader_function(|s| current.get_proc_address(s) as *const _),
            current,
        )
    };

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
                windowed_context.swap_buffers().unwrap();
            }
            _ => (),
        }
    });
}
