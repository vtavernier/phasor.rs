use ndarray::par_azip;
use ndarray::prelude::*;

use super::param_field::ParamField;

pub fn compute_output_stats(
    voxelized_field: &ParamField,
    input_mask: &ParamField,
    kernel_size_mm: f32,
) -> Result<(ParamField, ParamField), failure::Error> {
    let vx = voxelized_field.as_u8().unwrap();
    let im = input_mask.as_u8().unwrap();

    let dim = vx.dim();

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

    // Coordinate transform
    let transform = |k: usize, j: usize, i: usize| {
        let center = nalgebra::Vector3::new(i as f32 + 0.5, j as f32 + 0.5, k as f32 + 0.5)
            .component_div(&scale);
        let kernel_min = (center - kernel_offset_mm).component_mul(&scale);
        let kernel_max = (center + kernel_offset_mm).component_mul(&scale);

        let z_min = kernel_min.z.ceil().min((dim.0 - 1) as f32).max(0.) as usize;
        let z_max = kernel_max.z.floor().min((dim.0 - 1) as f32).max(0.) as usize;
        let y_min = kernel_min.y.ceil().min((dim.1 - 1) as f32).max(0.) as usize;
        let y_max = kernel_max.y.floor().min((dim.1 - 1) as f32).max(0.) as usize;
        let x_min = kernel_min.y.ceil().min((dim.2 - 1) as f32).max(0.) as usize;
        let x_max = kernel_max.y.floor().min((dim.2 - 1) as f32).max(0.) as usize;

        (
            nalgebra::Vector3::new(x_min, y_min, z_min),
            nalgebra::Vector3::new(x_max, y_max, z_max),
        )
    };

    let mut s = ndarray::Array3::<u8>::zeros(dim);
    let mut m2 = ndarray::Array4::<f32>::zeros((dim.0, dim.1, dim.2, 3));

    par_azip!((index (k, j, i), ds in &mut s, mut dm2 in m2.lanes_mut(Axis(3)), m in im, v in vx) {
        let (min, max) = transform(k, j, i);

        let mut mean = 0.0f32;
        let mut sum = 0.0f32;
        let mut m2x = 0.0f32;
        let mut m2y = 0.0f32;
        let mut m2z = 0.0f32;

        for z in min.z..=max.z {
            if z == k { continue; }

            let m = im[(z, j, i)] as f32 / 255.0;
            let v = vx[(z, j, i)] as f32 / 255.0;

            mean += m * v;
            m2z += m * ((z as f32 + 0.5) / scale.z - (k as f32 + 0.5) / scale.z).powf(2.0) * v;
            sum += m;
        }

        for y in min.y..=max.y {
            if y == j { continue; }

            let m = im[(k, y, i)] as f32 / 255.0;
            let v = vx[(k, y, i)] as f32 / 255.0;

            mean += m * v;
            m2y += m * ((y as f32 + 0.5) / scale.y - (j as f32 + 0.5) / scale.y).powf(2.0) * v;
            sum += m;
        }

        for x in min.x..=max.x {
            if x == i { continue; }

            let m = im[(k, j, x)] as f32 / 255.0;
            let v = vx[(k, j, x)] as f32 / 255.0;

            mean += m * v;
            m2x += m * ((x as f32 + 0.5) / scale.x - (i as f32 + 0.5) / scale.x).powf(2.0) * v;
            sum += m;
        }

        let m = *m as f32 / 255.0;

        if m > 0.0 {
            let v = *v as f32 / 255.0;

            mean += m * v;
            sum += m;
        }

        if sum > 0.0 {
            *ds = (m * mean / sum * 255.0) as u8;

            let v = m * nalgebra::Vector3::new(
                m2x / sum,
                m2y / sum,
                m2z / sum,
            );

            dm2[0] = v.x;
            dm2[1] = v.y;
            dm2[2] = v.z;
        }
    });

    Ok((
        ParamField::new_u8(voxelized_field.field_box_mm, s),
        ParamField::new_vec3(voxelized_field.field_box_mm, m2),
    ))
}
