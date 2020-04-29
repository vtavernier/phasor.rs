use std::path::Path;
use std::str::FromStr;

use gcode::*;
use itertools::Itertools;
use lazy_static::lazy_static;
use ndarray::par_azip;
use ndarray::prelude::*;
use rand::{Rng, SeedableRng};
use regex::Regex;

use super::param_field::ParamField;
use super::utils::BoundingBox;

#[derive(Debug, Clone)]
struct Segment {
    start: nalgebra::Vector3<f32>,
    end: nalgebra::Vector3<f32>,
    state: State,
}

#[derive(Debug, Clone, Copy, Default)]
struct State {
    fan: u8,
    f: f32,
    line: usize,
    layer: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default)]
struct GlobalState {
    nozzle_diameter: f32,
}

lazy_static! {
    static ref PARAMETER_REGEX: Regex = Regex::new(r"^; ([a-z0-9_]*) :\s*(.*)$").unwrap();
}

pub fn voxelize(
    path: &Path,
    geometry_bounding_box: Option<BoundingBox<f32>>,
    samples: usize,
) -> Result<ParamField, failure::Error> {
    // Parse gcode
    let gcode_src = std::fs::read_to_string(path)?;
    let mut gcode_lines = gcode_src.lines().enumerate();
    let gcode = gcode::parse(&gcode_src);

    let mut current_x = None;
    let mut current_y = None;
    let mut current_z = None;

    let mut current_state = State::default();
    let mut global_state = GlobalState::default();
    let mut segments = Vec::new();

    let mut gcode_current_line: isize = -1;
    let mut current_layer = 0;

    for part in gcode {
        current_state.line = part.span().line;

        while gcode_current_line != current_state.line as isize {
            if let Some((line_idx, line)) = gcode_lines.next() {
                if line == "; <layer>" {
                    current_state.layer = Some(current_layer);
                } else if line == "; </layer>" {
                    current_state.layer = None;
                    current_layer += 1;
                } else if let Some(captures) = PARAMETER_REGEX.captures(line) {
                    match captures.get(1).map(|m| m.as_str()) {
                        Some("nozzle_diameter_mm_0") => {
                            global_state.nozzle_diameter =
                                f32::from_str(captures.get(2).unwrap().as_str())?;
                        }
                        _ => {}
                    }
                }

                gcode_current_line = line_idx as isize;
            }
        }

        match part.mnemonic() {
            Mnemonic::General => {
                match part.major_number() {
                    1 => {
                        let x_arg = part
                            .arguments()
                            .iter()
                            .find(|arg| arg.letter == 'X')
                            .map(|arg| arg.value);
                        let y_arg = part
                            .arguments()
                            .iter()
                            .find(|arg| arg.letter == 'Y')
                            .map(|arg| arg.value);
                        let z_arg = part
                            .arguments()
                            .iter()
                            .find(|arg| arg.letter == 'Z')
                            .map(|arg| arg.value);
                        let e_arg = part
                            .arguments()
                            .iter()
                            .find(|arg| arg.letter == 'E')
                            .map(|arg| arg.value);
                        let f_arg = part
                            .arguments()
                            .iter()
                            .find(|arg| arg.letter == 'F')
                            .map(|arg| arg.value);

                        if let (Some(current_x), Some(current_y), Some(current_z)) =
                            (current_x, current_y, current_z)
                        {
                            let new_x = x_arg.unwrap_or(current_x);
                            let new_y = y_arg.unwrap_or(current_y);
                            let new_z = z_arg.unwrap_or(current_z);

                            // Update filament speed
                            current_state.f = f_arg.unwrap_or(current_state.f);

                            if let Some(e) = e_arg {
                                if e > 0.0 {
                                    // We are extruding a segment
                                    segments.push(Segment {
                                        start: nalgebra::Vector3::new(
                                            current_x, current_y, current_z,
                                        ),
                                        end: nalgebra::Vector3::new(new_x, new_y, new_z),
                                        state: current_state,
                                    })
                                }
                            }
                        }

                        current_x = x_arg.map_or_else(|| current_x, |x| Some(x));
                        current_y = y_arg.map_or_else(|| current_y, |y| Some(y));
                        current_z = z_arg.map_or_else(|| current_z, |z| Some(z));
                    }
                    _ => {}
                }
            }
            Mnemonic::Miscellaneous => match part.major_number() {
                106 => {
                    current_state.fan = part
                        .arguments()
                        .iter()
                        .find(|arg| arg.letter == 'S')
                        .map(|arg| arg.value as u8)
                        .unwrap_or(0);
                }
                _ => {}
            },
            _ => {}
        }
    }

    // Skip the first layer because of the supports, but extend it after
    let printer_bbox = BoundingBox::from(
        &mut segments
            .iter()
            .filter(|seg| seg.state.layer.map(|l| l > 0).unwrap_or(false))
            .map(|seg| (&seg.start, &seg.end))
            .into_iter(),
    );

    let offset = geometry_bounding_box
        .as_ref()
        .map_or_else(|| nalgebra::Vector3::zeros(), BoundingBox::center);

    let printer_bbox = BoundingBox {
        min_x: printer_bbox.min_x - global_state.nozzle_diameter / 2.0,
        min_y: printer_bbox.min_y - global_state.nozzle_diameter / 2.0,
        min_z: printer_bbox.min_z - 2.0 * global_state.nozzle_diameter / 2.0,
        max_x: printer_bbox.max_x + global_state.nozzle_diameter / 2.0,
        max_y: printer_bbox.max_y + global_state.nozzle_diameter / 2.0,
        max_z: printer_bbox.max_z + global_state.nozzle_diameter / 2.0,
    };

    let bbox_min =
        nalgebra::Vector3::new(printer_bbox.min_x, printer_bbox.min_y, printer_bbox.min_z);
    let bbox_size = printer_bbox.size();

    debug!(
        "extracted {} line segments from gcode over {} layers",
        segments.len(),
        current_layer
    );
    debug!("printing bounding box: {:?}", printer_bbox);

    // One cell per layer
    let zc = current_layer;
    let xc = zc;
    let yc = zc;

    let c = nalgebra::Vector3::new(xc as f32, yc as f32, zc as f32);

    // Turn segment list into list of per-layer segments
    let mut segarray: ndarray::Array1<Vec<&Segment>> = ndarray::Array1::default((zc,));
    for (key, iter) in &segments
        .iter()
        .filter(|seg| seg.state.layer.is_some())
        .group_by(|seg| seg.state.layer.unwrap())
    {
        segarray[key].extend(iter);
    }

    // Allocate voxel grid
    let mut vx = ndarray::Array3::<u8>::zeros((zc, yc, xc));

    let nozzle_dimensions = nalgebra::Vector2::new(
        global_state.nozzle_diameter / bbox_size[0] * xc as f32 / 2.0,
        global_state.nozzle_diameter / bbox_size[1] * yc as f32 / 2.0,
    );

    par_azip!((index k, mut vx_layer in vx.outer_iter_mut(), layer_segs in &segarray) {
        for seg in layer_segs {
            // We only process horizontal segments in the current layer
            assert!(seg.start[2] == seg.end[2]);

            // Convert end and start point into voxel coordinates
            let start = (seg.start - bbox_min).component_div(&bbox_size).component_mul(&c);
            let end = (seg.end - bbox_min).component_div(&bbox_size).component_mul(&c);

            let d = end - start;
            let d = nalgebra::Vector2::new(d[0], d[1]);

            let normal_vec = if d[1].abs() > d[0].abs() {
                nalgebra::Vector2::new(-d[1], d[0]).normalize()
            } else {
                nalgebra::Vector2::new(d[1], -d[0]).normalize()
            };

            let start = nalgebra::Vector2::new(start[0], start[1]);
            let end = nalgebra::Vector2::new(end[0], end[1]);

            let j_min = (if start[1] < end[1] {
                start[1] - nozzle_dimensions[1]
            } else {
                end[1] - nozzle_dimensions[1]
            }.floor() as isize).max(0).min((yc - 1) as isize) as usize;

            let j_max = (if start[1] < end[1] {
                end[1] + nozzle_dimensions[1]
            } else {
                start[1] + nozzle_dimensions[1]
            }.ceil() as isize).max(0).min((yc - 1) as isize) as usize;

            let i_min = (if start[0] < end[0] {
                start[0] - nozzle_dimensions[0]
            } else {
                end[0] - nozzle_dimensions[0]
            }.floor() as isize).max(0).min((xc - 1) as isize) as usize;

            let i_max = (if start[0] < end[0] {
                end[0] + nozzle_dimensions[0]
            } else {
                start[0] + nozzle_dimensions[0]
            }.ceil() as isize).max(0).min((xc - 1) as isize) as usize;

            for j in j_min..=j_max {
                for i in i_min..=i_max {
                    let v = vx_layer.get_mut((j, i)).ok_or_else(|| failure::err_msg(format!("out of bounds: ({}, {})", i, j))).unwrap();

                    let x = i as f32 + 0.5;
                    let y = j as f32 + 0.5;

                    let mut in_samples = 0;
                    let mut rnd = rand::rngs::SmallRng::seed_from_u64((k * yc * xc + j * xc + i) as u64);

                    for l in 0..samples {
                        let (x, y) = if l == 0 {
                            (x, y) // middle for first sample
                        } else {
                            (
                                x + rnd.gen_range(-0.5, 0.5),
                                y + rnd.gen_range(-0.5, 0.5),
                            )
                        };

                        // Sample location
                        let p = nalgebra::Vector2::new(x, y);

                        // Compute projection of sample onto segment
                        let s = (p - start).dot(&d) / d.dot(&d);
                        let proj = start + s * (end - start);

                        let is_in = if s > 1.0 {
                            // Outside end of segment
                            (p - end).component_div(&nozzle_dimensions).norm() < 1.0
                        } else if s < 0.0 {
                            // Outside start of segment
                            (p - start).component_div(&nozzle_dimensions).norm() < 1.0
                        } else {
                            ((p - proj).dot(&normal_vec) * normal_vec).component_div(&nozzle_dimensions).norm() < 1.0
                        };

                        if is_in {
                            in_samples += 1;
                        }
                    }

                    *v = v.saturating_add(((in_samples as f32 / samples as f32) * 255.0) as u8);
                }
            }
        }
    });

    Ok(ParamField::new_u8(printer_bbox, vx))
}
