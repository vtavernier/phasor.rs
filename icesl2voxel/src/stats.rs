use ndarray::par_azip;
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;
//use rand::{Rng, SeedableRng};

use super::param_field::ParamField;

pub struct OutputStats {
    pub mean_field: ParamField,
    pub mean_field_confidence: ParamField,
    pub dir_field: ParamField,
    pub dir_length_field: ParamField,
    pub dir_change_field: ParamField,
    pub dir_correlation: Option<ParamField>,
}

pub fn compute_output_stats(
    voxelized_field: &ParamField,
    input_mask: &ParamField,
    input_dir: Option<&ParamField>,
    kernel_size_mm: f32,
    dir_samples: usize,
) -> Result<OutputStats, failure::Error> {
    let vx = voxelized_field.as_u8().unwrap();
    let im = input_mask.as_u8().unwrap();

    let dim = vx.dim();

    let size = voxelized_field.field_box_mm.size();
    let scale = nalgebra::Vector3::new(
        dim.2 as f32 / size.x,
        dim.1 as f32 / size.y,
        dim.0 as f32 / size.z,
    );

    let kernel_size_mm = kernel_size_mm / 2.0;
    let kernel_offset_mm = nalgebra::Vector3::new(kernel_size_mm, kernel_size_mm, kernel_size_mm);

    let cell_count = 2. * kernel_offset_mm.component_mul(&scale);
    debug!("kernel size in cells: {:?}", cell_count);

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
        let x_min = kernel_min.x.ceil().min((dim.2 - 1) as f32).max(0.) as usize;
        let x_max = kernel_max.x.floor().min((dim.2 - 1) as f32).max(0.) as usize;

        (
            nalgebra::Vector3::new(x_min, y_min, z_min),
            nalgebra::Vector3::new(x_max, y_max, z_max),
        )
    };

    let mut dir_field = ndarray::Array4::<f32>::zeros((dim.0, dim.1, dim.2, 3));
    let mut dir_length_field = ndarray::Array3::<f32>::zeros(dim);
    let mut dir_change_field = ndarray::Array3::<f32>::zeros(dim);

    // Raytracer
    let raytrace = |k: usize, j: usize, i: usize, dir: nalgebra::Vector3<f32>| {
        const THRESHOLD: u8 = 128;

        let start_point: nalgebra::Vector3<f32> =
            nalgebra::convert(nalgebra::Vector3::new(i, j, k));
        let start_point = start_point.add_scalar(0.5).component_div(&scale);

        let untransform = |v: nalgebra::Vector3<f32>| {
            let idx = v.component_mul(&scale).add_scalar(-0.5);
            let idx = (idx.z.floor(), idx.y.floor(), idx.x.floor());

            (idx.0 as isize, idx.1 as isize, idx.2 as isize)
        };

        let mut current_point = start_point;
        let mut out = false;

        while !out {
            let end_point = current_point + kernel_size_mm * dir;

            for (z, y, x) in
                line_drawing::Bresenham3d::new(untransform(start_point), untransform(end_point))
            {
                if z < 0
                    || z >= dim.0 as isize
                    || y < 0
                    || y >= dim.1 as isize
                    || x < 0
                    || x >= dim.2 as isize
                {
                    out = true;
                    break;
                }

                let idx = (z as usize, y as usize, x as usize);
                let cval = vx[idx];
                let cval_im = im[idx];

                if cval >= THRESHOLD || cval_im <= THRESHOLD {
                    out = true;
                    break;
                }

                let np: nalgebra::Vector3<f32> = nalgebra::convert(nalgebra::Vector3::new(x, y, z));
                current_point = np.add_scalar(0.5).component_div(&scale);
            }
        }

        // We only need the norm
        (current_point - start_point).norm()
    };

    let find_max_direction = |k: usize, j: usize, i: usize| {
        // Commented: uniform sampling for direction instead of Halton sequence
        //let mut seed = (k * dim.1 * dim.2 + j * dim.2 + i) as u32;
        //seed = ((seed >> 16) ^ seed).wrapping_mul(0x45d9f3bu32);
        //seed = ((seed >> 16) ^ seed).wrapping_mul(0x45d9f3bu32);
        //seed = (seed >> 16) ^ seed;

        let mut rtheta = halton::Sequence::new(2);
        let mut rphi = halton::Sequence::new(3);
        //let mut rng = rand::rngs::SmallRng::seed_from_u64(seed as u64);

        let mut max_dir = nalgebra::Vector3::new(0.0, 0.0, 0.0);
        let mut max_val = 0.0;
        let mut last_change = 0.0;

        if im[(k, j, i)] > 0 {
            let dirs = [
                nalgebra::Vector3::new(0., 0., 1.),
                nalgebra::Vector3::new(0., 1., 0.),
                nalgebra::Vector3::new(1., 0., 0.),
                nalgebra::Vector3::new(0., 0., -1.),
                nalgebra::Vector3::new(0., -1., 0.),
                nalgebra::Vector3::new(-1., 0., 0.),
            ];

            for dir in dirs.iter().take(dir_samples) {
                let d = raytrace(k, j, i, *dir);
                if d > max_val {
                    last_change = d - max_val;
                    max_dir = *dir;
                    max_val = d;
                }
            }

            for k in 0..(dir_samples.max(dirs.len()) - dirs.len()) {
                let theta = rtheta.next().unwrap() * 2.0 * std::f64::consts::PI;
                let phi = rphi.next().unwrap() * std::f64::consts::PI;
                //let theta = rng.gen_range(-std::f32::consts::PI, std::f32::consts::PI);
                //let phi = rng.gen_range(-std::f32::consts::PI, std::f32::consts::PI);

                let dir = nalgebra::Vector3::new(
                    phi.cos() * -theta.sin(),
                    phi.cos() * -theta.cos(),
                    phi.sin(),
                );

                let dir = nalgebra::convert(dir);

                let d = raytrace(k, j, i, dir);
                if d > max_val {
                    last_change = d - max_val;
                    max_dir = dir;
                    max_val = d;
                }
            }
        }

        (max_dir, max_val, last_change)
    };

    // Raytrace direction
    if dir_samples > 0 {
        let steps = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let total = dim.2 * dim.1 * dim.0;

        let thd = std::thread::spawn({
            let steps = steps.clone();
            let dir_samples = dir_samples as u64;
            move || {
                let mut pb = pbr::ProgressBar::new(dir_samples * total as u64);
                pb.message("raytracing direction: ");
                let mut last = 0;

                while last != total {
                    let q = steps.load(std::sync::atomic::Ordering::Relaxed);
                    pb.add((q - last) as u64 * dir_samples);
                    last = q;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }

                pb.finish();
            }
        });

        par_azip!((index (k, j, i),
                mut ddir in dir_field.lanes_mut(Axis(3)),
                ddl in &mut dir_length_field,
                ddch in &mut dir_change_field,
                m in im) {
            if *m > 0 {
                let (max_dir, max_val, last_change) = find_max_direction(k, j, i);
                ddir[0] = max_dir.x;
                ddir[1] = max_dir.y;
                ddir[2] = max_dir.z;

                *ddl = max_val;
                *ddch = last_change;
            }

            steps.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        thd.join().unwrap();
    }

    let dir_correlation = if let Some(input_dir) = input_dir {
        let input_dir = input_dir.as_vec3().unwrap();
        let mut dir_correlation = ndarray::Array3::<u8>::zeros(dim);

        par_azip!((ddc in &mut dir_correlation,
            in_dir in input_dir.lanes(Axis(3)),
            out_dir in dir_field.lanes(Axis(3))) {
            *ddc = ((in_dir[0] * out_dir[0]
                    + in_dir[1] * out_dir[1]
                    + in_dir[2] * out_dir[2]).abs() * 255.0) as u8;
        });

        Some(ParamField::new_u8(
            voxelized_field.field_box_mm,
            dir_correlation,
        ))
    } else {
        None
    };

    let mut mean_field_a = ndarray::Array3::<f32>::zeros(dim);
    let mut mean_field_b = ndarray::Array3::<f32>::zeros(dim);
    let mut mean_field_confidence_f = ndarray::Array3::<f32>::ones(dim);

    let gauss = |x: usize, i: usize, s: f32| {
        let sigma = kernel_size_mm;
        (-0.5 * ((x as f32 - i as f32) / (s * sigma)).powf(2.0)).exp()
    };

    // Seed A buffer with input
    par_azip!((o in &mut mean_field_a, i in vx, m in im) {
        *o = *i as f32 / 255.0 * *m as f32 / 255.0;
    });

    {
        let src = &mean_field_a;
        let dst = &mut mean_field_b;

        par_azip!((index (k, j, i),
                o in dst,
                c in &mut mean_field_confidence_f) {
            let (min, max) = transform(k, j, i);

            let mut mean = 0.0f32;
            let mut sum = 0.0f32;
            let mut count = 0.0f32;

            for z in min.z..=max.z {
                let w = gauss(z, k, scale.z);
                mean += src[(z, j, i)] * w;
                sum += w;
                count += if im[(z, j, i)] > 0 { w } else { 0.0 };
            }

            *o = mean / sum;
            *c *= count;
        });
    }

    {
        let src = &mean_field_b;
        let dst = &mut mean_field_a;

        par_azip!((index (k, j, i),
                o in dst,
                c in &mut mean_field_confidence_f) {
            let (min, max) = transform(k, j, i);

            let mut mean = 0.0f32;
            let mut sum = 0.0f32;
            let mut count = 0.0f32;

            for y in min.y..=max.y {
                let w = gauss(y, j, scale.y);
                mean += src[(k, y, i)] * w;
                sum += w;
                count += if im[(k, y, i)] > 0 { w } else { 0.0 };
            }

            *o = mean / sum;
            *c *= count;
        });
    }

    {
        let src = &mean_field_a;
        let dst = &mut mean_field_b;

        par_azip!((index (k, j, i),
                o in dst,
                c in &mut mean_field_confidence_f) {
            let (min, max) = transform(k, j, i);

            let mut mean = 0.0f32;
            let mut sum = 0.0f32;
            let mut count = 0.0f32;

            for x in min.x..=max.x {
                let w = gauss(x, i, scale.x);
                mean += src[(k, j, x)] * w;
                sum += w;
                count += if im[(k, j, x)] > 0 { w } else { 0.0 };
            }

            *o = mean / sum;
            *c *= count;
        });
    }

    let mut mean_field = ndarray::Array3::<u8>::zeros(dim);
    let mut mean_field_confidence = ndarray::Array3::<u8>::zeros(dim);

    let count_max = mean_field_confidence_f
        .max()
        .expect("failed to compute maximum field confidence value");

    par_azip!((o in &mut mean_field, i in &mean_field_b, ic in &mean_field_confidence_f, oc in &mut mean_field_confidence, m in im) {
        let m = (*m as f32) / 255.0;
        *o = (m * *i * 255.0).max(0.0).min(255.0) as u8;
        *oc = (m * *ic * 255.0 / count_max).max(0.0).min(255.0) as u8;
    });

    Ok(OutputStats {
        mean_field: ParamField::new_u8(voxelized_field.field_box_mm, mean_field),
        mean_field_confidence: ParamField::new_u8(
            voxelized_field.field_box_mm,
            mean_field_confidence,
        ),
        dir_field: ParamField::new_vec3(voxelized_field.field_box_mm, dir_field),
        dir_length_field: ParamField::new_f32(voxelized_field.field_box_mm, dir_length_field),
        dir_change_field: ParamField::new_f32(voxelized_field.field_box_mm, dir_change_field),
        dir_correlation,
    })
}
