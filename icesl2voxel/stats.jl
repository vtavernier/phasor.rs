#!/usr/bin/env julia
# vim: ft=julia:ts=2:sw=2:et

# We need our environment
import Pkg
Pkg.activate(@__DIR__)

using Printf, HDF5, Statistics

# Process each input HDF5 file
for input in ARGS
  h5open(input, "r") do h5
    input_percentage = h5["/fields/input_percentage/data"]
    output_stats_mean = h5["/fields/output_stats_mean/data"]

    # Compute input density -> average material density correlation
    density_correlation = cor(input_percentage[:,:,:][:], output_stats_mean[:,:,:][:])

    @printf("%s: density correlation: %g\n", input, density_correlation)
  end
end
