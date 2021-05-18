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

use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

use structopt::StructOpt;

pub struct FieldMap {
    output_name: String,
    coords: Vec<String>,
}

impl std::str::FromStr for FieldMap {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kv_parts: Vec<_> = s.splitn(2, '=').collect();
        let coord_parts: Vec<_> = kv_parts[1].split(',').map(str::to_owned).collect();

        Ok(Self {
            output_name: kv_parts[0].to_owned(),
            coords: coord_parts,
        })
    }
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
    output: PathBuf,

    /// List of array parameters to force as fields
    #[structopt(long)]
    force_field: Vec<String>,

    /// List of fields to assemble as spherical vector fields
    #[structopt(
        long,
        default_value = "infill_dir=infill_theta,infill_phi",
        use_delimiter = true,
        value_delimiter = ":"
    )]
    assemble_spherical: Vec<FieldMap>,

    /// Gcode to extract extruded segments from
    #[structopt(short, long)]
    gcode: Option<PathBuf>,

    /// Number of samples for voxelizing geometry
    #[structopt(long, default_value = "4")]
    samples: std::num::NonZeroUsize,

    /// Export depth images for input geometry voxelizing
    #[structopt(long)]
    export_depth_images: bool,

    /// Export arrays in XDMF
    #[structopt(long)]
    xdmf_export_arrays: bool,

    /// Compute output geometry statistics
    #[structopt(
        long,
        default_value = "output_stats=10",
        use_delimiter = true,
        value_delimiter = ":"
    )]
    output_statistics: Vec<FieldMap>,

    /// Resample input fields using nearest interpolation and the input geometry mask
    #[structopt(
        long,
        default_value = "input_percentage=infill_percentage:input_dir=infill_dir:input_isotropy=infill_isotropy",
        use_delimiter = true,
        value_delimiter = ":"
    )]
    resample_fields: Vec<FieldMap>,

    /// Sampling factor in the XY plane for output voxelization
    #[structopt(long, default_value = "1.0")]
    xy_sampling_factor: f32,

    /// Number of rays to sample directions in output geometry
    #[structopt(long, default_value = "32")]
    dir_samples: usize,

    /// Pad all written fields with a single layer of 0 to generate closed surfaces
    #[structopt(long)]
    pad_fields: bool,
}

impl Opts {
    pub fn get_force_field(&self) -> HashSet<String> {
        let mut res =
            HashSet::with_capacity(self.force_field.len() + self.assemble_spherical.len() * 3);

        res.extend(self.force_field.iter().cloned());
        res.extend(
            self.assemble_spherical
                .iter()
                .flat_map(|df| df.coords.iter())
                .cloned(),
        );

        res
    }
}

mod geometry;
mod param;
mod param_array;
mod param_bag;
mod param_field;
mod parse;
mod stats;
mod utils;
mod voxelizer;

use param_bag::ParamBag;

fn write_hdf5(output: &Path, param_bag: &ParamBag) -> Result<(), failure::Error> {
    let _e = hdf5::silence_errors();
    let file = hdf5::File::create(&output)?;
    param_bag.write_hdf5(&file)
}

fn write_xdmf(
    offsets: nalgebra::Vector3<f32>,
    param_bag: &ParamBag,
    h5_file_name: &str,
    opts: &Opts,
) -> Result<(), failure::Error> {
    let mut meta = File::create(opts.output.with_extension("xdmf"))?;
    Ok(param_bag.write_xdmf(offsets, h5_file_name, &mut meta, opts.xdmf_export_arrays)?)
}

