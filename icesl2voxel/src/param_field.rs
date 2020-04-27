use ndarray::prelude::*;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ParamField {
    pub field_box_mm_min_x: f64,
    pub field_box_mm_min_y: f64,
    pub field_box_mm_min_z: f64,
    pub field_box_mm_max_x: f64,
    pub field_box_mm_max_y: f64,
    pub field_box_mm_max_z: f64,
    field: ndarray::Array4<u8>,
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
            ..Default::default()
        };

        // Allocate array
        // Big endian order: fastest dimension varying last
        field.field = ndarray::Array4::zeros((field_sz, field_sy, field_sx, 4));

        // Parse field data
        let mut attr_value_cursor = std::io::Cursor::new(&Self::attr(attributes, "field").value);
        let mut base64_decoder =
            base64::read::DecoderReader::new(&mut attr_value_cursor, base64::STANDARD);
        let mut zip_reader = libflate::zlib::Decoder::new(&mut base64_decoder)?;
        std::io::copy(&mut zip_reader, &mut field.field.as_slice_mut().unwrap())?;

        Ok(field)
    }

    pub fn has_same_box(&self, other: &Self) -> bool {
        self.dim() == other.dim()
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
        {
            let path = format!("{}/data", path);
            let field = self.field.index_axis(Axis(3), 0);
            std_layout.assign(&field);

            let dim: (usize, usize, usize) = std_layout.dim().into();
            let dataset = file.new_dataset::<u8>().gzip(6).create(&path, dim)?;
            dataset.write(std_layout.view())?;
        }

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
}
