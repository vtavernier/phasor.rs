#[macro_use]
extern crate log;

use std::rc::Rc;

use tinygl::prelude::*;
use tinygl::wrappers::GlHandle;

pub mod api;
pub mod shaders;
pub mod shared;

const DEFAULT_BANDWIDTH: f32 = 1.692568750643269; // 3.0 / sqrt(M_PI)

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

impl From<i32> for OptimizationMode {
    fn from(value: i32) -> Self {
        use std::convert::TryFrom;

        match u32::try_from(value) {
            Ok(shared::OM_OPTIMIZE) => Self::Optimize,
            Ok(shared::OM_AVERAGE) => Self::Average,
            _ => Self::None,
        }
    }
}

pub struct State {
    display_program: GlHandle<shaders::DisplayProgram>,
    init_program: GlHandle<shaders::InitProgram>,
    opt_program: GlHandle<shaders::OptProgram>,
    kernels: GlHandle<tinygl::wrappers::Buffer>,
    kernel_texture: GlHandle<tinygl::wrappers::Texture>,
    allocated_size: usize,
    texture_render_target: Option<TextureRenderTarget>,
}

struct TextureRenderTarget {
    framebuffer: GlHandle<tinygl::wrappers::Framebuffer>,
    depthbuffer: GlHandle<tinygl::wrappers::Renderbuffer>,
    texture_main: GlHandle<tinygl::wrappers::Texture>,
    texture_extra: GlHandle<tinygl::wrappers::Texture>,
    current_size: Option<cgmath::Vector2<i32>>,
}

impl TextureRenderTarget {
    fn new(
        gl: &Rc<tinygl::Context>,
        width: u32,
        height: u32,
    ) -> Result<TextureRenderTarget, String> {
        // Create objects
        let mut this = Self {
            framebuffer: GlHandle::new(gl, tinygl::wrappers::Framebuffer::new(gl)?),
            depthbuffer: GlHandle::new(gl, tinygl::wrappers::Renderbuffer::new(gl)?),
            texture_main: GlHandle::new(gl, tinygl::wrappers::Texture::new(gl)?),
            texture_extra: GlHandle::new(gl, tinygl::wrappers::Texture::new(gl)?),
            current_size: None,
        };

        // Initial allocation
        this.alloc(gl, width, height);

        // Don't use mipmaps
        unsafe {
            for tex in [&this.texture_main, &this.texture_extra].iter() {
                tex.bind(gl, tinygl::gl::TEXTURE_2D);
                gl.tex_parameter_i32(
                    tinygl::gl::TEXTURE_2D,
                    tinygl::gl::TEXTURE_MIN_FILTER,
                    tinygl::gl::NEAREST as i32,
                );
                gl.tex_parameter_i32(
                    tinygl::gl::TEXTURE_2D,
                    tinygl::gl::TEXTURE_MAG_FILTER,
                    tinygl::gl::NEAREST as i32,
                );
            }

            gl.bind_texture(tinygl::gl::TEXTURE_2D, None);
        }

        // Setup bindings
        unsafe {
            this.framebuffer.bind(gl, tinygl::gl::FRAMEBUFFER);
            this.framebuffer.renderbuffer(
                gl,
                tinygl::gl::FRAMEBUFFER,
                tinygl::gl::DEPTH_ATTACHMENT,
                Some(&this.depthbuffer),
            );
            this.framebuffer.texture(
                gl,
                tinygl::gl::FRAMEBUFFER,
                tinygl::gl::COLOR_ATTACHMENT0,
                Some(&this.texture_main),
                0,
            );
            this.framebuffer.texture(
                gl,
                tinygl::gl::FRAMEBUFFER,
                tinygl::gl::COLOR_ATTACHMENT1,
                Some(&this.texture_extra),
                0,
            );
            gl.draw_buffers(&[tinygl::gl::COLOR_ATTACHMENT0, tinygl::gl::COLOR_ATTACHMENT1]);
            gl.bind_framebuffer(tinygl::gl::FRAMEBUFFER, None);
        }

        Ok(this)
    }

