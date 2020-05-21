"""
    PhasorOpt

Main module for the interface to the Phasor noise optimizer used
to generate microstructures.

See the `framex` method documentation for more details
"""
module PhasorOpt

# Load binary dependency
const deps_file = joinpath(dirname(@__FILE__), "..", "deps", "deps.jl")
if !isfile(deps_file)
    error("PhasorOpt.jl is not installed properly, run Pkg.build(\"PhasorOpt\") and restart Julia.")
end
include(deps_file)

# Load API
include(joinpath(dirname(@__FILE__), "..", "deps", "phasor_common.jl"))
include(joinpath(dirname(@__FILE__), "..", "deps", "phasor_api.jl"))

function __init__()
    check_deps()
end

# Actual Julia PhasorOpt interface
"""
    init()

Initialize the optimizer state. Doesn't need to be called directly as framex
ensures initialization before running.
"""
init() = pg_init(true)

"""
    terminate()

Free resources used by the optimizer. Only needed for manual reset of the
optimizer state.
"""
terminate() = pg_terminate()

"""
    get_max_kernels()

Return the maximum number of kernels per optimization cells the optimizer
supports. This method is used to check what is the upper bound on the number of
kernels that was compiled in the native code and shaders.
"""
get_max_kernels() = pg_get_max_kernels()

"""
    framex(width; [iterations = 20, [seed = 2304, ...]])

Compute a 2D instance of optimized Phasor noise.

# Parameters

* `width` (required): width of the generated example in pixels
* `kernel_count` (default: 8): number of noise kernels per optimization cell
* `iterations` (default: 0): number of optimization iterations
* `seed` (default: 1): random seed for noise generation
* `height` (default: width): height of the generated example in pixels
* `angle_mode` (default: `AM_GAUSS`): type of orientation field to generate
* `angle` (default: 0.0): base angle for the orientation field, see `fields.glsl` for details on how this is used
* `angle_bandwidth` (default: 0.1): for `AM_GAUSS`, bandwidth of the Gaussian orientation field
* `angle_range` (default: pi): range of orientation variation in the orientation field
* `frequency_mode` (default: `FM_STATIC`): type of frequency field to generate
* `frequency` (default: 64.0): base frequency for the frequency field, see `fields.glsl` for details on how this is used
* `frequency_max` (default: 128.0): for `FM_GAUSS`, maximum frequency in the frequency field
* `frequency_bandwidth` (default: 0.1): for `FM_GAUSS`, bandwidth of the Gaussian frequency field
* `noise_bandwidth` (default: 3.0 / sqrt(pi)): bandwidth of the noise kernels
* `filter_bandwidth` (default: 0.0): bandwidth of the filtering kernel
* `filter_modulation` (default: 4.0): linear factor of the filter attenuation (resp. to orientation)
* `filter_modpower` (default: 1.0): power of the attenuation factor (resp. to orientation)
* `isotropy_mode` (default: IM_ANISOTROPIC): type of isotropy field to generate
* `isotropy_min` (default: 0.0): minimum isotropy amount
* `isotropy_max` (default: 1.0): maximum isotropy amount
* `isotropy_bandwidth` (default: 0.1): for `IM_GAUSS`, bandwidth of the Gaussian isotropy field
* `isotropy_modulation` (default: 2.0): linear factor of the filter attenuation (resp. to isotropy)
* `isotropy_power` (default: 4.0): power of the attenuation factor (resp. to isotropy)
* `cell_mode` (default: `CM_CLAMP`): behavior at boundary of cells
* `opt_method` (default: `OM_OPTIMIZE`): optimization strategy
* `export_extra` (default: false): export extra fields in the result
* `init_kernels` (default: true): initialize kernels before optimization. This
    can be set to false to iteratively optimize the same noise instance over.

# Returns

A tuple containing, in order:
* Complex Phasor noise
* Generated orientation map
* Generated frequency map
* Generated isotropy map
* (if `export_extra` is `true`) Generated attenuation factor
* (if `export_extra` is `true`) Internal state value (see display.frag for details)
"""
function framex(width;
                kernel_count = 8,
                iterations = 0,
                seed = 1,
                height = width,
                angle_mode = AM_GAUSS,
                angle = 0.0,
                angle_bandwidth = 0.1,
                angle_range = pi,
                frequency_mode = FM_STATIC,
                frequency = 64.0,
                frequency_max = 128.0,
                frequency_bandwidth = 0.1,
                noise_bandwidth = 3.0 / sqrt(pi),
                filter_bandwidth = 0.0,
                filter_modulation = 4.0,
                filter_modpower = 1.0,
                isotropy_mode = IM_ANISOTROPIC,
                isotropy_min = 0.0,
                isotropy_max = 1.0,
                isotropy_bandwidth = 0.1,
                isotropy_modulation = 2.0,
                isotropy_power = 4.0,
                cell_mode = CM_CLAMP,
                opt_method = OM_OPTIMIZE,
                export_extra = false,
                init_kernels = true)
  # Note that we adjust for the different frequency references
  # JL frequency = 64 -> phasoroptgen frequency = 2
  # -> Scaling factor of 32

  max_kernel_count = get_max_kernels()
  if kernel_count > get_max_kernels()
    error("too many kernels (max kernel count: " * string(max_kernel_count) * "): " * string(kernel_count))
  end

  unsafe_ptr = pg_optimize_ex(
                      width, height,
                      kernel_count,
                      seed, iterations,
                      angle_mode,
                      angle,
                      angle_bandwidth,
                      angle_range,
                      frequency_mode,
                      frequency / 32.0,
                      frequency_max / 32.0,
                      frequency_bandwidth,
                      noise_bandwidth,
                      filter_bandwidth,
                      filter_modulation,
                      filter_modpower,
                      isotropy_mode,
                      isotropy_min,
                      isotropy_max,
                      isotropy_bandwidth,
                      isotropy_modulation,
                      isotropy_power,
                      cell_mode,
                      opt_method,
                      DM_COMPLEX,
                      init_kernels)

  extra_ptr = pg_get_extra()

  # Check for errors
  if unsafe_ptr == C_NULL || extra_ptr == C_NULL
    error(get_error())
  end

  # Wrap float ptr into an array
  image_data = Base.unsafe_wrap(Array, unsafe_ptr, (width, height, 4))
  extra_data = Base.unsafe_wrap(Array, extra_ptr, (width, height, 4))

  # Reinterpret it in "Julia" order
  reinterpreted = PermutedDimsArray(reverse(reshape(image_data, (4, width, height)), dims=3), (1,3,2))
  reinter_extra = PermutedDimsArray(reverse(reshape(extra_data, (4, width, height)), dims=3), (1,3,2))

  # Output (noise, orientation, frequency, isotropy)
  result = (
    Complex.(reinterpreted[1,:,:], reinterpreted[2,:,:]),
    copy(reinterpreted[3,:,:]),
    reinterpreted[4,:,:] .* 32.0,
    copy(reinter_extra[1,:,:])
  )

  if export_extra
    (result..., copy(reinter_extra[2,:,:]), copy(reinter_extra[3,:,:]))
  else
    result
  end
