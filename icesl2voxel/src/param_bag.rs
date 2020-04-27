use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;

use itertools::Itertools;
use lazy_static::lazy_static;
use ndarray::prelude::*;
use regex::Regex;
use serde_derive::{Deserialize, Serialize};
use xml::reader::{EventReader, XmlEvent};

use super::param::Param;
use super::param_array::ParamArray;
use super::param_field::ParamField;

lazy_static! {
    static ref ELEMENT_NAME_PARAM_RE: Regex = Regex::new(r"^(.*)_(\d*)$").unwrap();
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct ParamBag {
    param_fields: HashMap<String, ParamField>,
    param_arrays: HashMap<String, ParamArray>,
    params: HashMap<String, Param>,
}

impl ParamBag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse(src: &mut dyn std::io::Read) -> Result<Self, failure::Error> {
        let mut param_bag = ParamBag::new();
        let mut field_names = HashSet::new();

        let parser = EventReader::new(src);
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

                        debug!("adding field {}", name);
                        field_names.insert(name.to_owned());
                        param_bag.add_field(&name, &attributes[..])?;
                    } else if let Some(attribute) = attributes
                        .iter()
                        .find(|attr| attr.name.local_name == "value")
                    {
                        if let Some(captures) = ELEMENT_NAME_PARAM_RE.captures(&name.local_name) {
                            // Discard values which are already fields
                            if field_names.contains(captures.get(1).unwrap().as_str()) {
                                continue;
                            }

                            if captures.get(2).unwrap().as_str() == "0" {
                                debug!("adding array {}", &captures.get(1).unwrap().as_str());
                            }

                            param_bag.add_array_item(&captures, &attribute.value)?;
                        } else {
                            debug!("adding parameter {}", name.local_name);
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

        Ok(param_bag)
    }

    pub fn convert_to_field(&mut self, name: &str) -> Result<(), failure::Error> {
        if let Some(array) = self.param_arrays.get(name) {
            // Insert converted field
            if let Some(field) = self
                .param_fields
                .values()
                .next()
                .and_then(|first_field| first_field.derive_from_array(&array))
            {
                // Add field to list
                self.param_fields.insert(name.to_owned(), field);

                // Delete array now that it has been converted
                self.param_arrays.remove(name);

                Ok(())
            } else {
                Err(failure::err_msg(format!(
                    "cannot derive field from param array {}",
                    name
                )))
            }
        } else {
            Err(failure::err_msg(format!("param array {} not found", name)))
        }
    }

    pub fn is_field(&self, name: &str) -> bool {
        self.param_fields.contains_key(name)
    }

    pub fn assemble_spherical(
        &mut self,
        name: &str,
        source_names: &[impl AsRef<str>],
    ) -> Result<&ParamField, failure::Error> {
        let sources: Result<Vec<&ParamField>, _> = source_names
            .iter()
            .map(|src_name| {
                self.param_fields.get(src_name.as_ref()).ok_or_else(|| {
                    failure::err_msg(format!("{} field not found", src_name.as_ref()))
                })
            })
            .collect();

        let sources = sources?;

        let dim = sources[0].dim();
        let mut data = ndarray::Array4::zeros((dim.0, dim.1, dim.2, 3));

        // Generate array from spherical coordinates
        let param_r = if sources.len() >= 3 {
            sources[0].as_f64_array(1.0).ok_or_else(|| {
                failure::err_msg(format!(
                    "could not convert {} field to float",
                    source_names[0].as_ref()
                ))
            })?
        } else {
            Cow::Owned(ndarray::Array3::ones((dim.0, dim.1, dim.2)))
        };

        let param_theta = if sources.len() >= 1 {
            let idx = if sources.len() >= 3 { 1 } else { 0 };
            sources[idx].as_f64_array(180.0).ok_or_else(|| {
                failure::err_msg(format!(
                    "could not convert {} field to float",
                    source_names[idx].as_ref()
                ))
            })?
        } else {
            Cow::Owned(ndarray::Array3::zeros((dim.0, dim.1, dim.2)))
        };

        let param_phi = if sources.len() >= 2 {
            let idx = if sources.len() >= 3 { 2 } else { 1 };
            sources[idx].as_f64_array(360.0).ok_or_else(|| {
                failure::err_msg(format!(
                    "could not convert {} field to float",
                    source_names[idx].as_ref()
                ))
            })?
        } else {
            Cow::Owned(ndarray::Array3::zeros((dim.0, dim.1, dim.2)))
        };

        azip!((mut vec in data.lanes_mut(Axis(3)), r in &*param_r, theta in &*param_theta, phi in &*param_phi)
        {
            let theta = *theta / 360.0 * 2.0 * std::f64::consts::PI;
            let phi = *phi / 360.0 * 2.0 * std::f64::consts::PI;

            vec[0] = *r * phi.cos() * -theta.sin();
            vec[1] = *r * phi.cos() * -theta.cos();
            vec[2] = *r * phi.sin();
        });

        let field = sources[0].derive_vec3_from_field(data);

        self.param_fields.insert(name.to_owned(), field);
        Ok(self.param_fields.get(name).unwrap())
    }

    fn add_item(&mut self, name: &str, value: &str) -> Result<(), failure::Error> {
        self.params.insert(name.to_owned(), Param::try_from(value)?);
        Ok(())
    }

    fn add_field(
        &mut self,
        name: &str,
        attributes: &[xml::attribute::OwnedAttribute],
    ) -> Result<(), failure::Error> {
        // Add to known fields
        self.param_fields
            .insert(name.to_owned(), ParamField::from_attr(attributes)?);
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

    pub fn write_hdf5(&self, file: &hdf5::File) -> Result<(), failure::Error> {
        // Assume all fields share the same grid
        let first_field = self.param_fields.iter().next().unwrap().1;

        // For converting into standard layout
        let dim = first_field.dim();
        let mut std_layout = ndarray::Array3::<u8>::zeros((dim.0, dim.1, dim.2));

        // Write fields
        for (name, field) in &self.param_fields {
            let path = format!("/fields/{}", name);

            // Since we assume all fields have the same bounding box, check that it's actually the
            // case
            if !field.has_same_box(first_field) {
                warn!("field {} doesn't have the same bounding box as the first field, this may lead to inconsistencies", name);
            }

            field.write_hdf5(&path, &file, &mut std_layout)?;
        }

        // Write array params
        for (name, array) in &self.param_arrays {
            let path = format!("/arrays/{}", name);

            match array.write_hdf5(&path, &file) {
                Err(err) => {
                    warn!("array {} not written to HDF5 file: {}", name, err);
                }
                _ => {}
            }
        }

        // Write params
        for (name, param) in &self.params {
            let path = format!("/parameters/{}", name);

            match param.write_hdf5(&path, &file) {
                Err(err) => {
                    error!("param {} not written to HDF5 file: {}", name, err);
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub fn write_xdmf(
        &self,
        (x_offset, y_offset, z_offset): (f64, f64, f64),
        h5_file_name: &str,
        dest: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        writeln!(dest, "<?xml version=\"1.0\" encoding=\"utf-8\" ?>")?;
        writeln!(dest, "<!DOCTYPE Xdmf SYSTEM \"Xdmf.dtd\" []>")?;
        writeln!(dest, "<Xdmf Version=\"2.0\">")?;
        writeln!(dest, "  <Domain>")?;

        // Assume all fields share the same grid
        let first_field = self.param_fields.iter().next().unwrap().1;

        writeln!(dest, "    <Grid Name=\"root\" GridType=\"Collection\">")?;
        writeln!(
            dest,
            "      <Grid Name=\"field_mesh\" GridType=\"Uniform\">"
        )?;
        writeln!(dest, "        <Topology Name=\"field_topo\" TopologyType=\"3DCoRectMesh\" NumberOfElements=\"{z} {y} {x}\" />",
            x = first_field.dim().2 + 1,
            y = first_field.dim().1 + 1,
            z = first_field.dim().0 + 1)?;
        writeln!(
            dest,
            "        <Geometry Name=\"field_geo\" Type=\"ORIGIN_DXDYDZ\">"
        )?;

        let x_scale = (first_field.field_box_mm_max_x - first_field.field_box_mm_min_x)
            / first_field.dim().2 as f64;
        let y_scale = (first_field.field_box_mm_max_y - first_field.field_box_mm_min_y)
            / first_field.dim().1 as f64;
        let z_scale = (first_field.field_box_mm_max_z - first_field.field_box_mm_min_z)
            / first_field.dim().0 as f64;

        // TODO: Write this in HDF
        writeln!(dest, "          <DataItem Format=\"XML\" Dimensions=\"3\">")?;
        writeln!(
            dest,
            "            {z} {y} {x}",
            x = x_offset + (first_field.field_box_mm_max_x - first_field.field_box_mm_min_x) / -2.0,
            y = y_offset + (first_field.field_box_mm_max_y - first_field.field_box_mm_min_y) / -2.0,
            z = z_offset + (first_field.field_box_mm_max_z - first_field.field_box_mm_min_z) / -2.0,
        )?;
        writeln!(dest, "          </DataItem>")?;
        // TODO: Write this in HDF
        writeln!(dest, "          <DataItem Format=\"XML\" Dimensions=\"3\">")?;
        writeln!(
            dest,
            "            {z} {y} {x}",
            x = x_scale,
            y = y_scale,
            z = z_scale,
        )?;
        writeln!(dest, "          </DataItem>")?;
        writeln!(dest, "        </Geometry>")?;

        // Write fields
        for (name, field) in &self.param_fields {
            let path = format!("/fields/{}", name);
            if let Some((data_type, precision, components)) = field.xdmf_type() {
                // Since we assume all fields have the same bounding box, check that it's actually the
                // case
                if !field.has_same_box(first_field) {
                    warn!("field {} doesn't have the same bounding box as the first field, this may lead to inconsistencies", name);
                }

                {
                    let path = format!("{}/data", path);
                    let dim: (usize, usize, usize, usize) = field.dim().into();

                    // Export all fields to XDMF
                    writeln!(
                        dest,
                        "        <Attribute Name=\"{name}\" AttributeType=\"{attribute_type}\" Center=\"Cell\">",
                        name = name,
                        attribute_type = if components == 1 {
                            "Scalar"
                        } else {
                            "Vector"
                        }
                    )?;
                    writeln!(dest, "          <DataItem Dimensions=\"{z} {y} {x}{d}\" Format=\"HDF5\" DataType=\"{data_type}\" Precision=\"{precision}\">",
                        x = dim.2,
                        y = dim.1,
                        z = dim.0,
                        data_type = data_type,
                        precision = precision,
                        d = if components == 1 {
                            "".to_owned()
                        } else {
                            format!(" {}", components)
                        },
                    )?;
                    writeln!(dest, "            {}:{}", h5_file_name, path)?;
                    writeln!(dest, "          </DataItem>")?;
                    writeln!(dest, "        </Attribute>")?;
                }
            }
        }

        writeln!(dest, "      </Grid>")?;

        // Write array params
        let mut arrays: Vec<_> = self.param_arrays.iter().collect();
        arrays.sort_by_key(|(_, array)| array.len());

        for (len, arrays) in &arrays.iter().group_by(|(_, array)| array.len()) {
            let mut scale = None;

            for (name, array) in arrays {
                let path = format!("/arrays/{}", name);

                if scale.is_none() {
                    let array_x_scale =
                        (first_field.field_box_mm_max_x - first_field.field_box_mm_min_x) / 1.0;
                    let array_y_scale =
                        (first_field.field_box_mm_max_y - first_field.field_box_mm_min_y) / 1.0;
                    let array_z_scale = (first_field.field_box_mm_max_z
                        - first_field.field_box_mm_min_z)
                        / len as f64;

                    writeln!(
                        dest,
                        "      <Grid Name=\"array{len:03}_mesh\" GridType=\"Uniform\">",
                        len = len,
                    )?;
                    writeln!(dest, "        <Topology Name=\"array{len:03}_topo\" TopologyType=\"3DCoRectMesh\" NumberOfElements=\"{z} {y} {x}\" />",
                        x = 1 + 1,
                        y = 1 + 1,
                        z = len + 1,
                        len = len,
                    )?;
                    writeln!(
                        dest,
                        "        <Geometry Name=\"array{len:03}_geo\" Type=\"ORIGIN_DXDYDZ\">",
                        len = len,
                    )?;

                    // TODO: Write this in HDF
                    writeln!(dest, "          <DataItem Format=\"XML\" Dimensions=\"3\">")?;
                    writeln!(
                        dest,
                        "            {z} {y} {x}",
                        x = x_offset
                            + (first_field.field_box_mm_max_x - first_field.field_box_mm_min_x)
                                / -2.0,
                        y = y_offset
                            + (first_field.field_box_mm_max_y - first_field.field_box_mm_min_y)
                                / -2.0,
                        z = z_offset
                            + (first_field.field_box_mm_max_z - first_field.field_box_mm_min_z)
                                / -2.0,
                    )?;
                    writeln!(dest, "          </DataItem>")?;
                    // TODO: Write this in HDF
                    writeln!(dest, "          <DataItem Format=\"XML\" Dimensions=\"3\">")?;
                    writeln!(
                        dest,
                        "            {z} {y} {x}",
                        x = array_x_scale,
                        y = array_y_scale,
                        z = array_z_scale,
                    )?;
                    writeln!(dest, "          </DataItem>")?;
                    writeln!(dest, "        </Geometry>")?;

                    scale = Some((array_x_scale, array_y_scale, array_z_scale));
                }

                let mut xdmf_type = array.xdmf_type();

                if *name != "infill_theta" {
                    xdmf_type = None;
                }

                if let Some((data_type, precision)) = xdmf_type {
                    writeln!(
                        dest,
                        "        <Attribute Name=\"{name}\" AttributeType=\"Scalar\" Center=\"Cell\">",
                        name = name,
                    )?;
                    writeln!(dest, "          <DataItem Dimensions=\"{z} {y} {x}\" Format=\"HDF5\" DataType=\"{data_type}\" Precision=\"{precision}\">",
                        x = 1,
                        y = 1,
                        z = array.len(),
                        data_type = data_type,
                        precision = precision,
                    )?;
                    writeln!(dest, "            {}:{}", h5_file_name, path)?;
                    writeln!(dest, "          </DataItem>")?;
                    writeln!(dest, "        </Attribute>")?;
                }
            }

            writeln!(dest, "      </Grid>")?;
        }

        writeln!(dest, "    </Grid>")?;
        writeln!(dest, "  </Domain>")?;
        writeln!(dest, "</Xdmf>")?;

        Ok(())
    }
}