    fn alloc(&mut self, gl: &Rc<tinygl::Context>, width: u32, height: u32) {
        let new_size = cgmath::vec2(width as i32, height as i32);

        if !self.current_size.map(|cs| cs == new_size).unwrap_or(false) {
            // Setup storage
            unsafe {
                // Depth buffer
                self.depthbuffer.bind(gl);
                gl.renderbuffer_storage(
                    tinygl::gl::RENDERBUFFER,
                    tinygl::gl::DEPTH_COMPONENT,
                    new_size.x,
                    new_size.y,
                );
                gl.bind_renderbuffer(tinygl::gl::RENDERBUFFER, None);

                // Textures
                for tex in [&self.texture_main, &self.texture_extra].iter() {
                    tex.bind(gl, tinygl::gl::TEXTURE_2D);
                    gl.tex_image_2d(
                        tinygl::gl::TEXTURE_2D,
                        0,
                        tinygl::gl::RGBA32F as i32,
                        new_size.x,
                        new_size.y,
                        0,
                        tinygl::gl::RGBA,
                        tinygl::gl::FLOAT,
                        None,
                    );
                }

                gl.bind_texture(tinygl::gl::TEXTURE_2D, None);
            }

            // Update size
            self.current_size = Some(new_size);
        }
    }
}

#[repr(C)]
pub struct Params {
    // Shared params
    pub angle_bandwidth: f32,
    pub angle_mode: i32,
    pub angle_offset: f32,
    pub angle_range: f32,
    pub frequency_bandwidth: f32,
    pub frequency_mode: i32,
    pub global_seed: i32,
    pub isotropy_bandwidth: f32,
    pub isotropy_mode: i32,
    pub isotropy_power: f32,
    pub max_frequency: f32,
    pub min_frequency: f32,
    pub max_isotropy: f32,
    pub min_isotropy: f32,

    // Extra params
    pub noise_bandwidth: f32,
    pub filter_bandwidth: f32,
    pub isotropy_modulation: f32,
    pub filter_mod_power: f32,
    pub filter_modulation: f32,

    // Global params
    pub cell_mode: i32,
    pub kernel_count: u32,
    pub grid_size: cgmath::Vector3<i32>,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            angle_bandwidth: 0.1,
            angle_mode: shared::AM_GAUSS as i32,
            angle_offset: 0.0,
            angle_range: std::f32::consts::PI,
            frequency_bandwidth: 0.1,
            frequency_mode: shared::FM_STATIC as i32,
            global_seed: 171,
            isotropy_bandwidth: 0.1,
            isotropy_mode: shared::IM_ANISOTROPIC as i32,
            isotropy_power: 1.0,
            max_frequency: 4.0,
            min_frequency: 2.0,
            max_isotropy: 1.0,
            min_isotropy: 0.0,
            //
            noise_bandwidth: DEFAULT_BANDWIDTH,
            filter_bandwidth: 0.0,
            isotropy_modulation: 2.0,
            filter_mod_power: 2.0,
            filter_modulation: 2.0,
            //
            kernel_count: 16,
            grid_size: Self::compute_grid_size(DEFAULT_BANDWIDTH),
            cell_mode: shared::CM_CLAMP as i32,
        }
    }
}

impl Params {
    pub fn compute_grid_size(noise_bandwidth: f32) -> cgmath::Vector3<i32> {
        let new_gsz = (32.0f32 / ((-(0.05f32.ln())).sqrt() / noise_bandwidth)).ceil() as i32;
        cgmath::vec3(new_gsz, new_gsz, 1)
    }

    fn apply_shared(
        &self,
        gl: &Rc<tinygl::Context>,
        program: &(impl shaders::SharedUniformSet + shaders::GlobalUniformSet),
    ) {
        self.apply_global(gl, program);
        program.set_u_angle_bandwidth(&gl, self.angle_bandwidth);
        program.set_u_angle_mode(&gl, self.angle_mode);
        program.set_u_angle_offset(&gl, self.angle_offset);
        program.set_u_angle_range(&gl, self.angle_range);
        program.set_u_frequency_bandwidth(&gl, self.frequency_bandwidth);
        program.set_u_frequency_mode(&gl, self.frequency_mode);
        program.set_u_global_seed(&gl, self.global_seed);
        program.set_u_isotropy_bandwidth(&gl, self.isotropy_bandwidth);
        program.set_u_isotropy_mode(&gl, self.isotropy_mode);
        program.set_u_isotropy_power(&gl, self.isotropy_power);
        program.set_u_max_frequency(&gl, self.max_frequency);
        program.set_u_max_isotropy(&gl, self.max_isotropy);
        program.set_u_min_frequency(&gl, self.min_frequency);
        program.set_u_min_isotropy(&gl, self.min_isotropy);
    }

