# icesl2voxel

icesl2voxel is a tool to convert files from the IceSL XML format to HDF5+XDMF files which can
be loaded inside Paraview for visualization or from other code for further processing.

### Usage

    # Extract fields and parameters from file.xml into file.h5 (and file.xdmf)
    cargo run --release -- -i file.xml -o file.h5

### Author

Vincent Tavernier <vince.tavernier@gmail.com>
