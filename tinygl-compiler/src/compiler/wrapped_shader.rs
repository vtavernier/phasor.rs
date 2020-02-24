use std::fs::File;
use std::io::prelude::*;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use heck::CamelCase;
use heck::SnakeCase;

use rspirv::dr as rr;

use crate::shader_kind::ShaderKindInfo;
use super::TargetType;

pub struct WrappedShader {
    shader: String,
    rs_file_name: String,
    uniforms: Vec<crate::reflect::FoundUniform>,
    kind: ShaderKindInfo,
    source_path: PathBuf,

    shader_struct_name: String,
    shader_variable_name: String,
    uniform_struct_name: String,
    uniform_locations_name: String,
}

impl WrappedShader {
    pub fn new(shader: &str, kind: ShaderKindInfo, source_path: &Path) -> Self {
        let base_name = shader.replace(".", "_");
        let shader_struct_name = (base_name.to_owned() + "_shader").to_camel_case();
        let shader_variable_name = shader_struct_name.to_snake_case();

        Self {
            shader: shader.to_owned(),
            rs_file_name: base_name.to_owned() + ".rs",
            uniforms: Vec::new(),
            kind,
            source_path: source_path.to_owned(),
            shader_struct_name,
            shader_variable_name,
            uniform_struct_name: (base_name.to_owned() + "_uniforms").to_camel_case(),
            uniform_locations_name: (base_name.to_owned() + "_locations").to_snake_case(),
        }
    }

    pub fn uniforms(&self) -> &[crate::reflect::FoundUniform] {
        &self.uniforms[..]
    }

    pub fn shader_struct_name(&self) -> &str {
        &self.shader_struct_name
    }

    pub fn shader_variable_name(&self) -> &str {
        &self.shader_variable_name
    }

    pub fn uniform_struct_name(&self) -> &str {
        &self.uniform_struct_name
    }

    pub fn uniform_locations_name(&self) -> &str {
        &self.uniform_locations_name
    }

    pub fn reflect_uniforms(&mut self, result: &[u32]) -> Result<(), crate::Error> {
        // Extract uniform data
        let mut loader = rr::Loader::new();
        rspirv::binary::parse_words(result, &mut loader).expect(&format!(
            "failed to parse binary module for {}",
            self.source_path.to_string_lossy()
        ));

        self.uniforms =
            crate::reflect::find_uniforms(&self.source_path.to_string_lossy(), &loader.module())?;

        Ok(())
    }

    pub fn write_shader(
        &self,
        dest: impl AsRef<Path>, 
        binary_result: &shaderc::CompilationArtifact,
        output_type: TargetType,
        skip_spirv: bool,
    ) -> crate::Result<String> {
        let shader_file_name = format!(
            "{}{}",
            self.shader,
            if let TargetType::SpirV = output_type {
                ".spv"
            } else {
                ""
            }
        );

        // Write binary to .spv/.glsl file
        let mut output = File::create(&Path::new(dest.as_ref()).join(&shader_file_name))?;

        match output_type {
            TargetType::SpirV => {
                // Just write spv file
                output.write_all(binary_result.as_binary_u8())?;
            }
            TargetType::Glsl(version) => {
                if skip_spirv {
                    // We skipped SPIR-V generation so just fix invalid stuff for OpenGL ES targets
                    // WebGL is more sensitive to leftovers from includes and stuff
                    // TODO: This is an ugly hack, maybe forbid skip_spirv + ES 3.00?
                    for l in binary_result.as_text().lines() {
                        if l.starts_with("#extension GL_GOOGLE_include_directive") {
                            continue;
                        } else if l.starts_with("#line") {
                            writeln!(output, "//{}", l)?;
                        } else {
                            writeln!(output, "{}", l)?;
                        }
                    }
                } else {
                    // Use spirv_cross to write valid code
                    let module = spirv_cross::spirv::Module::from_words(binary_result.as_binary());
                    let mut ast =
                        spirv_cross::spirv::Ast::<spirv_cross::glsl::Target>::parse(&module)?;

                    // Target the right GLSL version
                    ast.set_compiler_options(&spirv_cross::glsl::CompilerOptions {
                        version,
                        ..Default::default()
                    })
                    .unwrap();

                    write!(output, "{}", ast.compile()?)?;
                }
            }
            _ => unreachable!(),
        }

        Ok(shader_file_name)
    }

