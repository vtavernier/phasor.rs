use ndarray::par_azip;
use ndarray::prelude::*;

use super::param_field::ParamField;

pub fn compute_output_stats(
    voxelized_field: &ParamField,
    input_mask: &ParamField,
    kernel_size_mm: f32,
) -> Result<ParamField, failure::Error> {
    let vx = voxelized_field.as_u8().unwrap();
    let im = input_mask.as_u8().unwrap();

    let dim = vx.dim();
    let mut s = ndarray::Array3::<u8>::zeros(dim);

    let origin = voxelized_field.field_box_mm.min();
    let size = voxelized_field.field_box_mm.size();
    let scale = nalgebra::Vector3::new(
        dim.2 as f32 / size.x,
        dim.1 as f32 / size.y,
        dim.0 as f32 / size.z,
    );
    let kernel_offset_mm = nalgebra::Vector3::new(kernel_size_mm, kernel_size_mm, kernel_size_mm);

    debug!(
        "kernel size in cells: {:?}",
        2. * kernel_offset_mm.component_mul(&scale)
    );

    // Working array
    let mut mean_field = ndarray::Array4::<u64>::zeros((dim.0, dim.1, dim.2, 2));

    // Z mean
    par_azip!((index (k, j, i), mut d in mean_field.lanes_mut(Axis(3))) {
        let center = nalgebra::Vector3::new(i as f32 + 0.5, j as f32 + 0.5, k as f32 + 0.5).component_div(&scale);
        let kernel_min = (center - kernel_offset_mm).component_mul(&scale);
        let kernel_max = (center + kernel_offset_mm).component_mul(&scale);

        let z_min = kernel_min.z.ceil().min((dim.0 - 1) as f32).max(0.) as usize;
        let z_max = kernel_max.z.floor().min((dim.0 - 1) as f32).max(0.) as usize;

        let mut sum = 0u64;
        let mut total = 0u64;

        for z in z_min..=z_max {
            let m = im[(z, j, i)] as u64;
            let v = vx[(z, j, i)] as u64;

            sum += m * v;
            total += m;
        }

        d[0] += sum;
        d[1] += total;
    });

    // Y mean
    par_azip!((index (k, j, i), mut d in mean_field.lanes_mut(Axis(3))) {
        let center = nalgebra::Vector3::new(i as f32 + 0.5, j as f32 + 0.5, k as f32 + 0.5).component_div(&scale);
        let kernel_min = (center - kernel_offset_mm).component_mul(&scale);
        let kernel_max = (center + kernel_offset_mm).component_mul(&scale);

        let y_min = kernel_min.y.ceil().min((dim.1 - 1) as f32).max(0.) as usize;
        let y_max = kernel_max.y.floor().min((dim.1 - 1) as f32).max(0.) as usize;

        let mut sum = 0u64;
        let mut total = 0u64;

        for y in y_min..=y_max {
            let m = im[(k, y, i)] as u64;
            let v = vx[(k, y, i)] as u64;

            sum += m * v;
            total += m;
        }

        d[0] += sum;
        d[1] += total;
    });

    // X mean
    par_azip!((index (k, j, i), mut d in mean_field.lanes_mut(Axis(3))) {
        let center = nalgebra::Vector3::new(i as f32 + 0.5, j as f32 + 0.5, k as f32 + 0.5).component_div(&scale);
        let kernel_min = (center - kernel_offset_mm).component_mul(&scale);
        let kernel_max = (center + kernel_offset_mm).component_mul(&scale);

        let x_min = kernel_min.x.ceil().min((dim.2 - 1) as f32).max(0.) as usize;
        let x_max = kernel_max.x.floor().min((dim.2 - 1) as f32).max(0.) as usize;

        let mut sum = 0u64;
        let mut total = 0u64;

        for x in x_min..=x_max {
            let m = im[(k, j, x)] as u64;
            let v = vx[(k, j, x)] as u64;

            sum += m * v;
            total += m;
        }

        d[0] += sum;
        d[1] += total;
    });

    // Convert back to u8 array
    par_azip!((d in &mut s, m in mean_field.lanes_mut(Axis(3)), i in im) {
        if m[1] > 0 {
            *d = ((*i as u64 * m[0]) / (m[1] * 255)) as u8;
        }
    });

    Ok(ParamField::new_u8(voxelized_field.field_box_mm, s))
}
