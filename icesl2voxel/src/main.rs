//! icesl2voxel is a tool to convert files from the IceSL XML format to HDF5+XDMF files which can
//! be loaded inside Paraview for visualization or from other code for further processing.
//!
//! ## Usage
//!
//!     # Extract fields and parameters from file.xml into file.h5 (and file.xdmf)
//!     cargo run -- -i file.xml -o file.h5
//!
//! ## Author
//!
//! Vincent Tavernier <vince.tavernier@gmail.com>

#[macro_use]
extern crate log;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::PathBuf;

use xml::reader::{EventReader, XmlEvent};

use structopt::StructOpt;

use lazy_static::lazy_static;
use regex::Regex;

use ndarray::prelude::*;

use serde_derive::{Deserialize, Serialize};

lazy_static! {
    static ref ELEMENT_NAME_PARAM_RE: Regex = Regex::new(r"^(.*)_(\d*)$").unwrap();
}

#[derive(StructOpt)]
struct Opts {
    /// Input model XML file path
    #[structopt(short, long)]
    input: PathBuf,

    /// Geometry input
    #[structopt(short, long)]
    mesh: Option<PathBuf>,

    /// HDF5 file path for output
    #[structopt(short, long)]
    output: Option<PathBuf>,
}

trait Parse<T> {
    fn parse(value: &str) -> Result<T, failure::Error>;
}

impl Parse<f64> for f64 {
    fn parse(value: &str) -> Result<f64, failure::Error> {
        value.parse::<f64>().map_err(|e| e.into())
    }
}

impl Parse<bool> for bool {
    fn parse(value: &str) -> Result<bool, failure::Error> {
        match value {
            "True" => Ok(true),
            "False" => Ok(false),
            _ => Err(failure::err_msg("invalid bool")),
        }
    }
}

impl Parse<String> for String {
    fn parse(value: &str) -> Result<String, failure::Error> {
        Ok(value.to_owned())
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum Param {
    Bool(bool),
    Float(f64),
    String(String),
}

impl std::convert::TryFrom<&str> for Param {
    type Error = failure::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(if let Ok(value) = <f64 as Parse<f64>>::parse(value) {
            Param::Float(value)
        } else if let Ok(value) = <bool as Parse<bool>>::parse(value) {
            Param::Bool(value)
        } else {
            Param::String(value.to_owned())
        })
    }
}

impl Param {
    fn as_bool(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            _ => panic!(),
        }
    }

    fn as_float(&self) -> f64 {
        match self {
            Self::Float(value) => *value,
            _ => panic!(),
        }
    }
}

impl Into<bool> for Param {
    fn into(self) -> bool {
        self.as_bool()
    }
}

impl Into<f64> for Param {
    fn into(self) -> f64 {
        self.as_float()
    }
}

