use std::ffi::CString;
use std::rc::Rc;

#[repr(C)]
pub struct Kernel {
    coord_x: f32,
    coord_y: f32,
    frequ: f32,
    phase: f32,
    angle: f32,
    state: f32,
}

use glutin::event_loop::EventLoop;
use glutin::{Context, ContextBuilder, PossiblyCurrent};

use tinygl::prelude::*;

use super::{OptimizationMode, Params, State};

enum ApiContext {
    Unintialized,
    Ready(ApiState),
}

struct ApiState {
    el: EventLoop<()>,
    context: Context<PossiblyCurrent>,
    gl: Rc<tinygl::Context>,
    state: State,
    last_error: Option<CString>,
    grid_size: cgmath::Vector3<i32>,
    kernel_count: i32,
    buffer_main: Vec<f32>,
    buffer_extra: Vec<f32>,
    buffer_kernels: Vec<f32>,
}

impl ApiState {
    #[cfg(target_os = "linux")]
    fn get_event_loop() -> EventLoop<()> {
        glutin::platform::unix::EventLoopExtUnix::new_any_thread()
    }

    #[cfg(not(target_os = "linux"))]
    fn get_event_loop() -> EventLoop<()> {
        EventLoop::new()
    }

    fn new() -> Result<Self, String> {
        let el = Self::get_event_loop();

        let sz = glutin::dpi::PhysicalSize::new(512, 512);

        let headless_context = ContextBuilder::new()
            .with_gl(glutin::GlRequest::Specific(glutin::Api::OpenGl, (4, 6)))
            .with_gl_profile(glutin::GlProfile::Core)
            .with_gl_debug_flag(true)
            .build_headless(&el, sz)
            .expect("failed to initialize context");

        let (gl, headless_context) = unsafe {
            let headless_context = headless_context
                .make_current()
                .expect("failed to make context current");

            (
                Rc::new(tinygl::Context::from_loader_function(|s| {
                    headless_context.get_proc_address(s) as *const _
                })),
                headless_context,
            )
        };

        // Build an empty VAO for quad rendering
        let _vao = unsafe {
            let vao_name = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(vao_name));
            vao_name
        };

        let state = State::new(&gl)?;

        Ok(Self {
            el,
            context: headless_context,
            gl,
            state,
            last_error: None,
            grid_size: cgmath::vec3(0, 0, 0),
            kernel_count: 0,
            buffer_main: Vec::new(),
            buffer_extra: Vec::new(),
            buffer_kernels: Vec::new(),
        })
    }
}

impl ApiContext {
    fn ensure_init(&mut self) -> &mut ApiState {
        match self {
            Self::Unintialized => {
                crate::log::init();
                *self = Self::Ready(ApiState::new().expect("failed to initialize api"));
            }
            _ => {}
        }

        match self {
            Self::Ready(state) => state,
            _ => unreachable!(),
        }
    }

    fn if_init(&mut self) -> Option<&mut ApiState> {
        match self {
            Self::Ready(state) => Some(state),
            _ => None,
        }
    }

    fn terminate(&mut self) {
        *self = Self::Unintialized;
    }
}

static mut CURRENT_CONTEXT: ApiContext = ApiContext::Unintialized;

#[no_mangle]
pub extern "C" fn pg_init(hide_window: bool) {
    if !hide_window {
        panic!("phasor.rs doesn't support windowed library usage");
    }

    unsafe { CURRENT_CONTEXT.ensure_init() };
}

#[no_mangle]
pub extern "C" fn pg_terminate() {
    unsafe { CURRENT_CONTEXT.terminate() };
}

#[no_mangle]
pub extern "C" fn pg_optimize_ex(
    width: i32,
    height: i32,
    kernel_count: i32,
    seed: i32,
    iterations: i32,
    angle_mode: i32,
    angle_offset: f32,
    angle_bandwidth: f32,
    angle_range: f32,
    frequency_mode: i32,
    frequency_min: f32,
    frequency_max: f32,
    frequency_bandwidth: f32,
    noise_bandwidth: f32,
    filter_bandwidth: f32,
    filter_modulation: f32,
    filter_modpower: f32,
    isotropy_mode: i32,
    isotropy_min: f32,
    isotropy_max: f32,
    isotropy_bandwidth: f32,
    isotropy_modulation: f32,
    isotropy_power: f32,
    cell_mode: i32,
    opt_method: i32,
    display_mode: i32,
    init_kernels: bool,
) -> *const f32 {
    let api_state = unsafe { CURRENT_CONTEXT.ensure_init() };
    let state = &mut api_state.state;

    let params = Params {
        angle_bandwidth,
        angle_mode,
        angle_offset,
        angle_range,
        cell_mode,
        frequency_bandwidth,
        frequency_mode,
        global_seed: seed,
        isotropy_bandwidth,
        isotropy_mode,
        isotropy_power,
        max_frequency: frequency_max,
        min_frequency: frequency_min,
        max_isotropy: isotropy_max,
        min_isotropy: isotropy_min,
        noise_bandwidth,
        filter_bandwidth,
        isotropy_modulation,
        filter_mod_power: filter_modpower,
        filter_modulation,
        kernel_count: kernel_count as u32,
        grid_size: Params::compute_grid_size(noise_bandwidth),
    };

    // Remember grid size change
    api_state.grid_size = params.grid_size;
    api_state.kernel_count = params.kernel_count as i32;

    let mode = OptimizationMode::from(opt_method);

    if init_kernels {
        state.run_init(&api_state.gl, &params);
    }

    if iterations > 0 {
        state.run_optimize(&api_state.gl, mode, iterations as u32, &params);
    }

    // TODO: Errors could happen here
    state.render_to_texture(
        &api_state.gl,
        width as u32,
        height as u32,
        display_mode,
        &params,
        &mut api_state.buffer_main,
        &mut api_state.buffer_extra,
    );

    // No error occurred
    api_state.last_error = None;

    api_state.buffer_main.as_ptr()
}

