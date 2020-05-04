use super::{shaders, shared};

use std::rc::Rc;

const DEFAULT_BANDWIDTH: f32 = 1.692568750643269; // 3.0 / sqrt(M_PI)

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

    pub fn apply_shared(
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

    pub fn apply_global(&self, gl: &Rc<tinygl::Context>, program: &impl shaders::GlobalUniformSet) {
        program.set_u_cell_mode(&gl, self.cell_mode);
        program.set_u_grid(&gl, self.grid_size);
        program.set_u_kernel_count(&gl, self.kernel_count);
    }
}
