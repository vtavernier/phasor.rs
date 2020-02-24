use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use heck::CamelCase;
use heck::SnakeCase;

use super::wrapped_shader::WrappedShader;

pub struct WrappedProgram {
    id: String,
    struct_name: String,
    rs_file_name: String,
    attached_shaders: Vec<String>,
}

pub struct WrappedProgramUniforms<'a> {
    pub shaders: Vec<&'a WrappedShader>,
    pub shaders_with_uniforms: Vec<&'a WrappedShader>,
}

impl WrappedProgram {
    pub fn new(program_name: &str, attached_shaders: &[&str]) -> Self {
        let id = program_name.to_snake_case();
        let struct_name = program_name.to_camel_case() + "Program";
        let rs_file_name = struct_name.to_snake_case() + ".rs";

        Self {
            id,
            struct_name,
            rs_file_name,
            attached_shaders: attached_shaders.iter().map(|n| (*n).to_owned()).collect()
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn struct_name(&self) -> &str {
        &self.struct_name
    }

    pub fn resolve_shaders<'a>(
        &self,
        wrapped_shaders: &'a HashMap<PathBuf, WrappedShader>,
    ) -> crate::Result<WrappedProgramUniforms<'a>> {
        // Find wrapped shader details
        let shaders: std::result::Result<Vec<_>, _> = self.attached_shaders
            .iter()
            .map(|name| {
                std::fs::canonicalize(name)
                    .map_err(|err| err.into())
                    .and_then(|path| {
                        wrapped_shaders
                            .get(&path)
                            .ok_or_else(|| crate::Error::UnwrappedShader((*name).to_owned()))
                    })
            })
            .collect();

        // Unwrap to propagate errors
        let shaders = shaders?;
        let shaders_with_uniforms: Vec<_> = shaders
            .iter()
            .filter(|s| !s.uniforms().is_empty())
            .map(|s| *s)
            .collect();

        Ok(WrappedProgramUniforms {
            shaders,
            shaders_with_uniforms,
        })
    }

