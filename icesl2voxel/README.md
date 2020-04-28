# icesl2voxel

icesl2voxel is a tool to convert files from the IceSL XML format to HDF5+XDMF files which can
be loaded inside Paraview for visualization or from other code for further processing.

### Usage

    # Extract fields and parameters from file.xml into file.h5 (and file.xdmf),
    # read bounding box adjustments from file.stl, and assemble spherical coordinates
    # fields infill_theta and infill_phi into the 3D vector field infill_dir

    cargo run --release -- -i file.xml -o file.h5 -m file.stl --assemble-spherical infill_dir=infill_theta,infill_phi

### Author

Vincent Tavernier <vince.tavernier@gmail.com>