#[no_mangle]
pub extern "C" fn pg_get_extra() -> *const f32 {
    unsafe {
        CURRENT_CONTEXT
            .if_init()
            .map(|api_state| api_state.buffer_extra.as_ptr())
            .unwrap_or(std::ptr::null())
    }
}

#[no_mangle]
pub extern "C" fn pg_noise_kernel_width(
    width: i32,
    noise_bandwidth: f32,
    filter_bandwidth: f32,
) -> f32 {
    use std::f32::consts::PI;
    let xsize = unsafe {
        CURRENT_CONTEXT
            .if_init()
            .map(|api_state| api_state.grid_size.x)
            .unwrap_or(0)
    };

    let b = if filter_bandwidth > 0.0 {
        noise_bandwidth.powi(2) / (noise_bandwidth.powi(2) + filter_bandwidth.powi(2)).sqrt()
    } else {
        noise_bandwidth
    };

    (-(0.05f32.ln()) / PI).sqrt() / b * xsize as f32 / width as f32
}

#[no_mangle]
pub extern "C" fn pg_gauss_kernel_width(width: i32, bandwidth: f32) -> f32 {
    use std::f32::consts::PI;
    let xsize = unsafe {
        CURRENT_CONTEXT
            .if_init()
            .map(|api_state| api_state.grid_size.x)
            .unwrap_or(0)
    };

    (-(0.05f32.ln()) / PI).sqrt() / bandwidth * xsize as f32 / width as f32
}

#[no_mangle]
pub extern "C" fn pg_get_error() -> *const i8 {
    unsafe {
        CURRENT_CONTEXT
            .if_init()
            .and_then(|api_state| api_state.last_error.as_ref())
            .map(|err| err.as_ptr())
            .unwrap_or(std::ptr::null())
    }
}

#[no_mangle]
pub extern "C" fn pg_get_max_kernels() -> i32 {
    super::shared::MAX_K as i32
}

#[no_mangle]
pub extern "C" fn pg_get_kernels(
    grid_x: &mut i32,
    grid_y: &mut i32,
    kernel_count: &mut i32,
) -> *const Kernel {
    unsafe {
        CURRENT_CONTEXT
            .if_init()
            .and_then(|api_state| {
                *grid_x = api_state.grid_size.x;
                *grid_y = api_state.grid_size.y;
                *kernel_count = api_state.kernel_count;

                // Allocate CPU-side buffer that's large enough
                let target_size =
                    (super::shared::NFLOATS as i32 * *grid_x * *grid_y * *kernel_count) as usize;
                if api_state.buffer_kernels.len() < target_size {
                    api_state.buffer_kernels.resize(target_size, 0.0);
                }

                // Bind buffer
                let buf = api_state.state.kernels_buffer();
                buf.bind(&api_state.gl, tinygl::gl::COPY_READ_BUFFER);
                // Copy data to CPU
                api_state.gl.get_buffer_sub_data(
                    tinygl::gl::COPY_READ_BUFFER,
                    0,
                    std::slice::from_raw_parts_mut(
                        api_state.buffer_kernels.as_mut_ptr() as *mut u8,
                        target_size * std::mem::size_of::<f32>(),
                    ),
                );
                // Unbind buffer
                api_state.gl.bind_buffer(tinygl::gl::COPY_READ_BUFFER, None);

                Some(api_state.buffer_kernels.as_ptr() as *const _)
            })
            .unwrap_or(std::ptr::null())
    }
}

#[no_mangle]
pub extern "C" fn pg_set_kernels(
    kernels: *const Kernel,
    grid_x: i32,
    grid_y: i32,
    kernel_count: i32,
) -> bool {
    unsafe {
        CURRENT_CONTEXT
            .if_init()
            .and_then(|api_state| {
                api_state.grid_size = cgmath::vec3(grid_x, grid_y, 1);
                api_state.kernel_count = kernel_count;

                // Bind buffer
                let buf = api_state.state.kernels_buffer();
                buf.bind(&api_state.gl, tinygl::gl::COPY_WRITE_BUFFER);
                // Copy data to CPU
                api_state.gl.buffer_data_u8_slice(
                    tinygl::gl::COPY_WRITE_BUFFER,
                    std::slice::from_raw_parts(
                        kernels as *const u8,
                        std::mem::size_of::<Kernel>() * (grid_x * grid_y * kernel_count) as usize,
                    ),
                    tinygl::gl::DYNAMIC_DRAW,
                );
                // Unbind buffer
                api_state
                    .gl
                    .bind_buffer(tinygl::gl::COPY_WRITE_BUFFER, None);

                Some(true)
            })
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn pg_optimize_ex() {
        super::pg_init(true);

        let params = crate::Params::default();
        super::pg_optimize_ex(
            512,
            512,
            16,
            params.global_seed,
            4,
            params.angle_mode,
            params.angle_offset,
            params.angle_bandwidth,
            params.angle_range,
            params.frequency_mode,
            params.min_frequency,
            params.max_frequency,
            params.frequency_bandwidth,
            params.noise_bandwidth,
            params.filter_bandwidth,
            params.filter_modulation,
            params.filter_mod_power,
            params.isotropy_mode,
            params.min_isotropy,
            params.max_isotropy,
            params.isotropy_bandwidth,
            params.isotropy_modulation,
            params.isotropy_power,
            params.cell_mode,
            crate::shared::OM_AVERAGE as i32,
            crate::shared::DM_NOISE as i32,
            true,
        );
    }
}