end

"""
    kernel_width(width, bandwidth, filter_bandwidth)

Return the width in pixels of the noise kernel given its bandwidth and the filtering bandwidth.
"""
function kernel_width(width, bandwidth, filter_bandwidth)
  pg_noise_kernel_width(width, bandwidth, filter_bandwidth)
end

"""
    kernel_width(width, bandwidth)

Return the width in pixels of the noise kernel given its bandwidth and no filter.
"""
function kernel_width(width, bandwidth)
  pg_gauss_kernel_width(width, bandwidth)
end

"""
    get_error()

Return the last error that occurred in the optimizer.
"""
function get_error()
  ptr = pg_get_error()
  if ptr == C_NULL
    nothing
  else
    unsafe_string(ptr)
  end
end

"""
    get_kernels()

Retrieve the kernel data from the GPU. The returned array contains for each kernel:
* X coordinate
* Y coordinate
* Frequency
* Phase
* Orientation
* Internal state value
"""
function get_kernels()
  grid_x = Ref{Int32}(0)
  grid_y = Ref{Int32}(0)
  kernel_count = Ref{Int32}(0)

  unsafe_ptr = pg_get_kernels(grid_x, grid_y, kernel_count)

  if unsafe_ptr == C_NULL
    error(get_error())
  end

  println("grid_x: " * string(grid_x) * " grid_y: " * string(grid_y) * " cnt: " * string(kernel_count))
  Base.unsafe_wrap(Array, unsafe_ptr, (kernel_count[], grid_x[], grid_y[]))
end

"""
    set_kernels(kernels)

Set the kernel data to the given array. You can use `get_kernels()` and change values
inside the returned array to generate suitable data for this. You can then call
`framex(d; init_kernels = false)` to render the resulting noise.
"""
function set_kernels(kernels::Array{Kernel,3})
  (kernel_count, grid_y, grid_x) = size(kernels)

  result = pg_set_kernels(kernels, grid_x, grid_y, kernel_count)

  if !result
    error(get_error())
  end

  nothing
end

export init, terminate, optimize, framex, kernel_width, get_kernels
export DM_NOISE, DM_COMPLEX, DM_STATE, AM_STATIC, AM_GAUSS, AM_RANGLE, AM_RADIAL, FM_STATIC, FM_GAUSS, IM_ANISOTROPIC, IM_GAUSS, IM_ISOTROPIC, IM_RAMP, CM_CLAMP, CM_MOD, OM_OPTIMIZE, OM_AVERAGE, OM_HYBRID, OM_COND_AVERAGE

# For compatibility with former lib
const PhasorOptGen = PhasorOpt
export PhasorOptGen

end # module
