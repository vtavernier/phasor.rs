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
sudo apt install build-essential cmake ninja-build llvm
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

On Windows, if you have Chocolatey installed:
```
cinst rustup.install visualstudio2019community visualstudio2019-workload-vctools cmake ninja llvm
```

## Usage

### Standalone usage

In a terminal, to compile and run the standalone binary:

```bash
# Build and launch the test executable
cargo run
```

Then, press `Space` to start the optimization.

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

```jl
import PhasorOpt, Images
r = PhasorOpt.framex(512; iterations = 32, filter_bandwidth = 1.5 / sqrt(pi), opt_method = PhasorOpt.OM_AVERAGE)
Images.Gray.(angle.(r[1]) / 2pi .+ .5)
```

## Examples

These examples are generated using the Julia interface. You can also link to
`target/debug/libphasor.so` and use the exported C API (see the generated
`phasor/phasoropt.h` header).

```jl
# Phase aligned and filtered noise
PhasorOpt.framex(512; iterations = 32, filter_bandwidth = 1.5 / sqrt(pi), opt_method = PhasorOpt.OM_AVERAGE)

# Phase aligned and filtered noise, fixed orientation (AM_STATIC)
PhasorOpt.framex(512; iterations = 32, angle_mode = PhasorOpt.AM_STATIC, filter_bandwidth = 1.5 / sqrt(pi), opt_method = PhasorOpt.OM_AVERAGE)

# Phase alignment only (filter_bandwidth = 0.)
PhasorOpt.framex(512; iterations = 32, opt_method = PhasorOpt.OM_AVERAGE)

# Filtering only (iterations = 0)
PhasorOpt.framex(512; filter_bandwidth = 1.5 / sqrt(pi))

# Show detailed documentation for all parameters
?PhasorOpt.framex
```

## Project structure

* [`shaders/`](shaders/): compute and fragment shaders which implement the phase alignment, noise rendering and filtering
  * [`display.frag`](shaders/display.frag): noise evaluation and display
  * [`display.vert`](shaders/display.vert): full-screen quad vertex shader
  * [`fields.glsl`](shaders/fields.glsl): implementation of input parameter fields
  * [`gabor.glsl`](shaders/gabor.glsl): implementation of the noise kernels (regular and filtered)
  * [`init.comp`](shaders/init.comp): kernel initialization compute shader
  * [`opt.comp`](shaders/opt.comp): phase alignment compute shader
* [`src/`](src/): supporting code for noise evaluation
  * [`PhasorOpt.jl`](src/PhasorOpt.jl): Julia module interface
  * [`*.rs`](src/): supporting Rust code for OpenGL context creation
* [`vendor/`](vendor/): vendored third-party dependencies for reproducible builds

## Copyright

This code is part of a submission to SIGGRAPH Asia 2020, and as such all rights
are reserved to the original authors. This covers all files outside of the
`vendor/` directory, regardless of the presence of a copyright notice in the
headers.
