#!/bin/bash

# Run cargo vendor and generate config
git checkout HEAD -- .cargo/config
cargo vendor >>.cargo/config

# Create git archive
PREFIX=phasor.rs
DEST=phasor-$(git rev-parse --short HEAD).tar
git archive -o $DEST --prefix=$PREFIX/ HEAD . ":(exclude)icesl2voxel"
tar --transform "flags=r;s|^|$PREFIX/|" -rf $DEST vendor .cargo

# Anonymize stuff
FILES="Cargo.toml JuliaProject.toml xtask/Cargo.toml"
git checkout HEAD -- $FILES
sed -i '/authors/d' $FILES
# Mysterious sed bug
if ! [ -r Cargo.toml ]; then
    chmod 0644 $FILES
fi
tar --transform "flags=r;s|^|$PREFIX/|" -uf $DEST $FILES

# Restore files
git checkout HEAD -- $FILES .cargo/config