    fn apply_global(&self, gl: &Rc<tinygl::Context>, program: &impl shaders::GlobalUniformSet) {
        program.set_u_cell_mode(&gl, self.cell_mode);
        program.set_u_grid(&gl, self.grid_size);
        program.set_u_kernel_count(&gl, self.kernel_count);
    }
}

impl State {
    pub fn new(gl: &Rc<tinygl::Context>) -> Result<Self, String> {
        // Build demo state
        let mut state = Self {
            display_program: GlHandle::new(gl, shaders::DisplayProgram::build(&gl)?),
            init_program: GlHandle::new(gl, shaders::InitProgram::build(&gl)?),
            opt_program: GlHandle::new(gl, shaders::OptProgram::build(&gl)?),
            kernels: GlHandle::new(gl, tinygl::wrappers::Buffer::new(&gl)?),
            kernel_texture: GlHandle::new(gl, tinygl::wrappers::Texture::new(&gl)?),
            allocated_size: 0,
            texture_render_target: None,
        };

        // Initialize grid
        state
            .check_grid(gl, &Params::default())
            .map_err(|err| format!("OpenGL error: {}", err))?;

        // Setup texture for buffer storage
        unsafe {
            gl.bind_texture(
                tinygl::gl::TEXTURE_BUFFER,
                Some(state.kernel_texture.name()),
            );
            gl.tex_buffer(
                tinygl::gl::TEXTURE_BUFFER,
                tinygl::gl::R32F,
                state.kernels.name(),
            );
            gl.bind_texture(tinygl::gl::TEXTURE_BUFFER, None);
        }

        Ok(state)
    }

    pub fn run_init(&mut self, gl: &Rc<tinygl::Context>, params: &Params) {
        // Check grid status
        self.check_grid(gl, params)
            .expect("failed to allocate grid");

        // Set params
        self.init_program.use_program(gl);
        params.apply_shared(gl, self.init_program.as_ref());

        unsafe {
            // Bind kernel data
            gl.bind_image_texture(
                self.init_program.get_u_kernels_binding(),
                self.kernel_texture.name(),
                0,
                false,
                0,
                tinygl::gl::READ_WRITE,
                tinygl::gl::R32F,
            );

            // Dispatch program
            gl.dispatch_compute(
                params.grid_size.x as u32,
                params.grid_size.y as u32,
                params.grid_size.z as u32,
            );

            gl.memory_barrier(tinygl::gl::TEXTURE_FETCH_BARRIER_BIT);
        }
    }

    pub fn run_optimize(
        &mut self,
        gl: &Rc<tinygl::Context>,
        mode: OptimizationMode,
        steps: u32,
        params: &Params,
    ) {
        if !mode.is_active() {
            warn!("invalid optimization mode: {:?}", mode);
            return;
        }

        if steps < 1 {
            warn!("invalid optimization step count: {:?}", steps);
            return;
        }

        // Check grid status
        self.check_grid(gl, params)
            .expect("failed to allocate grid");

        // Run one optimization pass
        self.opt_program.use_program(gl);
        params.apply_global(gl, self.opt_program.as_ref());
        self.opt_program
            .set_u_noise_bandwidth(gl, params.noise_bandwidth);
        self.opt_program.set_u_opt_method(gl, mode.as_mode());
        self.opt_program.set_u_opt_steps(gl, steps);

        unsafe {
            // Bind kernel data
            gl.bind_image_texture(
                self.opt_program.get_u_kernels_binding(),
                self.kernel_texture.name(),
                0,
                false,
                0,
                tinygl::gl::READ_WRITE,
                tinygl::gl::R32F,
            );

            gl.dispatch_compute(
                (params.grid_size.x * params.grid_size.y * params.grid_size.z) as u32,
                1,
                1,
            );

            gl.memory_barrier(tinygl::gl::TEXTURE_FETCH_BARRIER_BIT);
        }
    }

