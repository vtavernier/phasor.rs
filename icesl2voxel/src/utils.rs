use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BoundingBox {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BoundingBoxSize {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl BoundingBox {
    pub fn size(&self) -> BoundingBoxSize {
        BoundingBoxSize {
            x: self.max_x - self.min_x,
            y: self.max_y - self.min_y,
            z: self.max_z - self.min_z,
        }
    }
}
