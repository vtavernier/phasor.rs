use std::fs::File;
use std::path::Path;

use super::utils::BoundingBox;

pub fn load_mesh(mesh_path: &Path) -> Result<stl_io::IndexedMesh, failure::Error> {
    let mut mesh = File::open(mesh_path)?;
    Ok(stl_io::read_stl(&mut mesh)?)
}

pub fn get_bounding_box(mesh: &stl_io::IndexedMesh) -> BoundingBox<f32> {
    let mut min_x = std::f32::MAX;
    let mut min_y = std::f32::MAX;
    let mut min_z = std::f32::MAX;
    let mut max_x = std::f32::MIN;
    let mut max_y = std::f32::MIN;
    let mut max_z = std::f32::MIN;

    for vertex in &mesh.vertices {
        min_x = vertex[0].min(min_x);
        min_y = vertex[1].min(min_y);
        min_z = vertex[2].min(min_z);
        max_x = vertex[0].max(max_x);
        max_y = vertex[1].max(max_y);
        max_z = vertex[2].max(max_z);
    }

    BoundingBox {
        min_x,
        min_y,
        min_z,
        max_x,
        max_y,
        max_z,
    }
}