impl Into<String> for Param {
    fn into(self) -> String {
        match self {
            Self::String(value) => value,
            _ => panic!(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum ParamArrayStorage {
    Bool(Vec<bool>),
    Float(Vec<f64>),
    String(Vec<String>),
}

impl ParamArrayStorage {
    /// Returns Ok(is_empty)
    fn parse_and_add<T: Default + Parse<T> + Clone>(
        tgt: &mut Vec<T>,
        idx: usize,
        value: &str,
    ) -> Result<bool, failure::Error> {
        let is_empty = value.is_empty();
        let parsed_val = if is_empty {
            T::default()
        } else {
            T::parse(value).map_err(|_| {
                failure::err_msg(format!("failed to parse attribute value: `{}`", value))
            })?
        };

        Self::add_value(tgt, idx, parsed_val)?;

        Ok(is_empty)
    }

    /// Returns Ok(is_empty)
    fn add_value<T: Default + Parse<T> + Clone>(
        tgt: &mut Vec<T>,
        idx: usize,
        value: T,
    ) -> Result<(), failure::Error> {
        if tgt.len() <= idx {
            tgt.resize(idx + 1, T::default());
        }

        tgt[idx] = value;

        Ok(())
    }

    /// Returns Ok(is_empty)
    fn try_add(&mut self, idx: usize, value: &str) -> Result<bool, failure::Error> {
        match self {
            Self::Bool(tgt) => Self::parse_and_add(tgt, idx, value),
            Self::Float(tgt) => Self::parse_and_add(tgt, idx, value),
            Self::String(tgt) => Self::parse_and_add(tgt, idx, value),
        }
    }

    fn add(&mut self, idx: usize, value: Param) {
        match self {
            Self::Bool(tgt) => Self::add_value(tgt, idx, value.into()),
            Self::Float(tgt) => Self::add_value(tgt, idx, value.into()),
            Self::String(tgt) => Self::add_value(tgt, idx, value.into()),
        }
        .unwrap();
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ParamArray {
    values: ParamArrayStorage,
}

impl ParamArray {
    fn from_val(idx: usize, val: Param) -> Self {
        let mut this = match val {
            Param::Bool(_) => Self {
                values: ParamArrayStorage::Bool(Vec::with_capacity(256)),
            },
            Param::Float(_) => Self {
                values: ParamArrayStorage::Float(Vec::with_capacity(256)),
            },
            Param::String(_) => Self {
                values: ParamArrayStorage::String(Vec::with_capacity(256)),
            },
        };

        this.add_param(idx, val);
        this
    }

    fn new(idx: usize, value: &str) -> Result<Self, failure::Error> {
        Ok(if value.is_empty() {
            Self::from_val(idx, Param::String(String::new()))
        } else {
            Self::from_val(idx, Param::try_from(value)?)
        })
    }

    fn add(&mut self, idx: usize, value: &str) -> Result<(), failure::Error> {
        self.values.try_add(idx, value).map(|_| ())
    }

    fn add_param(&mut self, idx: usize, param: Param) {
        self.values.add(idx, param)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ParamField {
    field_sx: usize,
    field_sy: usize,
    field_sz: usize,
    field_box_mm_min_x: f64,
    field_box_mm_min_y: f64,
    field_box_mm_min_z: f64,
    field_box_mm_max_x: f64,
    field_box_mm_max_y: f64,
    field_box_mm_max_z: f64,
    field: ndarray::Array4<u8>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct ParamBag {
    param_fields: HashMap<String, ParamField>,
    param_arrays: HashMap<String, ParamArray>,
    params: HashMap<String, Param>,
}

impl ParamBag {
    fn new() -> Self {
        Self::default()
    }

    fn add_item(&mut self, name: &str, value: &str) -> Result<(), failure::Error> {
        self.params.insert(name.to_owned(), Param::try_from(value)?);
        Ok(())
    }

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

    fn add_field(
        &mut self,
        name: &str,
        attributes: &[xml::attribute::OwnedAttribute],
    ) -> Result<(), failure::Error> {
        // Parse field parameters
        let mut field = ParamField {
            field_sx: Self::attr_into(attributes, "field_sx"),
            field_sy: Self::attr_into(attributes, "field_sy"),
            field_sz: Self::attr_into(attributes, "field_sz"),
            field_box_mm_min_x: Self::attr_into(attributes, "field_box_mm_min_x"),
            field_box_mm_min_y: Self::attr_into(attributes, "field_box_mm_min_y"),
            field_box_mm_min_z: Self::attr_into(attributes, "field_box_mm_min_z"),
            field_box_mm_max_x: Self::attr_into(attributes, "field_box_mm_max_x"),
            field_box_mm_max_y: Self::attr_into(attributes, "field_box_mm_max_y"),
            field_box_mm_max_z: Self::attr_into(attributes, "field_box_mm_max_z"),
            ..Default::default()
        };

        // Allocate array
        field.field = ndarray::Array4::zeros((field.field_sx, field.field_sy, field.field_sz, 4));

        // Parse field data
        let mut attr_value_cursor = std::io::Cursor::new(&Self::attr(attributes, "field").value);
        let mut base64_decoder =
            base64::read::DecoderReader::new(&mut attr_value_cursor, base64::STANDARD);
        let mut zip_reader = libflate::zlib::Decoder::new(&mut base64_decoder)?;
        std::io::copy(&mut zip_reader, &mut field.field.as_slice_mut().unwrap())?;

        // Add to known fields
        self.param_fields.insert(name.to_owned(), field);

        Ok(())
    }

    fn add_array_item(
        &mut self,
        parsed: &regex::Captures,
        value: &str,
    ) -> Result<(), failure::Error> {
        let param_name = parsed
            .get(1)
            .ok_or_else(|| failure::err_msg("missing param name"))?
            .as_str();
        let param_idx: usize = parsed
            .get(2)
            .ok_or_else(|| failure::err_msg("missing param idx"))?
            .as_str()
            .parse()?;

        if let Some(array) = self.param_arrays.get_mut(param_name) {
            array.add(param_idx, value)?;
        } else {
            self.param_arrays
                .insert(param_name.to_owned(), ParamArray::new(param_idx, value)?);
        };

        Ok(())
    }
}

#[paw::main]
fn main(opts: Opts) -> Result<(), failure::Error> {
    env_logger::Builder::from_default_env()
        .format_timestamp(None)
        .format_module_path(false)
        .init();

    let file = File::open(&opts.input)?;
    let file = BufReader::new(file);

    let mut param_bag = ParamBag::new();

    let parser = EventReader::new(file);
    for e in parser {
        match &e {
            Ok(XmlEvent::StartElement {
                name, attributes, ..
            }) => {
                if let Some(_) = attributes
                    .iter()
                    .find(|attr| attr.name.local_name == "field")
                {
                    let name = ELEMENT_NAME_PARAM_RE
                        .captures(&name.local_name)
                        .unwrap()
                        .get(1)
                        .unwrap()
                        .as_str();

                    param_bag.add_field(&name, &attributes[..])?;
                }
                if let Some(attribute) = attributes
                    .iter()
                    .find(|attr| attr.name.local_name == "value")
                {
                    if let Some(captures) = ELEMENT_NAME_PARAM_RE.captures(&name.local_name) {
                        param_bag.add_array_item(&captures, &attribute.value)?;
                    } else {
                        param_bag.add_item(&name.local_name, &attribute.value)?;
                    }
                } else {
                    warn!("no value attribute found on {:?}", e);
                }
            }
            Ok(XmlEvent::EndElement { .. }) => {
                // We don't care about EndElement
            }
            other => {
                trace!("parser event: {:?}", other);
            }
        }
    }

    // Load geometry and compute origin
    let (x_offset, y_offset, z_offset) = if let Some(mesh_path) = opts.mesh {
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

        (
            ((x_min + x_max) / 2.0).into(),
            ((y_min + y_max) / 2.0).into(),
            ((z_min + z_max) / 2.0).into(),
        )
    } else {
        (0.0f64, 0.0f64, 0.0f64)
    };

    // Write fields
    let _e = hdf5::silence_errors();

    if let Some(output) = opts.output {
        let h5_file_name = output.file_name().unwrap().to_string_lossy();

        let file = hdf5::File::create(&output)?;
        let mut meta = File::create(output.with_extension("xdmf"))?;

        writeln!(meta, "<?xml version=\"1.0\" encoding=\"utf-8\" ?>")?;
        writeln!(meta, "<!DOCTYPE Xdmf SYSTEM \"Xdmf.dtd\" []>")?;
        writeln!(meta, "<Xdmf Version=\"2.0\">")?;
        writeln!(meta, "  <Domain>")?;

        // Assume all fields share the same grid
        let first_field = param_bag.param_fields.iter().next().unwrap().1;

        writeln!(meta, "    <Grid Name=\"mesh\" GridType=\"Uniform\">")?;
        writeln!(meta, "      <Topology Name=\"topo\" TopologyType=\"3DCoRectMesh\" NumberOfElements=\"{x} {y} {z}\" />",
            x = first_field.field.dim().0,
            y = first_field.field.dim().1,
            z = first_field.field.dim().2,)?;
        writeln!(meta, "      <Geometry Name=\"geo\" Type=\"ORIGIN_DXDYDZ\">")?;

        let x_scale = (first_field.field_box_mm_max_x - first_field.field_box_mm_min_x)
            / first_field.field_sx as f64;
        let y_scale = (first_field.field_box_mm_max_y - first_field.field_box_mm_min_y)
            / first_field.field_sy as f64;
        let z_scale = (first_field.field_box_mm_max_z - first_field.field_box_mm_min_z)
            / (first_field.field_sz - 1) as f64;

        // TODO: Write this in HDF
        writeln!(meta, "        <DataItem Format=\"XML\" Dimensions=\"3\">")?;
        writeln!(
            meta,
            "          {z} {y} {x}",
            x = x_offset
                + (first_field.field_box_mm_max_x - first_field.field_box_mm_min_x) / -2.0
                + 0.75 / x_scale,
            y = y_offset
                + (first_field.field_box_mm_max_y - first_field.field_box_mm_min_y) / -2.0
                + 0.75 / y_scale,
            z = z_offset
                + (first_field.field_box_mm_max_z - first_field.field_box_mm_min_z) / -2.0
                + 0.25 / z_scale,
        )?;
        writeln!(meta, "        </DataItem>")?;
        // TODO: Write this in HDF
        writeln!(meta, "        <DataItem Format=\"XML\" Dimensions=\"3\">")?;
        writeln!(
            meta,
            "          {z} {y} {x}",
            x = x_scale,
            y = y_scale,
            z = z_scale,
        )?;
        writeln!(meta, "        </DataItem>")?;
        writeln!(meta, "      </Geometry>")?;

        // For converting into standard layout
        let mut std_layout = ndarray::Array3::<u8>::zeros((
            first_field.field.dim().0,
            first_field.field.dim().1,
            first_field.field.dim().2,
        ));

        // Write fields
        for (name, field) in &param_bag.param_fields {
            for (idx, channel_name) in ["r", "g", "b", "a"].iter().enumerate() {
                let field = field.field.index_axis(Axis(3), idx);
                std_layout.assign(&field);
                let path = format!("/fields/{}/{}", name, channel_name);

                let dim: (usize, usize, usize) = std_layout.dim().into();
                let dataset = file.new_dataset::<u8>().gzip(6).create(&path, dim)?;
                dataset.write(std_layout.view())?;

                writeln!(meta, "        <Attribute Name=\"{name}_{channel}\" AttributeType=\"Scalar\" Center=\"Cell\">",
                    name = name,
                    channel = channel_name)?;
                writeln!(meta, "          <DataItem Dimensions=\"{x} {y} {z}\" Format=\"HDF5\" DataType=\"UInt\" Precision=\"1\">",
                    x = dim.0,
                    y = dim.1,
                    z = dim.2)?;
                writeln!(meta, "            {}:{}", h5_file_name, path)?;
                writeln!(meta, "          </DataItem>")?;
                writeln!(meta, "        </Attribute>")?;
            }

            // Bounding box
            file.new_dataset::<f64>()
                .create(&format!("/fields/{}/bounding_box_min", name), (3,))?
                .write(&[
                    field.field_box_mm_min_x,
                    field.field_box_mm_min_y,
                    field.field_box_mm_min_z,
                ])?;
            file.new_dataset::<f64>()
                .create(&format!("/fields/{}/bounding_box_max", name), (3,))?
                .write(&[
                    field.field_box_mm_max_x,
                    field.field_box_mm_max_y,
                    field.field_box_mm_max_z,
                ])?;
        }

        // Write array params
        for (name, array) in &param_bag.param_arrays {
            let path = format!("/arrays/{}", name);

            match &array.values {
                ParamArrayStorage::Bool(value) => {
                    file.new_dataset::<u8>()
                        .gzip(6)
                        .create(&path, (value.len(),))?
                        .write(
                            &value
                                .iter()
                                .map(|b| if *b { 1 } else { 0 })
                                .collect::<Vec<_>>(),
                        )?;
                }
                ParamArrayStorage::Float(value) => {
                    file.new_dataset::<f64>()
                        .gzip(6)
                        .create(&path, (value.len(),))?
                        .write(&value[..])?;
                }
                _ => {}
            }
        }

        // Write params
        for (name, param) in &param_bag.params {
            let path = format!("/parameters/{}", name);

            match param {
                Param::Bool(value) => {
                    file.new_dataset::<u8>()
                        .create(&path, (1,))?
                        .write(&[if *value { 1 } else { 0 }])?;
                }
                Param::Float(value) => {
                    file.new_dataset::<f64>()
                        .create(&path, (1,))?
                        .write(&[*value])?;
                }
                _ => {}
            }
        }

        writeln!(meta, "    </Grid>")?;
        writeln!(meta, "  </Domain>")?;
        writeln!(meta, "</Xdmf>")?;
    }

    Ok(())
}