    pub fn write_rust_wrapper(&self, dest: impl AsRef<Path>, output_type: TargetType, shader_file_name: &str) -> crate::Result<()> {
        // Write Rust interface code
        let output_rs = File::create(&Path::new(dest.as_ref()).join(&self.rs_file_name)).unwrap();
        let mut wr = BufWriter::new(output_rs);

        // Shader resource structure
        writeln!(wr, "/// {} Rust wrapper", self.shader)?;
        writeln!(wr, "pub struct {} {{", self.shader_struct_name())?;
        writeln!(
            wr,
            "    name: <::tinygl::glow::Context as ::tinygl::HasContext>::Shader,"
        )?;
        writeln!(wr, "}}")?;

        writeln!(wr, "impl {} {{", self.shader_struct_name())?;
        writeln!(
            wr,
            "    pub fn build(gl: &::tinygl::Context) -> Result<Self, String> {{"
        )?;
        writeln!(
            wr,
            "        Ok(Self {{ name: <Self as {st}>::build(gl)? }})",
            st = if output_type.is_source() {
                "::tinygl::wrappers::SourceShader"
            } else {
                "::tinygl::wrappers::BinaryShader"
            }
        )?;
        writeln!(wr, "    }}")?;
        writeln!(wr, "}}")?;

        // Write struct for holding uniform locations
        writeln!(wr, "#[derive(Default)]")?;
        writeln!(wr, "pub struct {} {{", self.uniform_struct_name())?;

        for uniform in &self.uniforms {
            writeln!(wr, "    {name}: Option<<::tinygl::glow::Context as ::tinygl::glow::HasContext>::UniformLocation>,",
                name = uniform.location_name())?;
        }
        writeln!(wr, "}}")?;

        writeln!(wr, "impl {} {{", self.uniform_struct_name())?;
        // Write constructor
        writeln!(
            wr,
            "    pub fn new({prefix}gl: &::tinygl::Context, {prefix}program: <::tinygl::glow::Context as ::tinygl::glow::HasContext>::Program) -> Self {{",
            prefix = if output_type.is_source() {
                if self.uniforms.is_empty() {
                    "_"
                } else {
                    ""
                }
            } else {
                "_"
            })?;
        if output_type.is_source() {
            if !self.uniforms.is_empty() {
                writeln!(wr, "        use ::tinygl::HasContext;")?;
            }
        }
        writeln!(wr, "        Self {{")?;

        for uniform in &self.uniforms {
            if output_type.is_source() {
                // Source shader: find uniform locations from variable names
                writeln!(wr, "            {name}: unsafe {{ gl.get_uniform_location(program, \"{uniform_name}\") }},",
                    name = uniform.location_name(),
                    uniform_name = uniform.name)?;
            } else {
                // Binary shader: assume locations form reflection on SPIR-V
                writeln!(
                    wr,
                    "            {name}: Some({location}),",
                    name = uniform.location_name(),
                    location = uniform.location
                )?;
            }
        }

        writeln!(wr, "        }}")?;
        writeln!(wr, "    }}")?;

        // Write getter/setter methods
        for uniform in &self.uniforms {
            let ty = uniform.ty.unwrap();

            if let Some(binding) = uniform.binding {
                writeln!(
                    wr,
                    "    pub fn get_{uniform_sc_name}_binding(&self) -> {type_name} {{",
                    uniform_sc_name = uniform.name.to_snake_case(),
                    type_name = ty.rstype()
                )?;
                writeln!(wr, "        {}", binding)?;
                writeln!(wr, "    }}")?;
            }

            writeln!(
                wr,
                "    pub fn set_{uniform_sc_name}(&self, gl: &::tinygl::Context, value: {type_name}) {{",
                uniform_sc_name = uniform.name.to_snake_case(),
                type_name = ty.cgmath_name()
            )?;

            writeln!(wr, "        use ::tinygl::HasContext;")?;

            writeln!(wr, "        unsafe {{ gl.uniform_{components}_{rstype}_slice(self.{location}.as_ref(), {what}) }};",
                components = ty.components(),
                rstype = ty.rstype(),
                location = uniform.location_name(),
                what = ty.glow_value("value"))?;

            writeln!(wr, "    }}")?;
        }
        writeln!(wr, "}}")?;

        // A wrapped shader implements ShaderCommon
        writeln!(
            wr,
            "impl ::tinygl::wrappers::ShaderCommon for {} {{",
            self.shader_struct_name()
        )?;
        writeln!(wr, "    fn kind() -> u32 {{")?;
        writeln!(wr, "        ::tinygl::gl::{}", self.kind.constant_name)?;
        writeln!(wr, "    }}")?;
        writeln!(
            wr,
            "    fn name(&self) -> <::tinygl::glow::Context as ::tinygl::HasContext>::Shader {{"
        )?;
        writeln!(wr, "        self.name")?;
        writeln!(wr, "    }}")?;
        writeln!(wr, "}}")?;

        // Implement GlDrop
        writeln!(
            wr,
            "impl ::tinygl::wrappers::GlDrop for {} {{",
            self.shader_struct_name()
        )?;
        writeln!(wr, "    fn drop(&mut self, gl: &::tinygl::Context) {{")?;
        writeln!(wr, "        use ::tinygl::prelude::*;")?;
        writeln!(wr, "        unsafe {{ gl.delete_shader(self.name()) }};")?;
        writeln!(wr, "    }}")?;
        writeln!(wr, "}}")?;

        // Implement the right shader trait for the given output type
        if output_type.is_source() {
            writeln!(
                wr,
                "impl ::tinygl::wrappers::SourceShader<'static> for {} {{",
                self.shader_struct_name()
            )?;
            writeln!(wr, "    fn get_source() -> &'static str {{")?;
            writeln!(wr, "        include_str!(\"{}\")", shader_file_name)?;
            writeln!(wr, "    }}")?;
            writeln!(wr, "}}")?;
        } else {
            writeln!(
                wr,
                "impl ::tinygl::wrappers::BinaryShader<'static> for {} {{",
                self.shader_struct_name()
            )?;
            writeln!(wr, "    fn get_binary() -> &'static [u8] {{")?;
            writeln!(wr, "        include_bytes!(\"{}\")", shader_file_name)?;
            writeln!(wr, "    }}")?;
            writeln!(wr, "}}")?;
        }

        Ok(())
    }

    pub fn write_root_include(&self, mut wr: impl Write) -> std::io::Result<()> {
        writeln!(wr, "// {}", self.source_path.to_string_lossy())?;
        writeln!(wr, "include!(\"{}\");", self.rs_file_name)?;
        Ok(())
    }
}
