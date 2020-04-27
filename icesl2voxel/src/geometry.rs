use std::fs::File;
use std::path::PathBuf;

pub fn read_offsets(geometry: &Option<PathBuf>) -> Result<(f64, f64, f64), failure::Error> {
    if let Some(mesh_path) = geometry {
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

        Ok((
            ((x_min + x_max) / 2.0).into(),
            ((y_min + y_max) / 2.0).into(),
            ((z_min + z_max) / 2.0).into(),
        ))
    } else {
        Ok((0.0f64, 0.0f64, 0.0f64))
    }
}
