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

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use structopt::StructOpt;

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

    /// List of array parameters to force as fields
    #[structopt(long)]
    force_field: Vec<String>,
}

mod geometry;
mod param;
mod param_array;
mod param_bag;
mod param_field;
mod parse;

use param_bag::ParamBag;

fn write_hdf5(output: &Path, param_bag: &ParamBag) -> Result<(), failure::Error> {
    let _e = hdf5::silence_errors();
    let file = hdf5::File::create(&output)?;
    param_bag.write_hdf5(&file)
}

fn write_xdmf(
    (x_offset, y_offset, z_offset): (f64, f64, f64),
    param_bag: &ParamBag,
    h5_file_name: &str,
    dest: &Path,
) -> Result<(), failure::Error> {
    let mut meta = File::create(dest)?;
    Ok(param_bag.write_xdmf((x_offset, y_offset, z_offset), h5_file_name, &mut meta)?)
}

#[paw::main]
fn main(opts: Opts) -> Result<(), failure::Error> {
    env_logger::Builder::from_default_env()
        .format_timestamp(None)
        .format_module_path(false)
        .init();

    let file = File::open(&opts.input)?;
    let mut file = BufReader::new(file);

    let mut param_bag = ParamBag::parse(&mut file)?;

    for force_field in &opts.force_field {
        match param_bag.convert_to_field(force_field) {
            Ok(_) => info!("converted {} to a field", force_field),
            Err(error) => error!("could not convert {} to a field: {}", force_field, error),
        }
    }

    if let Some(output) = opts.output {
        let h5_file_name = output.file_name().unwrap().to_string_lossy();

        // We only need 2 threads for now
        rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build_global()?;

        let (xdmf, h5) = rayon::join(
            {
                let mesh = opts.mesh.clone();
                let output = (&output).clone();
                let param_bag = (&param_bag).clone();

                move || -> Result<(), failure::Error> {
                    // Load geometry and compute origin
                    let (x_offset, y_offset, z_offset) = geometry::read_offsets(&mesh)?;

                    // Write XDMF
                    write_xdmf(
                        (x_offset, y_offset, z_offset),
                        &param_bag,
                        &h5_file_name,
                        &output.with_extension("xdmf"),
                    )?;

                    Ok(())
                }
            },
            {
                let output = (&output).clone();
                let param_bag = (&param_bag).clone();

                move || {
                    // Write HDF5
                    write_hdf5(&output, &param_bag)
                }
            },
        );

        xdmf?;
        h5?;
    }

    Ok(())
}