    pub fn run_display(&mut self, gl: &Rc<tinygl::Context>, params: &Params, display_mode: i32) {
        // Check grid status
        self.check_grid(gl, params)
            .expect("failed to allocate grid");

        self.display_program.use_program(gl);
        params.apply_shared(gl, self.display_program.as_ref());
        self.display_program
            .set_u_filter_modulation(gl, params.filter_modulation);
        self.display_program
            .set_u_filter_mod_power(gl, params.filter_mod_power);
        self.display_program
            .set_u_isotropy_modulation(gl, params.isotropy_modulation);
        self.display_program
            .set_u_noise_bandwidth(gl, params.noise_bandwidth);
        self.display_program
            .set_u_filter_bandwidth(gl, params.filter_bandwidth);
        self.display_program.set_u_display_mode(gl, display_mode);

        unsafe {
            // Bind kernel data
            gl.bind_image_texture(
                self.display_program.get_u_kernels_binding(),
                self.kernel_texture.name(),
                0,
                false,
                0,
                tinygl::gl::READ_WRITE,
                tinygl::gl::R32F,
            );

            // Draw current program
            gl.draw_arrays(tinygl::gl::TRIANGLES, 0, 3);
        }
    }

    pub fn render_to_texture(
        &mut self,
        gl: &Rc<tinygl::Context>,
        width: u32,
        height: u32,
        display_mode: i32,
        params: &Params,
        buffer_main: &mut Vec<f32>,
        buffer_extra: &mut Vec<f32>,
    ) {
        // Prepare render target
        let trt = {
            if self.texture_render_target.is_none() {
                self.texture_render_target = Some(
                    TextureRenderTarget::new(gl, width, height)
                        .expect("failed to create render target"),
                );
            }

            self.texture_render_target.as_mut().unwrap()
        };

        trt.alloc(gl, width, height);

        // Set target framebuffer
        trt.framebuffer.bind(gl, tinygl::gl::FRAMEBUFFER);

        unsafe {
            // Set viewport
            gl.viewport(0, 0, width as i32, height as i32);

            // Render
            self.run_display(gl, params, display_mode);

            // Render target
            let trt = self.texture_render_target.as_mut().unwrap();

            // Get images
            trt.texture_main.bind(gl, tinygl::gl::TEXTURE_2D);
            buffer_main.resize(
                width as usize * height as usize * std::mem::size_of::<f32>() * 4,
                0.0,
            );
            gl.get_tex_image_u8_slice(
                tinygl::gl::TEXTURE_2D,
                0,
                tinygl::gl::RGBA,
                tinygl::gl::FLOAT,
                Some(std::mem::transmute(&buffer_main[..])),
            );

            trt.texture_extra.bind(gl, tinygl::gl::TEXTURE_2D);
            buffer_extra.resize(
                width as usize * height as usize * std::mem::size_of::<f32>() * 4,
                0.0,
            );
            gl.get_tex_image_u8_slice(
                tinygl::gl::TEXTURE_2D,
                0,
                tinygl::gl::RGBA,
                tinygl::gl::FLOAT,
                Some(std::mem::transmute(&buffer_extra[..])),
            );
        }

        // Cleanup
        unsafe {
            gl.bind_framebuffer(tinygl::gl::FRAMEBUFFER, None);
        }
    }

    fn check_grid(&mut self, gl: &Rc<tinygl::Context>, params: &Params) -> Result<(), u32> {
        let new_alloc_size = shared::NFLOATS as usize
            * std::mem::size_of::<f32>()
            * (params.grid_size.x * params.grid_size.y * params.grid_size.z) as usize
            * params.kernel_count as usize;

        if new_alloc_size > self.allocated_size {
            info!(
                "reallocating for grid_size: {:?}, kernel_count: {}, bytes: {}",
                params.grid_size, params.kernel_count, bytesize::ByteSize(new_alloc_size as u64)
            );

            // Setup buffer storage
            unsafe {
                gl.bind_buffer(tinygl::gl::TEXTURE_BUFFER, Some(self.kernels.name()));
                gl.buffer_data_size(
                    tinygl::gl::TEXTURE_BUFFER,
                    new_alloc_size as i32,
                    tinygl::gl::DYNAMIC_DRAW,
                );

                // Check allocation errors
                let error = gl.get_error();

                gl.bind_buffer(tinygl::gl::TEXTURE_BUFFER, None);

                if error != tinygl::gl::NO_ERROR {
                    return Err(error);
                }
            }

            // Updated allocated size
            self.allocated_size = new_alloc_size;
        }

        Ok(())
    }
}
