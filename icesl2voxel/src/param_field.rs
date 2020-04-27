use std::borrow::Cow;

use ndarray::prelude::*;
use serde_derive::{Deserialize, Serialize};

use super::param_array::ParamArray;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum FieldStorage {
    Byte(ndarray::Array4<u8>),
    Float(ndarray::Array3<f64>),
    Vec3(ndarray::Array4<f64>),
}

impl FieldStorage {
    fn as_f64_slice_mut(&mut self) -> Option<&mut [f64]> {
        match self {
            Self::Float(array) => array.as_slice_mut(),
            Self::Vec3(array) => array.as_slice_mut(),
            _ => None,
        }
    }

    fn as_u8_slice_mut(&mut self) -> Option<&mut [u8]> {
        match self {
            Self::Byte(array) => array.as_slice_mut(),
            _ => None,
        }
    }

    fn dim(&self) -> (usize, usize, usize, usize) {
        match self {
            Self::Byte(array) => array.dim(),
            Self::Float(array) => {
                let d = array.dim();
                (d.0, d.1, d.2, 0)
            }
            Self::Vec3(array) => array.dim(),
        }
    }

    fn write_hdf5(
        &self,
        path: &str,
        file: &hdf5::File,
        std_layout: &mut ndarray::Array3<u8>,
    ) -> Result<(), hdf5::Error> {
        match self {
            Self::Byte(array) => {
                let field = array.index_axis(Axis(3), 0);
                std_layout.assign(&field);

                let dim: (usize, usize, usize) = std_layout.dim().into();
                let dataset = file.new_dataset::<u8>().gzip(6).create(&path, dim)?;
                dataset.write(std_layout.view())?;
            }
            Self::Float(array) => {
                file.new_dataset::<f64>()
                    .gzip(6)
                    .create(&path, array.dim())?
                    .write(array.view())?;
            }
            Self::Vec3(array) => {
                file.new_dataset::<f64>()
                    .gzip(6)
                    .create(&path, array.dim())?
                    .write(array.view())?;
            }
        }

        Ok(())
    }