#[paw::main]
fn main(opts: Opts) -> Result<(), failure::Error> {
    env_logger::Builder::from_env(
        env_logger::Env::new().default_filter_or("icesl2voxel=debug,opengl=debug"),
    )
    .format_timestamp(None)
    .init();

    let mut param_bag = {
        let start = Instant::now();

        let file = File::open(&opts.input)?;
        let mut file = BufReader::new(file);
        let bag = ParamBag::parse(&mut file)?;

        debug!("loaded parameters in {:.2}ms", start.elapsed().as_millis());

        bag
    };

    for force_field in &opts.get_force_field() {
        if param_bag.get_field(force_field).is_some() {
            // Nothing to do
        } else {
            let start = Instant::now();

            match param_bag.convert_to_field(force_field) {
                Ok(_) => info!(
                    "converted {} to a field in {:.2}ms",
                    force_field,
                    start.elapsed().as_millis()
                ),
                Err(error) => error!("could not convert {} to a field: {}", force_field, error),
            }
        }
    }

    for assemble_spherical in &opts.assemble_spherical {
        let start = Instant::now();

        match param_bag.assemble_spherical(
            &assemble_spherical.output_name,
            &assemble_spherical.coords[..],
        ) {
            Ok(_) => info!(
                "assembled {} as spherical vector field in {:.2}ms",
                assemble_spherical.output_name,
                start.elapsed().as_millis(),
            ),
            Err(error) => error!(
                "could not assemble {}: {}",
                assemble_spherical.output_name, error
            ),
        }
    }

    let (geometry_bounding_box, offsets, mesh) = if let Some(mesh_path) = &opts.mesh {
        let start = Instant::now();

        let mesh = geometry::load_mesh(mesh_path)?;
        let bbox = geometry::get_bounding_box(&mesh);
        let offsets = bbox.center();

        debug!(
            "loaded bounding box for geometry in {:.2}ms: {:?}",
            start.elapsed().as_millis(),
            bbox
        );

        (Some(bbox), offsets, Some(mesh))
    } else {
        (None, nalgebra::Vector3::zeros(), None)
    };

    // Voxelize printed geometry
    if let Some(gcode_path) = &opts.gcode {
        let start = Instant::now();

        let voxelized_field =
            voxelizer::voxelize_gcode(gcode_path, opts.samples.into(), opts.xy_sampling_factor)?;

        debug!(
            "voxelized printed geometry in {:.2}ms",
            start.elapsed().as_millis()
        );

        if let Some(mesh) = &mesh {
            let start = Instant::now();

            let voxelized_mesh = voxelizer::voxelize_mesh(
                mesh,
                geometry_bounding_box.as_ref().unwrap(),
                &voxelized_field,
                opts.export_depth_images,
            )?;

            debug!(
                "voxelized input geometry in {:.2}ms",
                start.elapsed().as_millis()
            );

            for input_spec in &opts.resample_fields {
                if let Some(field) = param_bag.get_field(&input_spec.coords[0]) {
                    let start = Instant::now();

                    let field = field.resample(&voxelized_mesh);
                    debug!(
                        "resampled {} as {} in {:.2}ms",
                        input_spec.coords[0],
                        input_spec.output_name,
                        start.elapsed().as_millis()
                    );

                    param_bag.add_field(&input_spec.output_name, field);
                } else {
                    error!("field {} not found for resampling", input_spec.coords[0]);
                }
            }

            for out_spec in &opts.output_statistics {
                let start = Instant::now();

                let output_stats = stats::compute_output_stats(
                    &voxelized_field,
                    &voxelized_mesh,
                    param_bag.get_field("input_dir"),
                    out_spec
                        .coords
                        .iter()
                        .next()
                        .ok_or_else(|| failure::err_msg("you need to specify the kernel size"))
                        .and_then(|f| f.parse::<f32>().map_err(|e| e.into()))?,
                    opts.dir_samples,
                )?;

                debug!(
                    "computed {} statistics in {:.2}ms",
                    out_spec.output_name,
                    start.elapsed().as_millis()
                );

                param_bag.add_field(
                    &format!("{}_mean", out_spec.output_name),
                    output_stats.mean_field,
                );
                param_bag.add_field(
                    &format!("{}_mean_confidence", out_spec.output_name),
                    output_stats.mean_field_confidence,
                );
                param_bag.add_field(
                    &format!("{}_dir", out_spec.output_name),
                    output_stats.dir_field,
                );
                param_bag.add_field(
                    &format!("{}_dir_length", out_spec.output_name),
                    output_stats.dir_length_field,
                );
                param_bag.add_field(
                    &format!("{}_dir_change", out_spec.output_name),
                    output_stats.dir_change_field,
                );
                if let Some(dir_correlation) = output_stats.dir_correlation {
                    param_bag.add_field(
                        &format!("{}_dir_correlation", out_spec.output_name),
                        dir_correlation,
                    );
                }
            }

            param_bag.add_field("input_geometry", voxelized_mesh);
        }

        param_bag.add_field("output_geometry", voxelized_field);
    }

    if opts.pad_fields {
        param_bag.pad_fields(1);
    }

    let h5_file_name = opts.output.file_name().unwrap().to_string_lossy();

    // Write XDMF
    write_xdmf(offsets, &param_bag, &h5_file_name, &opts)?;

    // Write HDF5
    write_hdf5(&opts.output, &param_bag)?;

    Ok(())
}
