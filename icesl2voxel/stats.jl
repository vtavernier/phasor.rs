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
    input_mask = h5["/fields/input_geometry/data"]
    mean_confidence = h5["/fields/output_stats_mean_confidence/data"]

    # Compute input density -> average material density correlation
    mask = (input_mask[:,:,:] .> 250) .& (mean_confidence[:,:,:] .> 250)
    density_correlation = cor(input_percentage[:,:,:][mask], output_stats_mean[:,:,:][mask])

    @printf("%s: density correlation: %g\n", input, density_correlation)
  end
end
