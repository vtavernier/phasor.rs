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

mod shaders;

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

pub fn voxelize_gcode(path: &Path, samples: usize) -> Result<ParamField, failure::Error> {
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

    let printer_bbox = BoundingBox {
        min_x: printer_bbox.min_x - global_state.nozzle_diameter / 2.0,
        min_y: printer_bbox.min_y - global_state.nozzle_diameter / 2.0,
        min_z: printer_bbox.min_z - 2.0 * global_state.nozzle_diameter / 2.0,
        max_x: printer_bbox.max_x + global_state.nozzle_diameter / 2.0,
        max_y: printer_bbox.max_y + global_state.nozzle_diameter / 2.0,
        max_z: printer_bbox.max_z + global_state.nozzle_diameter / 2.0,
    };

    let bbox_min = nalgebra::Vector3::from(printer_bbox.min());
    let bbox_size = nalgebra::Vector3::from(printer_bbox.size());

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

    let nozzle_dimensions =
        c.xy().component_div(&bbox_size.xy()) * global_state.nozzle_diameter / 2.0;

    par_azip!((index k, mut vx_layer in vx.outer_iter_mut(), layer_segs in &segarray) {
        for seg in layer_segs {
            // We only process horizontal segments in the current layer
            assert!(seg.start.z == seg.end.z);

            // Convert end and start point into voxel coordinates
            let start = (seg.start - bbox_min).component_div(&bbox_size).component_mul(&c).xy();
            let end = (seg.end - bbox_min).component_div(&bbox_size).component_mul(&c).xy();

            let d = end - start;

            let normal_vec = if d.y.abs() > d.x.abs() {
                nalgebra::Vector2::new(-d.y, d.x).normalize()
            } else {
                nalgebra::Vector2::new(d.y, -d.x).normalize()
            };

            let j_min = (if start.y < end.y {
                start.y - nozzle_dimensions.y
            } else {
                end.y - nozzle_dimensions.y
            }.floor() as isize).max(0).min((yc - 1) as isize) as usize;

            let j_max = (if start.y < end.y {
                end.y + nozzle_dimensions.y
            } else {
                start.y + nozzle_dimensions.y
            }.ceil() as isize).max(0).min((yc - 1) as isize) as usize;

            let i_min = (if start.x < end.x {
                start.x - nozzle_dimensions.x
            } else {
                end.x - nozzle_dimensions.x
            }.floor() as isize).max(0).min((xc - 1) as isize) as usize;

            let i_max = (if start.x < end.x {
                end.x + nozzle_dimensions.x
            } else {
                start.x + nozzle_dimensions.x
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

use glutin::event_loop::EventLoop;
use glutin::ContextBuilder;

use tinygl::gl;
use tinygl::prelude::*;

fn render_axis(
    mesh_bbox: &BoundingBox<f32>,
    image_width: usize,
    image_height: usize,
    transform: cgmath::Matrix4<f32>,
    prog: &shaders::MeshProgram,
    gl: &tinygl::Context,
    mesh: &stl_io::IndexedMesh,
) -> Result<(ndarray::Array2<f32>, ndarray::Array2<f32>), failure::Error> {
    let framebuffer = tinygl::wrappers::GlRefHandle::new(
        gl,
        tinygl::wrappers::Framebuffer::new(&gl)
            .map_err(|emsg| failure::err_msg(format!("failed to create framebuffer: {}", emsg)))?,
    );
    framebuffer.bind(&gl, gl::FRAMEBUFFER);

    // Create framebuffer and depth texture
    let depth_texture = tinygl::wrappers::GlRefHandle::new(
        gl,
        tinygl::wrappers::Texture::new(&gl).map_err(|emsg| {
            failure::err_msg(format!("failed to create depth texture: {}", emsg))
        })?,
    );
    depth_texture.bind(&gl, gl::TEXTURE_2D);

    unsafe {
        gl.tex_parameter_i32(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
        gl.tex_parameter_i32(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);

        gl.tex_image_2d(
            gl::TEXTURE_2D,
            0,
            gl::DEPTH_COMPONENT as i32,
            image_width as i32,
            image_height as i32,
            0,
            gl::DEPTH_COMPONENT,
            gl::FLOAT,
            None,
        );
    }

    let v1 = cgmath::vec4(mesh_bbox.min_x, mesh_bbox.min_y, mesh_bbox.min_z, 1.0);
    let v2 = cgmath::vec4(mesh_bbox.max_x, mesh_bbox.max_y, mesh_bbox.max_z, 1.0);

    debug!("original viewport: ({:?}; {:?})", v1, v2);

    let v1 = transform * v1;
    let v2 = transform * v2;

    debug!("transformed viewport: ({:?}; {:?})", v1, v2);
    debug!("transformation: {:?}", transform);

    // Set view matrix
    const OFFSET: f32 = 0.25;
    prog.set_view_matrix(
        &gl,
        false,
        cgmath::ortho(
            v1.x - OFFSET,
            v2.x + OFFSET,
            v1.y - OFFSET,
            v2.y + OFFSET,
            v1.z - OFFSET,
            v2.z + OFFSET,
        ) * transform,
    );

    framebuffer.texture_2d(
        &gl,
        gl::FRAMEBUFFER,
        gl::DEPTH_ATTACHMENT,
        gl::TEXTURE_2D,
        Some(&depth_texture),
        0,
    );

    debug!("framebuffer status: {}", unsafe {
        match gl.check_framebuffer_status(gl::FRAMEBUFFER) {
            gl::FRAMEBUFFER_INCOMPLETE_ATTACHMENT => {
                "GL_FRAMEBUFFER_INCOMPLETE_ATTACHMENT".to_owned()
            }
            gl::FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT => {
                "GL_FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT".to_owned()
            }
            gl::FRAMEBUFFER_UNSUPPORTED => "GL_FRAMEBUFFER_UNSUPPORTED".to_owned(),
            gl::FRAMEBUFFER_COMPLETE => "GL_FRAMEBUFFER_COMPLETE".to_owned(),
            other => format!("{}", other),
        }
    });

    unsafe {
        let draw = || {
            // Clear depth
            gl.clear(gl::DEPTH_BUFFER_BIT);

            // Render
            gl.draw_elements(
                gl::TRIANGLES,
                (3 * mesh.faces.len()) as i32,
                gl::UNSIGNED_INT as u32,
                0,
            );

            // Fetch image
            let mut depth_buf = ndarray::Array2::<f32>::zeros((image_height, image_width));
            gl.get_tex_image_u8_slice(
                gl::TEXTURE_2D,
                0,
                gl::DEPTH_COMPONENT,
                gl::FLOAT,
                Some({
                    let slice = depth_buf.as_slice().unwrap();
                    std::slice::from_raw_parts(
                        slice.as_ptr() as *const _,
                        slice.len() * std::mem::size_of_val(&slice[0]),
                    )
                }),
            );

            // Scale back values
            for val in &mut depth_buf {
                *val = (*val - 0.5) * (1.0 + 2.0 * OFFSET / (v2.z - v1.z).abs()) + 0.5;
            }

            // Invert everything
            depth_buf.invert_axis(Axis(1));

            depth_buf
        };

        // Set viewport
        gl.viewport(0, 0, image_width as i32, image_height as i32);

        // Draw closest points
        gl.depth_func(gl::LEQUAL);
        gl.clear_depth_f32(1.0);

        let buf_plus = draw();

        // Draw furthest points
        gl.depth_func(gl::GEQUAL);
        gl.clear_depth_f32(0.0);

        let buf_minus = draw();

        Ok((buf_plus, buf_minus))
    }
}

fn write_depth_img(
    buf: &ndarray::Array2<f32>,
    dest: impl AsRef<Path>,
) -> Result<(), failure::Error> {
    let (height, width) = buf.dim();

    let img = image::GrayImage::from_fn(width as u32, height as u32, |x, y| {
        let mut v = buf.get((y as usize, x as usize)).unwrap() * 255.0;
        if v < 0.0 {
            v = 0.0;
        }
        if v > 255.0 {
            v = 255.0;
        }
        image::Luma([v as u8])
    });

    img.save(dest.as_ref())?;
    Ok(())
}

pub fn voxelize_mesh(
    mesh: &stl_io::IndexedMesh,
    mesh_bbox: &BoundingBox<f32>,
    printed_field: &ParamField,
    export_depth_images: bool,
) -> Result<ParamField, failure::Error> {
    use cgmath::*;

    let el = EventLoop::new();
    let sz = glutin::dpi::PhysicalSize::new(128, 128);
    let headless_context = ContextBuilder::new()
        .with_gl(glutin::GlRequest::Specific(glutin::Api::OpenGl, (4, 6)))
        .with_gl_profile(glutin::GlProfile::Core)
        .with_gl_debug_flag(true)
        .build_headless(&el, sz)?;

    let (gl, _headless_context) = unsafe {
        let headless_context = headless_context
            .make_current()
            .map_err(|_| failure::err_msg("failed to make context current"))?;

        (
            tinygl::Context::from_loader_function(|s| {
                headless_context.get_proc_address(s) as *const _
            }),
            headless_context,
        )
    };

    // VAO
    let _vao = unsafe {
        let name = gl.create_vertex_array().map_err(|emsg| {
            failure::err_msg(format!("failed to create vertex array object: {}", emsg))
        })?;
        gl.bind_vertex_array(Some(name));
        name
    };

    // Upload mesh vertices
    let vertices_buffer = tinygl::wrappers::Buffer::new(&gl)
        .map_err(|_| failure::err_msg("failed to create vertex buffer"))?;

    vertices_buffer.bind(&gl, gl::ARRAY_BUFFER);
    unsafe {
        gl.buffer_data_u8_slice(
            gl::ARRAY_BUFFER,
            {
                let slice = mesh.vertices.as_slice();
                std::slice::from_raw_parts(
                    slice.as_ptr() as *const _,
                    slice.len() * std::mem::size_of_val(&mesh.vertices[0]),
                )
            },
            gl::STATIC_DRAW,
        );
    }

    // Upload mesh indices
    let indices_buffer = tinygl::wrappers::Buffer::new(&gl)
        .map_err(|_| failure::err_msg("failed to create index buffer"))?;
    indices_buffer.bind(&gl, gl::ELEMENT_ARRAY_BUFFER);
    unsafe {
        let byte_count = (std::mem::size_of::<u32>() * mesh.faces.len() * 3) as i32;

        // Allocate storage
        gl.buffer_storage(
            gl::ELEMENT_ARRAY_BUFFER,
            byte_count,
            None,
            gl::MAP_WRITE_BIT,
        );

        // Map buffer
        let ptr = std::slice::from_raw_parts_mut(
            gl.map_buffer_range(gl::ELEMENT_ARRAY_BUFFER, 0, byte_count, gl::MAP_WRITE_BIT)
                as *mut u32,
            mesh.faces.len() * 3,
        );

        // Write indices to buffer
        for (idx, face) in mesh.faces.iter().enumerate() {
            for (index_idx, vertex_idx) in face.vertices.iter().enumerate() {
                ptr[idx * 3 + index_idx] = *vertex_idx as u32;
            }
        }

        // Unmap buffer (uploads)
        gl.unmap_buffer(gl::ELEMENT_ARRAY_BUFFER);
    }

    // Build display program
    let prog = shaders::MeshProgram::build(&gl)
        .map_err(|emsg| failure::err_msg(format!("failed to build program: {}", emsg)))?;
    prog.use_program(&gl);

    unsafe {
        // Enable vertex position attribute (vec3)
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, gl::FLOAT, false, 0, 0);

        // We only render depth
        gl.depth_mask(true);
        gl.color_mask(false, false, false, false);

        // We need depth test
        gl.enable(gl::DEPTH_TEST);

        // We need both front and back faces for rendering two types of depth
        gl.polygon_mode(gl::FRONT_AND_BACK, gl::FILL);
    }

    let printed_dim = printed_field.dim();

    let center = Vector3::from(mesh_bbox.center());
    let size = Vector3::from(mesh_bbox.size());

    debug!("input geometry center: {:?}", center);
    debug!("input geometry size: {:?}", size);

    // Render Z axis
    let (zplus, zminus) = {
        debug!("rendering Z axis depth");

        let trans = Matrix4::identity();

        render_axis(
            mesh_bbox,
            printed_dim.2,
            printed_dim.1,
            trans,
            &prog,
            &gl,
            mesh,
        )?
    };

    if export_depth_images {
        write_depth_img(&zplus, "zplus.png")?;
        write_depth_img(&zminus, "zminus.png")?;
    }

    let get_tran = |rot: Basis3<f32>| {
        Matrix4::from_translation(center)
            * Matrix4::from(Matrix3::from(rot))
            * Matrix4::from_translation(-center)
    };

    // Render Y axis
    let (yplus, yminus) = {
        debug!("rendering Y axis depth");

        let rot: Basis3<_> = Rotation3::from_angle_x(Rad(std::f32::consts::FRAC_PI_2));
        let trans = get_tran(rot);

        render_axis(
            mesh_bbox,
            printed_dim.2,
            printed_dim.0,
            trans,
            &prog,
            &gl,
            mesh,
        )?
    };

    if export_depth_images {
        write_depth_img(&yplus, "yplus.png")?;
        write_depth_img(&yminus, "yminus.png")?;
    }

    // Render X axis
    let (xplus, xminus) = {
        debug!("rendering X axis depth");

        let rot: Basis3<_> = Rotation3::from_angle_y(Rad(-std::f32::consts::FRAC_PI_2));
        let trans = get_tran(rot);

        render_axis(
            mesh_bbox,
            printed_dim.0,
            printed_dim.1,
            trans,
            &prog,
            &gl,
            mesh,
        )?
    };

    if export_depth_images {
        write_depth_img(&xplus, "xplus.png")?;
        write_depth_img(&xminus, "xminus.png")?;
    }

    // Compute visibility from depth buffers
    let mut vis = ndarray::Array3::<u8>::zeros((printed_dim.0, printed_dim.1, printed_dim.2));

    par_azip!((index (k, j, i), v in &mut vis) {
        let zw = zplus.dim().1;
        let z_min = zplus[(j, zw - 1 - i)] * printed_dim.0 as f32;
        let z_max = zminus[(j, zw - 1 - i)] * printed_dim.0 as f32;

        let yw = yplus.dim().1;
        let y_min = yplus[(k, yw - 1 - i)] * printed_dim.1 as f32;
        let y_max = yminus[(k, yw - 1 - i)] * printed_dim.1 as f32;

        let xw = xplus.dim().1;
        let x_min = xplus[(j, xw - 1 - k)] * printed_dim.2 as f32;
        let x_max = xminus[(j, xw - 1 - k)] * printed_dim.2 as f32;

        let k = (printed_dim.0 - 1 - k) as f32 + 0.5;
        let j = (printed_dim.1 - 1 - j) as f32 + 0.5;
        let i = (printed_dim.2 - 1 - i) as f32 + 0.5;

        fn axis_val(pos: f32, min: f32, max: f32) -> f32 {
            if pos >= min.ceil() && pos <= max.floor() {
                1.0
            } else if pos < min.floor() || pos > max.ceil() {
                0.0
            } else if pos < min.ceil() {
                (min.ceil() - pos).fract()
            } else {
                (pos - max.floor()).fract()
            }
        }

        *v = ((axis_val(k, z_min, z_max)
                * axis_val(j, y_min, y_max)
                * axis_val(i, x_min, x_max))
            * 255.0) as u8;
    });

    Ok(ParamField::new_u8(printed_field.field_box_mm, vis))
}
