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
init() = pg_init(true)
terminate() = pg_terminate()
get_max_kernels() = pg_get_max_kernels()

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
                      1,
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
    (result..., copy(reinter_extra[2,:,:]))
  else
    result
  end
end

function kernel_width(width, bandwidth, filter_bandwidth)
  pg_noise_kernel_width(width, bandwidth, filter_bandwidth)
end

function kernel_width(width, bandwidth)
  pg_gauss_kernel_width(width, bandwidth)
end

function get_error()
  ptr = pg_get_error()
  if ptr == C_NULL
    nothing
  else
    unsafe_string(ptr)
  end
end

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

function set_kernels(kernels::Array{Kernel,3})
  (kernel_count, grid_y, grid_x) = size(kernels)

  result = pg_set_kernels(kernels, grid_x, grid_y, kernel_count)

  if !result
    error(get_error())
  end

  nothing
end

export init, terminate, optimize, framex, kernel_width, get_kernels
export DM_NOISE, DM_COMPLEX, DM_STATE, AM_STATIC, AM_GAUSS, AM_RANGLE, AM_RADIAL, FM_STATIC, FM_GAUSS, IM_ANISOTROPIC, IM_GAUSS, IM_ISOTROPIC, IM_RAMP, CM_CLAMP, CM_MOD, OM_OPTIMIZE, OM_AVERAGE

# For compatibility with former lib
const PhasorOptGen = PhasorOpt
export PhasorOptGen

end # module
