# icesl2voxel

icesl2voxel is a tool to convert files from the IceSL XML format to HDF5+XDMF files which can
be loaded inside Paraview for visualization or from other code for further processing.

It also computes printed geometry as a voxel grid from the generated printer paths. This is then
used to compute output statistics and correlation with input fields.

### Usage

    # Extract fields and parameters from file.xml into file.h5 (and file.xdmf),
    # read input geometry from file.stl, compute output geometry from printer Gcode

    cargo run --release -- -i file.xml -o file.h5 -m file.stl -g file.gcode

### Author

Vincent Tavernier <vince.tavernier@gmail.com>
