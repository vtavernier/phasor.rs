use std::borrow::Cow;

use ndarray::par_azip;
use ndarray::prelude::*;
use serde_derive::{Deserialize, Serialize};

use super::param_array::ParamArray;
use super::utils::BoundingBox;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum FieldStorage {
    Byte(ndarray::Array3<u8>),
    ByteVec4(ndarray::Array4<u8>),
    Float(ndarray::Array3<f32>),
    Vec3(ndarray::Array4<f32>),
}

impl FieldStorage {
    fn as_u8_slice_mut(&mut self) -> Option<&mut [u8]> {
        match self {
            Self::Byte(array) => array.as_slice_mut(),
            Self::ByteVec4(array) => array.as_slice_mut(),
            _ => None,
        }
    }

    fn dim(&self) -> (usize, usize, usize, usize) {
        match self {
            Self::Byte(array) => {
                let d = array.dim();
                (d.0, d.1, d.2, 0)
            }
            Self::ByteVec4(array) => array.dim(),
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
                file.new_dataset::<u8>()
                    .gzip(6)
                    .create(&path, array.dim())?
                    .write(array.view())?;
            }
            Self::ByteVec4(array) => {
                let field = array.index_axis(Axis(3), 0);
                if std_layout.shape() != field.shape() {
                    let d = array.dim();
                    *std_layout = ndarray::Array3::zeros((d.0, d.1, d.2));
                }

                std_layout.assign(&field);

                let dim: (usize, usize, usize) = std_layout.dim().into();
                let dataset = file.new_dataset::<u8>().gzip(6).create(&path, dim)?;
                dataset.write(std_layout.view())?;
            }
            Self::Float(array) => {
                file.new_dataset::<f32>()
                    .gzip(6)
                    .create(&path, array.dim())?
                    .write(array.view())?;
            }
            Self::Vec3(array) => {
                file.new_dataset::<f32>()
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
            Self::ByteVec4(_) => Some(("UInt", 1, 1)),
            Self::Float(_) => Some(("Float", 4, 1)),
            Self::Vec3(array) => Some(("Float", 4, array.dim().3)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamField {
    pub field_box_mm: BoundingBox<f32>,
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

    pub fn new_u8(field_box_mm: BoundingBox<f32>, storage: ndarray::Array3<u8>) -> Self {
        Self {
            field_box_mm,
            field: FieldStorage::Byte(storage),
        }
    }

    pub fn new_vec3(field_box_mm: BoundingBox<f32>, storage: ndarray::Array4<f32>) -> Self {
        assert!(storage.dim().3 == 3);

        Self {
            field_box_mm,
            field: FieldStorage::Vec3(storage),
        }
    }

    pub fn new_f32(field_box_mm: BoundingBox<f32>, storage: ndarray::Array3<f32>) -> Self {
        Self {
            field_box_mm,
            field: FieldStorage::Float(storage),
        }
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
            field_box_mm: BoundingBox {
                min_x: Self::attr_into(attributes, "field_box_mm_min_x"),
                min_y: Self::attr_into(attributes, "field_box_mm_min_y"),
                min_z: Self::attr_into(attributes, "field_box_mm_min_z"),
                max_x: Self::attr_into(attributes, "field_box_mm_max_x"),
                max_y: Self::attr_into(attributes, "field_box_mm_max_y"),
                max_z: Self::attr_into(attributes, "field_box_mm_max_z"),
            },
            // Allocate array
            // Big endian order: fastest dimension varying last
            field: FieldStorage::ByteVec4(ndarray::Array4::zeros((
                field_sz, field_sy, field_sx, 4,
            ))),
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
            && self.field_box_mm == other.field_box_mm
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
        file.new_dataset::<f32>()
            .create(&format!("{}/bounding_box_min", path), (3,))?
            .write(&[
                self.field_box_mm.min_x,
                self.field_box_mm.min_y,
                self.field_box_mm.min_z,
            ])?;
        file.new_dataset::<f32>()
            .create(&format!("{}/bounding_box_max", path), (3,))?
            .write(&[
                self.field_box_mm.max_x,
                self.field_box_mm.max_y,
                self.field_box_mm.max_z,
            ])?;

        Ok(())
    }

    /// Returns (item_type, precision, components)
    pub fn xdmf_type(&self) -> Option<(&'static str, usize, usize)> {
        self.field.xdmf_type()
    }

    pub fn as_f32_array(&self, byte_scale: f32) -> Option<Cow<ndarray::Array3<f32>>> {
        match &self.field {
            FieldStorage::Float(array) => Some(Cow::Borrowed(array)),
            FieldStorage::Byte(array) => {
                let dim = self.dim();
                let mut mapped = ndarray::Array3::zeros((dim.0, dim.1, dim.2));

                // Scale u8 into [0, 1] f32
                azip!((f in &mut mapped, x in array) *f = (*x as f32 / 255.0) * byte_scale);

                Some(Cow::Owned(mapped))
            }
            FieldStorage::ByteVec4(array) => {
                let dim = self.dim();
                let mut mapped = ndarray::Array3::zeros((dim.0, dim.1, dim.2));

                // Scale u8 into [0, 1] f32
                azip!((f in &mut mapped, x in &array.index_axis(Axis(3), 0)) *f = (*x as f32 / 255.0) * byte_scale);

                Some(Cow::Owned(mapped))
            }
            _ => None,
        }
    }

    pub fn as_u8(&self) -> Option<&ndarray::Array3<u8>> {
        match &self.field {
            FieldStorage::Byte(array) => Some(array),
            _ => None,
        }
    }

    pub fn as_vec3(&self) -> Option<&ndarray::Array4<f32>> {
        match &self.field {
            FieldStorage::Vec3(array) => Some(array),
            _ => None,
        }
    }

    pub fn derive_vec3_from_field(&self, data: ndarray::Array4<f32>) -> Self {
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
            let norm_z = (z as f32 + 0.5) / (dim.0 + 1) as f32;
            // z in 0..src.len() range
            let src_z = norm_z * (src.len() + 1) as f32 - 0.5;
            // z idx in src
            let src_idx = src_z.floor() as usize;
            let src_idx_p1 = (src_idx + 1).min(src.len());
            // Linear interpolation
            let val = src[src_idx] as f32 * (1.0 - src_z.fract())
                + src[src_idx_p1] as f32 * src_z.fract();

            data.index_axis_mut(Axis(0), z).fill(val);
        }

        Some(Self {
            field: FieldStorage::Float(data),
            ..*self
        })
    }

    pub fn resample(&self, mask: &ParamField) -> Self {
        use nalgebra::Vector3;

        debug!("input field bounding box: {:?}", self.field_box_mm);
        debug!("mask field bounding box: {:?}", mask.field_box_mm);

        let im = mask.as_u8().expect("invalid input mask type");

        let out_scale = mask.field_box_mm.size().component_div(&Vector3::new(
            im.dim().2 as f32,
            im.dim().1 as f32,
            im.dim().0 as f32,
        ));

        let in_scale = self.field_box_mm.size().component_div(&Vector3::new(
            self.dim().2 as f32,
            self.dim().1 as f32,
            self.dim().0 as f32,
        ));

        match &self.field {
            FieldStorage::ByteVec4(array) => {
                let mut out = ndarray::Array3::<u8>::zeros(im.dim());

                par_azip!((index (k, j, i), d in &mut out, m in im) {
                    // Convert output coordinates into point in input array
                    let p = Vector3::new(i as f32 + 0.5, j as f32 + 0.5, k as f32 + 0.5); // Float array coordinates
                    let p = p.component_mul(&out_scale); // mm coordinates
                    let p = p.component_div(&in_scale); // input coordinates

                    // Tri-linear interpolation
                    let px1 = (p.x.floor() as isize).max(0).min((array.dim().2 - 1) as isize) as usize;
                    let py1 = (p.y.floor() as isize).max(0).min((array.dim().1 - 1) as isize) as usize;
                    let pz1 = (p.z.floor() as isize).max(0).min((array.dim().0 - 1) as isize) as usize;
                    let px2 = (p.x.ceil() as isize).max(0).min((array.dim().2 - 1) as isize) as usize;
                    let py2 = (p.y.ceil() as isize).max(0).min((array.dim().1 - 1) as isize) as usize;
                    let pz2 = (p.z.ceil() as isize).max(0).min((array.dim().0 - 1) as isize) as usize;
                    let ax = p.x.fract();
                    let ay = p.y.fract();
                    let az = p.z.fract();

                    let c00 = array[(pz1, py1, px1, 0)] as f32 * (1.0 - ax) + array[(pz1, py1, px2, 0)] as f32 * ax;
                    let c01 = array[(pz2, py1, px1, 0)] as f32 * (1.0 - ax) + array[(pz2, py1, px2, 0)] as f32 * ax;
                    let c10 = array[(pz1, py2, px1, 0)] as f32 * (1.0 - ax) + array[(pz1, py2, px2, 0)] as f32 * ax;
                    let c11 = array[(pz2, py2, px1, 0)] as f32 * (1.0 - ax) + array[(pz1, py2, px2, 0)] as f32 * ax;

                    let c0 = c00 * (1.0 - ay) + c10 * ay;
                    let c1 = c01 * (1.0 - ay) + c11 * ay;

                    *d = (*m as f32 / 255.0 * (c0 * (1.0 - az) + c1 * az)) as u8;
                });

                ParamField::new_u8(mask.field_box_mm, out)
            }
            FieldStorage::Vec3(array) => {
                let mut out =
                    ndarray::Array4::<f32>::zeros((im.dim().0, im.dim().1, im.dim().2, 3));

                par_azip!((index (k, j, i), mut d in out.lanes_mut(Axis(3)), m in im) {
                    // Convert output coordinates into point in input array
                    let p = Vector3::new(i as f32 + 0.5, j as f32 + 0.5, k as f32 + 0.5); // Float array coordinates
                    let p = p.component_mul(&out_scale); // mm coordinates
                    let p = p.component_div(&in_scale); // input coordinates

                    // Tri-linear interpolation
                    let px1 = (p.x.floor() as isize).max(0).min((array.dim().2 - 1) as isize) as usize;
                    let py1 = (p.y.floor() as isize).max(0).min((array.dim().1 - 1) as isize) as usize;
                    let pz1 = (p.z.floor() as isize).max(0).min((array.dim().0 - 1) as isize) as usize;
                    let px2 = (p.x.ceil() as isize).max(0).min((array.dim().2 - 1) as isize) as usize;
                    let py2 = (p.y.ceil() as isize).max(0).min((array.dim().1 - 1) as isize) as usize;
                    let pz2 = (p.z.ceil() as isize).max(0).min((array.dim().0 - 1) as isize) as usize;
                    let ax = p.x.fract();
                    let ay = p.y.fract();
                    let az = p.z.fract();

                    let c000: Vector3<f32> = nalgebra::convert(Vector3::from_row_slice(array.slice(s![pz1, py1, px1, ..]).as_slice().unwrap()));
                    let c001: Vector3<f32> = nalgebra::convert(Vector3::from_row_slice(array.slice(s![pz2, py1, px1, ..]).as_slice().unwrap()));
                    let c010: Vector3<f32> = nalgebra::convert(Vector3::from_row_slice(array.slice(s![pz1, py2, px1, ..]).as_slice().unwrap()));
                    let c011: Vector3<f32> = nalgebra::convert(Vector3::from_row_slice(array.slice(s![pz2, py2, px1, ..]).as_slice().unwrap()));
                    let c100: Vector3<f32> = nalgebra::convert(Vector3::from_row_slice(array.slice(s![pz1, py1, px2, ..]).as_slice().unwrap()));
                    let c101: Vector3<f32> = nalgebra::convert(Vector3::from_row_slice(array.slice(s![pz2, py1, px2, ..]).as_slice().unwrap()));
                    let c110: Vector3<f32> = nalgebra::convert(Vector3::from_row_slice(array.slice(s![pz1, py2, px2, ..]).as_slice().unwrap()));
                    let c111: Vector3<f32> = nalgebra::convert(Vector3::from_row_slice(array.slice(s![pz2, py2, px2, ..]).as_slice().unwrap()));

                    let c00 = c000 * (1.0 - ax) + c100 * ax;
                    let c01 = c001 * (1.0 - ax) + c101 * ax;
                    let c10 = c010 * (1.0 - ax) + c110 * ax;
                    let c11 = c011 * (1.0 - ax) + c111 * ax;

                    let c0 = c00 * (1.0 - ay) + c10 * ay;
                    let c1 = c01 * (1.0 - ay) + c11 * ay;

                    // Normalize because we only resample direction vectors
                    let c = *m as f32 / 255.0 * (c0 * (1.0 - az) + c1 * az).normalize();

                    d[0] = c.x;
                    d[1] = c.y;
                    d[2] = c.z;
                });

                Self {
                    field_box_mm: mask.field_box_mm,
                    field: FieldStorage::Vec3(out),
                }
            }
            _ => panic!("unsupported field storage type for resampling"),
        }
    }
}
