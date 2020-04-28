use std::fs::File;
use std::path::{Path, PathBuf};

use super::utils::BoundingBox;

pub fn get_bounding_box(mesh_path: &Path) -> Result<BoundingBox, failure::Error> {
    let mut mesh = File::open(mesh_path)?;
    let mesh = stl_io::read_stl(&mut mesh)?;

    let mut x_min = std::f32::MAX;
    let mut y_min = std::f32::MAX;
    let mut z_min = std::f32::MAX;
    let mut x_max = std::f32::MIN;
    let mut y_max = std::f32::MIN;
    let mut z_max = std::f32::MIN;

    for vertex in mesh.vertices {
        x_min = vertex[0].min(x_min);
        y_min = vertex[1].min(y_min);
        z_min = vertex[2].min(z_min);
        x_max = vertex[0].max(x_max);
        y_max = vertex[1].max(y_max);
        z_max = vertex[2].max(z_max);
    }

    Ok(BoundingBox {
        min_x: x_min as f64,
        min_y: y_min as f64,
        min_z: z_min as f64,
        max_x: x_max as f64,
        max_y: y_max as f64,
        max_z: z_max as f64,
    })
}

pub fn read_offsets(geometry: &Option<PathBuf>) -> Result<(f64, f64, f64), failure::Error> {
    if let Some(mesh_path) = geometry {
        let bounding_box = get_bounding_box(mesh_path)?;

        Ok((
            ((bounding_box.min_x + bounding_box.max_x) / 2.0),
            ((bounding_box.min_y + bounding_box.max_y) / 2.0),
            ((bounding_box.min_z + bounding_box.max_z) / 2.0),
        ))
    } else {
        Ok((0.0f64, 0.0f64, 0.0f64))
    }
}
