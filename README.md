# phasor.rs

Implementation of a microstructure generation algorithm based on Phasor noise.

The core part of the method is implemented in Rust and OpenGL/GLSL, and exposes
an API so it can be interfaced with other clients. We used
[Julia](https://julialang.org), but any language which supports the C ABI can be
used.

## Installing

### Build from source

You'll need the following dependencies available in your PATH:

* A C++ Compiler
* [Rust](https://rustup.rs/)
* [CMake](https://cmake.org/)
* [Ninja](https://ninja-build.org/)
* [libclang (LLVM)](https://llvm.org/)

On Debian these can be installed with apt:

```
# Other dependencies
sudo apt install build-essentials cmake ninja-build clang
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

On Windows, if you have Chocolatey installed:
```
cinst rustup.install visualstudio2019community visualstudio2019-workload-vctools cmake ninja llvm
```

Then, in a terminal:

```bash
# Get the source
git clone https://github.com/vtavernier/phasor.rs.git
cd phasor.rs

# Build and launch the test executable
cargo run
```

### Usage from Julia

This repository contains the necessary code to be used as a Julia module.
Assuming you cloned this repository in `../phasor.rs` you can add the Julia module
as a dependency of the current project with the following code:

```jl
import Pkg
Pkg.develop(Pkg.PackageSpec(path="../phasor.rs"))
Pkg.build("PhasorOpt")
```

Then you can use it by importing the relevant module (assuming you have the `Images`
package installed, using `Pkg.add("Images")`):

```
import PhasorOpt, Images
r = PhasorOpt.framex(512; iterations = 32, filter_bandwidth = 1.5 / sqrt(pi))
Images.Gray.(angle.(r[1]) / 2pi .+ .5)
```
