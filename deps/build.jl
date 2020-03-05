using Clang

const rustlibname = "phasor"
const juliapackage = "PhasorOpt"

const libname = Sys.iswindows() ? rustlibname : "lib" * rustlibname
# Windows .dlls do not have the "lib" prefix

function build_dylib()
    # TODO: --release?
    run(Cmd(`cargo build`, dir=joinpath(@__DIR__, "..")))

    # TODO: release?
    release_dir = joinpath(@__DIR__, "../target/debug")
    dylib = dylib_filename()

    release_dylib_filepath = joinpath(release_dir, dylib)
    @assert isfile(release_dylib_filepath) "$release_dylib_filepath not found. Build may have failed."
    cp(release_dylib_filepath, joinpath(@__DIR__, dylib), force=true)

    write_deps_file("lib" * rustlibname, dylib, juliapackage)
end

function dylib_filename()
    @static if Sys.isapple()
        "$libname.dylib"
    elseif Sys.islinux()
        "$libname.so"
    elseif Sys.iswindows()
        "$libname.dll"
    else
        error("Not supported: $(Sys.KERNEL)")
    end
end

function write_deps_file(libname, libfile, juliapackage)
    script = """
import Libdl

const $libname = joinpath(@__DIR__, "$libfile")

function check_deps()
    global $libname
    if !isfile($libname)
        error("\$$libname does not exist, Please re-run Pkg.build(\\"$juliapackage\\"), and restart Julia.")
    end

    if Libdl.dlopen_e($libname) == C_NULL
        error("\$$libname cannot be opened, Please re-run Pkg.build(\\"$juliapackage\\"), and restart Julia.")
    end
end
"""

    # LIBPHASOR_HEADERS are those headers to be wrapped.
    LIBPHASOR_INCLUDE = joinpath(@__DIR__, "..", "phasor") |> normpath
    LIBPHASOR_HEADERS = [joinpath(LIBPHASOR_INCLUDE, "shaders", "shared.h"), joinpath(LIBPHASOR_INCLUDE, "phasoropt.h")]

    wc = init(; headers = LIBPHASOR_HEADERS,
                output_file = joinpath(@__DIR__, "phasor_api.jl"),
                common_file = joinpath(@__DIR__, "phasor_common.jl"),
                clang_includes = vcat(LIBPHASOR_INCLUDE, CLANG_INCLUDE),
                clang_args = ["-I", LIBPHASOR_INCLUDE],
                header_wrapped = (root, current)->root == current,
                header_library = x->"libphasor",
                clang_diagnostics = true,
                )

    run(wc)

    open(joinpath(@__DIR__, "deps.jl"), "w") do f
        write(f, script)
    end
end

build_dylib()
