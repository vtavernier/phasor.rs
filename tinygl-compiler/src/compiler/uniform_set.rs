use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::prelude::*;
use std::io::BufWriter;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};

use heck::CamelCase;
use heck::SnakeCase;

use super::{WrappedProgram, WrappedProgramUniforms, WrappedShader};
use crate::reflect::FoundUniform;

pub struct WrappedUniformSet {
    /// Identifier for this uniform set
    id: String,
    /// Name of the Rust wrapper file for this set
    rs_file_name: String,
    /// Name of the target trait
    trait_name: String,
}

pub struct UniformSetProgram<'a> {
    pub program: &'a WrappedProgram,
    pub uniforms: WrappedProgramUniforms<'a>,
}

pub struct ResolvedUniformSet<'a>(pub Vec<UniformSetProgram<'a>>);

impl WrappedUniformSet {
    pub fn new(id: &str) -> Self {
        let id = id.to_snake_case();
        let trait_name = (id.clone() + "_uniform_set").to_camel_case();
        let rs_file_name = trait_name.to_snake_case() + ".rs";

        Self {
            id,
            rs_file_name,
            trait_name,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn resolve_programs<'a>(
        &self,
        programs: &[&str],
        wrapped_programs: &'a HashMap<String, WrappedProgram>,
        wrapped_shaders: &'a HashMap<PathBuf, WrappedShader>,
    ) -> crate::Result<ResolvedUniformSet<'a>> {
        // Resolve programs
        let wrapped_programs: std::result::Result<Vec<_>, _> = programs
            .iter()
            .map(|name| {
                let name = name.to_snake_case();
                wrapped_programs
                    .get(&name)
                    .ok_or_else(|| crate::Error::UnwrappedProgram((*name).to_owned()))
            })
            .collect();

        let wrapped_programs = wrapped_programs?;

        // Resolve uniforms for each program
        let wrapped_uniforms: std::result::Result<Vec<_>, _> = wrapped_programs
            .iter()
            .map(|program| {
                program
                    .resolve_shaders(&wrapped_shaders)
                    .map(|uniforms| UniformSetProgram { program, uniforms })
            })
            .collect();

        Ok(ResolvedUniformSet(wrapped_uniforms?))
    }

    pub fn write_rust_wrapper(
        &self,
        dest: impl AsRef<Path>,
        programs: ResolvedUniformSet<'_>,
    ) -> crate::Result<()> {
        // Write Rust program code
        let output_rs = File::create(&Path::new(dest.as_ref()).join(&self.rs_file_name))?;
        let mut wr = BufWriter::new(output_rs);

        // Compute uniform set intersection
        let uniform_sets: Vec<HashSet<&FoundUniform>> = programs
            .0
            .iter()
            .map(|program| {
                HashSet::<&FoundUniform>::from_iter(
                    program
                        .uniforms
                        .shaders_with_uniforms
                        .iter()
                        .flat_map(|shader| shader.uniforms()),
                )
            })
            .collect();

        // Clone the first set
        let mut unified = uniform_sets.first().map(Clone::clone).unwrap_or_else(HashSet::new);
        for others in uniform_sets.iter().skip(1) {
            unified = HashSet::from_iter(others.intersection(&unified).map(|x| *x));
        }

        // Turn it into a vec, sort by name
        let mut unified: Vec<_> = unified.into_iter().collect();
        unified.sort_by_key(|f| &f.name);

        // Write trait declaration
        writeln!(wr, "pub trait {} {{", self.trait_name)?;
        // Write methods
        for uniform in &unified {
            let ty = uniform.ty.unwrap();

            writeln!(
                wr,
                "    fn set_{uniform_sc_name}(&self, gl: &::tinygl::Context, value: {type_name});",
                uniform_sc_name = uniform.name.to_snake_case(),
                type_name = ty.cgmath_name()
            )?;
        }
        writeln!(wr, "}}")?;

        // Write implementations for the known programs
        for program in &programs.0 {
            writeln!(
                wr,
                "impl {trait_name} for {program_struct_name} {{",
                trait_name = self.trait_name,
                program_struct_name = program.program.struct_name()
            )?;

            for uniform in &unified {
                let ty = uniform.ty.unwrap();

                writeln!(
                    wr,
                    "    fn set_{uniform_sc_name}(&self, gl: &::tinygl::Context, value: {type_name}) {{",
                    uniform_sc_name = uniform.name.to_snake_case(),
                    type_name = ty.cgmath_name()
                )?;
                writeln!(wr, "        {struct_name}::set_{uniform_sc_name}(self, gl, value)",
                    struct_name = program.program.struct_name(),
                    uniform_sc_name = uniform.name.to_snake_case())?;
                writeln!(wr, "    }}")?;
            }

            writeln!(wr, "}}")?;
        }

        Ok(())
    }

    pub fn write_root_include(&self, mut wr: impl Write) -> std::io::Result<()> {
        writeln!(wr, "// {}", self.id)?;
        writeln!(wr, "include!(\"{}\");", self.rs_file_name)?;
        Ok(())
    }
}