    fn xdmf_type(&self) -> Option<(&'static str, usize, usize)> {
        match self {
            Self::Byte(_) => Some(("UInt", 1, 1)),
            Self::Float(_) => Some(("Float", 8, 1)),
            Self::Vec3(array) => Some(("Float", 8, array.dim().3)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamField {
    pub field_box_mm_min_x: f64,
    pub field_box_mm_min_y: f64,
    pub field_box_mm_min_z: f64,
    pub field_box_mm_max_x: f64,
    pub field_box_mm_max_y: f64,
    pub field_box_mm_max_z: f64,
    field: FieldStorage,
}

impl ParamField {
    fn attr<'a>(
        attributes: &'a [xml::attribute::OwnedAttribute],
        name: &str,
    ) -> &'a xml::attribute::OwnedAttribute {
        attributes
            .iter()
            .find(|a| a.name.local_name == name)
            .unwrap()
    }

    fn attr_into<T: std::str::FromStr>(
        attributes: &[xml::attribute::OwnedAttribute],
        name: &str,
    ) -> T
    where
        <T as std::str::FromStr>::Err: std::fmt::Debug,
    {
        Self::attr(attributes, name).value.parse::<T>().unwrap()
    }

    pub fn from_attr(
        attributes: &[xml::attribute::OwnedAttribute],
    ) -> Result<Self, failure::Error> {
        let (field_sx, field_sy, field_sz) = (
            Self::attr_into(attributes, "field_sx"),
            Self::attr_into(attributes, "field_sy"),
            Self::attr_into(attributes, "field_sz"),
        );

        // Parse field parameters
        let mut field = ParamField {
            field_box_mm_min_x: Self::attr_into(attributes, "field_box_mm_min_x"),
            field_box_mm_min_y: Self::attr_into(attributes, "field_box_mm_min_y"),
            field_box_mm_min_z: Self::attr_into(attributes, "field_box_mm_min_z"),
            field_box_mm_max_x: Self::attr_into(attributes, "field_box_mm_max_x"),
            field_box_mm_max_y: Self::attr_into(attributes, "field_box_mm_max_y"),
            field_box_mm_max_z: Self::attr_into(attributes, "field_box_mm_max_z"),
            // Allocate array
            // Big endian order: fastest dimension varying last
            field: FieldStorage::Byte(ndarray::Array4::zeros((field_sz, field_sy, field_sx, 4))),
        };

        // Parse field data
        let mut attr_value_cursor = std::io::Cursor::new(&Self::attr(attributes, "field").value);
        let mut base64_decoder =
            base64::read::DecoderReader::new(&mut attr_value_cursor, base64::STANDARD);
        let mut zip_reader = libflate::zlib::Decoder::new(&mut base64_decoder)?;
        std::io::copy(&mut zip_reader, &mut field.field.as_u8_slice_mut().unwrap())?;

        Ok(field)
    }

    pub fn has_same_box(&self, other: &Self) -> bool {
        self.dim().0 == other.dim().0
            && self.dim().1 == other.dim().1
            && self.dim().2 == other.dim().2
            && self.field_box_mm_min_x == other.field_box_mm_min_x
            && self.field_box_mm_min_y == other.field_box_mm_min_y
            && self.field_box_mm_min_z == other.field_box_mm_min_z
            && self.field_box_mm_max_x == other.field_box_mm_max_x
            && self.field_box_mm_max_y == other.field_box_mm_max_y
            && self.field_box_mm_max_z == other.field_box_mm_max_z
    }

    pub fn dim(&self) -> (usize, usize, usize, usize) {
        self.field.dim()
    }

    pub fn write_hdf5(
        &self,
        path: &str,
        file: &hdf5::File,
        std_layout: &mut ndarray::Array3<u8>,
    ) -> Result<(), hdf5::Error> {
        self.field
            .write_hdf5(&format!("{}/data", path), file, std_layout)?;

        // Bounding box
        file.new_dataset::<f64>()
            .create(&format!("{}/bounding_box_min", path), (3,))?
            .write(&[
                self.field_box_mm_min_x,
                self.field_box_mm_min_y,
                self.field_box_mm_min_z,
            ])?;
        file.new_dataset::<f64>()
            .create(&format!("{}/bounding_box_max", path), (3,))?
            .write(&[
                self.field_box_mm_max_x,
                self.field_box_mm_max_y,
                self.field_box_mm_max_z,
            ])?;

        Ok(())
    }

    /// Returns (item_type, precision, components)
    pub fn xdmf_type(&self) -> Option<(&'static str, usize, usize)> {
        self.field.xdmf_type()
    }

    pub fn as_f64_array(&self, byte_scale: f64) -> Option<Cow<ndarray::Array3<f64>>> {
        match &self.field {
            FieldStorage::Float(array) => Some(Cow::Borrowed(array)),
            FieldStorage::Byte(array) => {
                let dim = self.dim();
                let mut mapped = ndarray::Array3::zeros((dim.0, dim.1, dim.2));

                // Scale u8 into [0, 1] f32
                azip!((f in &mut mapped, x in &array.index_axis(Axis(3), 0)) *f = (*x as f64 / 255.0) * byte_scale);

                Some(Cow::Owned(mapped))
            }
            _ => None,
        }
    }

    pub fn derive_vec3_from_field(&self, data: ndarray::Array4<f64>) -> Self {
        Self {
            field: FieldStorage::Vec3(data),
            ..*self
        }
    }

    pub fn derive_from_array(&self, array: &ParamArray) -> Option<Self> {
        let dim = self.dim();
        let mut data = ndarray::Array3::zeros((dim.0, dim.1, dim.2));
        let src = array.as_f64_slice()?;

        for z in 0..dim.0 {
            // z in 0..1 range (cell middle)
            let norm_z = (z as f64 + 0.5) / (dim.0 + 1) as f64;
            // z in 0..src.len() range
            let src_z = norm_z * (src.len() + 1) as f64 - 0.5;
            // z idx in src
            let src_idx = src_z.floor() as usize;
            let src_idx_p1 = (src_idx + 1).min(src.len());
            // Linear interpolation
            let val = src[src_idx] * (1.0 - src_z.fract()) + src[src_idx_p1] * src_z.fract();

            data.index_axis_mut(Axis(0), z).fill(val);
        }

        Some(Self {
            field: FieldStorage::Float(data),
            ..*self
        })
    }
}