    pub fn write_rust_wrapper(
        &self,
        dest: impl AsRef<Path>,
        attached_shaders: WrappedProgramUniforms<'_>,
    ) -> crate::Result<()> {
        // Write Rust program code
        let output_rs = File::create(&Path::new(dest.as_ref()).join(&self.rs_file_name))?;
        let mut wr = BufWriter::new(output_rs);

        writeln!(wr, "pub struct {} {{", self.struct_name)?;
        // Program name handle
        writeln!(
            wr,
            "    name: <::tinygl::glow::Context as ::tinygl::HasContext>::Program,"
        )?;
        // Write uniform handles
        for shader in &attached_shaders.shaders_with_uniforms {
            writeln!(
                wr,
                "    {}: {},",
                shader.uniform_locations_name(),
                shader.uniform_struct_name()
            )?;
        }
        writeln!(wr, "}}")?;

        writeln!(wr, "impl {} {{", self.struct_name)?;
        // Constructor function
        writeln!(wr, "    pub fn new(gl: &::tinygl::Context,")?;
        // Add shader parameters
        for shader in &attached_shaders.shaders {
            writeln!(
                wr,
                "               {param_name}: &{param_type},",
                param_name = shader.shader_variable_name(),
                param_type = shader.shader_struct_name()
            )?;
        }
        writeln!(wr, "              ) -> Result<Self, String> {{")?;
        writeln!(wr, "        use ::tinygl::wrappers::ShaderCommon;")?;
        writeln!(wr, "        use ::tinygl::HasContext;")?;
        writeln!(wr, "        unsafe {{")?;
        writeln!(wr, "            let program_name = gl.create_program()?;")?;
        for shader in &attached_shaders.shaders {
            writeln!(
                wr,
                "            gl.attach_shader(program_name, {}.name());",
                shader.shader_variable_name()
            )?;
        }
        writeln!(wr, "            gl.link_program(program_name);")?;
        for shader in &attached_shaders.shaders {
            writeln!(
                wr,
                "            gl.detach_shader(program_name, {}.name());",
                shader.shader_variable_name()
            )?;
        }
        writeln!(
            wr,
            "            if !gl.get_program_link_status(program_name) {{"
        )?;
        writeln!(
            wr,
            "                let error = gl.get_program_info_log(program_name);"
        )?;
        writeln!(wr, "                gl.delete_program(program_name);")?;
        writeln!(wr, "                return Err(error);")?;
        writeln!(wr, "            }}")?;
        writeln!(wr, "            Ok(Self {{")?;
        writeln!(wr, "                name: program_name,")?;
        for shader in &attached_shaders.shaders_with_uniforms {
            writeln!(
                wr,
                "                {}: {}::new(gl, program_name),",
                shader.uniform_locations_name(),
                shader.uniform_struct_name()
            )?;
        }
        writeln!(wr, "            }})")?;
        writeln!(wr, "        }}")?;
        writeln!(wr, "    }}")?;
        // Write builder (constructs shaders and then calls the constructor)
        writeln!(
            wr,
            "    pub fn build(gl: &::tinygl::Context) -> Result<Self, String> {{"
        )?;
        for shader in &attached_shaders.shaders {
            writeln!(
                wr,
                "        let {} = ::tinygl::wrappers::GlRefHandle::new(gl, {}::build(gl)?);",
                shader.shader_variable_name(),
                shader.shader_struct_name()
            )?;
        }
        writeln!(wr, "        Ok(Self::new(")?;
        writeln!(wr, "            gl,")?;
        for shader in &attached_shaders.shaders {
            writeln!(
                wr,
                "            {name}.as_ref(),",
                name = shader.shader_variable_name(),
            )?;
        }
        writeln!(wr, "        )?)")?;
        writeln!(wr, "    }}")?;
        // Uniform setters for the included shaders
        for shader in &attached_shaders.shaders_with_uniforms {
            for uniform in shader.uniforms() {
                let ty = uniform.ty.unwrap();

                writeln!(
                    wr,
                    "    pub fn set_{uniform_sc_name}(&self, gl: &::tinygl::Context, value: {type_name}) {{",
                    uniform_sc_name = uniform.name.to_snake_case(),
                    type_name = ty.cgmath_name()
                )?;

                writeln!(
                    wr,
                    "        self.{location_name}.set_{uniform_sc_name}(gl, value);",
                    location_name = shader.uniform_locations_name(),
                    uniform_sc_name = uniform.name.to_snake_case(),
                )?;

                writeln!(wr, "    }}")?;
            }
        }
        writeln!(wr, "}}")?;

        // Implement ProgramCommon
        writeln!(
            wr,
            "impl ::tinygl::wrappers::ProgramCommon for {} {{",
            self.struct_name
        )?;
        // Name getter
        writeln!(
            wr,
            "    fn name(&self) -> <::tinygl::glow::Context as ::tinygl::HasContext>::Program {{"
        )?;
        writeln!(wr, "        self.name")?;
        writeln!(wr, "    }}")?;
        writeln!(wr, "}}")?;

        // Implement GlDrop
        writeln!(
            wr,
            "impl ::tinygl::wrappers::GlDrop for {} {{",
            self.struct_name
        )?;
        writeln!(wr, "    fn drop(&mut self, gl: &::tinygl::Context) {{")?;
        writeln!(wr, "        use ::tinygl::HasContext;")?;
        writeln!(wr, "        use ::tinygl::wrappers::ProgramCommon;")?;
        writeln!(wr, "        unsafe {{ gl.delete_program(self.name()) }};")?;
        writeln!(wr, "    }}")?;
        writeln!(wr, "}}")?;

        Ok(())
    }

    pub fn write_root_include(&self, mut wr: impl Write) -> std::io::Result<()> {
        writeln!(wr, "// {}", self.id)?;
        writeln!(wr, "include!(\"{}\");", self.rs_file_name)?;
        Ok(())
    }
}
