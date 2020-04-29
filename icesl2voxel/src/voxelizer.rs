use std::path::Path;
use std::str::FromStr;

use gcode::*;
use itertools::Itertools;
use lazy_static::lazy_static;
use ndarray::par_azip;
use ndarray::prelude::*;
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

    let printer_bbox = BoundingBox::from(
        &mut segments
            .iter()
            .filter(|seg| seg.state.layer.is_some())
            .map(|seg| (&seg.start, &seg.end))
            .into_iter(),
    );

    let offset = geometry_bounding_box
        .as_ref()
        .map_or_else(|| nalgebra::Vector3::zeros(), BoundingBox::center);

    let printer_bbox = BoundingBox {
        min_x: printer_bbox.min_x - global_state.nozzle_diameter / 2.0,
        min_y: printer_bbox.min_y - global_state.nozzle_diameter / 2.0,
        min_z: printer_bbox.min_z - global_state.nozzle_diameter / 2.0,
        max_x: printer_bbox.max_x + global_state.nozzle_diameter / 2.0,
        max_y: printer_bbox.max_y + global_state.nozzle_diameter / 2.0,
        max_z: printer_bbox.max_z + global_state.nozzle_diameter / 2.0,
    };

    let bbox_size = printer_bbox.size();

    debug!(
        "extracted {} line segments from gcode over {} layers",
        segments.len(),
        current_layer
    );
    debug!("printing bounding box: {:?}", printer_bbox);
    debug!(
        "memory usage: {}",
        bytesize::ByteSize::b((std::mem::size_of::<Segment>() * segments.len()) as u64)
    );

    // One cell per layer
    let zc = current_layer;
    let xc = zc;
    let yc = zc;

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

    par_azip!((mut vx_layer in vx.outer_iter_mut(), layer_segs in &segarray) {
        for seg in layer_segs {
            // We only process horizontal segments in the current layer
            assert!(seg.start[2] == seg.end[2]);

            // Convert end and start point into voxel coordinates
            let start_x = ((seg.start[0].min(seg.end[0]) - printer_bbox.min_x) / bbox_size.x) * (xc - 1) as f32 + 0.5;
            let start_y = ((seg.start[1].min(seg.end[1]) - printer_bbox.min_y) / bbox_size.y) * (yc - 1) as f32 + 0.5;
            let end_x = ((seg.end[0].max(seg.start[0]) - printer_bbox.min_x) / bbox_size.x) * (xc - 1) as f32 + 0.5;
            let end_y = ((seg.end[1].max(seg.start[1]) - printer_bbox.min_y) / bbox_size.y) * (yc - 1) as f32 + 0.5;

            for ((x, y), value) in line_drawing::XiaolinWu::<f32, i32>::new((start_x, start_y), (end_x, end_y)) {
                if let Some(c) = vx_layer.get_mut((y as usize, x as usize)) {
                    *c = c.saturating_add((value * 255.0) as u8);
                } else {
                    error!("out of bounds: ({}, {}) for segment ({}, {}) -> ({}, {})", x, y, start_x, start_y, end_x, end_y);
                }
            }
        }
    });

    Ok(ParamField::new_u8(printer_bbox, vx))
}
