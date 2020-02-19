use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use heck::CamelCase;
use heck::SnakeCase;

use crate::{shader_kind::ShaderKindInfo, Error, Result};

#[derive(Debug)]
pub enum TargetType {
    Automatic,
    SpirV,
    Glsl(spirv_cross::glsl::Version),
}

impl Default for TargetType {
    fn default() -> Self {
        Self::Automatic
    }
}

#[derive(Default)]
pub struct CompilerBuilder {
    skip_cargo: bool,
    dest: Option<PathBuf>,
    skip_spirv: bool,
    output_type: TargetType,
}

impl CompilerBuilder {
    pub fn skip_cargo(self, skip_cargo: bool) -> Self {
        Self { skip_cargo, ..self }
    }

    pub fn dest(self, dest: impl Into<PathBuf>) -> Self {
        Self {
            dest: Some(dest.into()),
            ..self
        }
    }

    pub fn skip_spirv(self, skip_spirv: bool) -> Self {
        Self { skip_spirv, ..self }
    }

    pub fn output_type(self, output_type: TargetType) -> Self {
        Self {
            output_type,
            ..self
        }
    }

    pub fn build(mut self) -> Result<Compiler> {
        // Are we building for WASM?
        let is_wasm = std::env::var("TARGET")
            .map(|v| v.starts_with("wasm32"))
            .unwrap_or(false);

        // Default path to OUT_DIR
        if self.dest.is_none() {
            self.dest = std::env::var("OUT_DIR").map(PathBuf::from).ok();
        }

        // If building for WASM, force source usage unless a specific version is required
        let output_type = match self.output_type {
            TargetType::Automatic => {
                if is_wasm {
                    TargetType::Glsl(spirv_cross::glsl::Version::V3_00Es)
                } else {
                    if self.skip_spirv {
                        TargetType::Glsl(spirv_cross::glsl::Version::V4_60)
                    } else {
                        TargetType::SpirV
                    }
                }
            }
            TargetType::SpirV => {
                if is_wasm {
                    return Err(Error::InvalidTargetType(self.output_type));
                } else {
                    if self.skip_spirv {
                        return Err(Error::InvalidSkipSpirV);
                    } else {
                        TargetType::SpirV
                    }
                }
            }
            TargetType::Glsl(version) => {
                if is_wasm {
                    match version {
                        spirv_cross::glsl::Version::V3_00Es
                        | spirv_cross::glsl::Version::V1_00Es => TargetType::Glsl(version),
                        _ => {
                            return Err(Error::InvalidTargetType(self.output_type));
                        }
                    }
                } else {
                    TargetType::Glsl(version)
                }
            }
        };

        Ok(Compiler {
            compiler: shaderc::Compiler::new().unwrap(),
            skip_cargo: self.skip_cargo,
            wrapped_shaders: HashMap::new(),
            wrapped_programs: HashMap::new(),
            dest: self.dest.expect(
                "dest was not specified for the compiler and the OUT_DIR variable was not defined",
            ),
            skip_spirv: self.skip_spirv,
            output_type,
        })
    }
}

struct WrappedShader {
    rs_file_name: String,
    uniforms: Vec<crate::reflect::FoundUniform>,

    shader_struct_name: String,
    shader_variable_name: String,
    uniform_struct_name: String,
    uniform_locations_name: String,
}

impl WrappedShader {
    pub fn new(base_name: &str) -> Self {
        let shader_struct_name = (base_name.to_owned() + "_shader").to_camel_case();
        let shader_variable_name = shader_struct_name.to_snake_case();

        Self {
            rs_file_name: base_name.to_owned() + ".rs",
            uniforms: Vec::new(),
            shader_struct_name,
            shader_variable_name,
            uniform_struct_name: (base_name.to_owned() + "_uniforms").to_camel_case(),
            uniform_locations_name: (base_name.to_owned() + "_locations").to_snake_case(),
        }
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
}

struct WrappedProgram {
    rs_file_name: String,
}

pub struct Compiler {
    compiler: shaderc::Compiler,
    wrapped_shaders: HashMap<PathBuf, WrappedShader>,
    wrapped_programs: HashMap<String, WrappedProgram>,
    skip_cargo: bool,
    dest: PathBuf,
    skip_spirv: bool,
    output_type: TargetType,
}

impl Compiler {
    fn write_shader(
        &self,
        shader: &str,
        binary_result: &shaderc::CompilationArtifact,
    ) -> Result<String> {
        let shader_file_name = format!(
            "{}{}",
            shader,
            if let TargetType::SpirV = self.output_type {
                ".spv"
            } else {
                ""
            }
        );

        // Write binary to .spv/.glsl file
        let mut output = File::create(&self.dest.join(&shader_file_name))?;

        match self.output_type {
            TargetType::SpirV => {
                // Just write spv file
                output.write_all(binary_result.as_binary_u8())?;
            }
            TargetType::Glsl(version) => {
                if self.skip_spirv {
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

    fn write_rust_wrapper(
        &self,
        wrapped_shader: &WrappedShader,
        shader: &str,
        kind: ShaderKindInfo,
        shader_file_name: &str,
    ) -> Result<()> {
        // Write Rust interface code
        let output_rs = File::create(&self.dest.join(&wrapped_shader.rs_file_name)).unwrap();
        let mut wr = BufWriter::new(output_rs);

        // Shader resource structure
        writeln!(wr, "/// {} Rust wrapper", shader)?;
        writeln!(wr, "pub struct {} {{", wrapped_shader.shader_struct_name())?;
        writeln!(
            wr,
            "    name: <::tinygl::glow::Context as ::tinygl::HasContext>::Shader,"
        )?;
        writeln!(wr, "}}")?;

        writeln!(wr, "impl {} {{", wrapped_shader.shader_struct_name())?;
        writeln!(wr, "    pub fn build(gl: &::tinygl::Context) -> Result<Self, String> {{")?;
        writeln!(wr, "        Ok(Self {{ name: <Self as {st}>::build(gl)? }})",
            st = if let TargetType::Glsl(_) = self.output_type {
                "::tinygl::SourceShader"
            } else {
                "::tinygl::BinaryShader"
            })?;
        writeln!(wr, "    }}")?;
        writeln!(wr, "}}")?;

        // Write struct for holding uniform locations
        writeln!(wr, "#[derive(Default)]")?;
        writeln!(wr, "pub struct {} {{", wrapped_shader.uniform_struct_name())?;

        for uniform in &wrapped_shader.uniforms {
            writeln!(wr, "    {name}: Option<<::tinygl::glow::Context as ::tinygl::glow::HasContext>::UniformLocation>,",
                name = uniform.location_name())?;
        }
        writeln!(wr, "}}")?;

        writeln!(wr, "impl {} {{", wrapped_shader.uniform_struct_name())?;
        // Write constructor
        writeln!(
            wr,
            "    pub fn new({prefix}gl: &::tinygl::Context, {prefix}program: <::tinygl::glow::Context as ::tinygl::glow::HasContext>::Program) -> Self {{",
            prefix = if let TargetType::Glsl(_) = self.output_type {
                if wrapped_shader.uniforms.is_empty() {
                    "_"
                } else {
                    ""
                }
            } else {
                "_"
            })?;
        if let TargetType::Glsl(_) = self.output_type {
            if !wrapped_shader.uniforms.is_empty() {
                writeln!(wr, "        use ::tinygl::HasContext;")?;
            }
        }
        writeln!(wr, "        Self {{")?;

        for uniform in &wrapped_shader.uniforms {
            if let TargetType::Glsl(_) = self.output_type {
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

        // Write setter methods
        for uniform in &wrapped_shader.uniforms {
            let ty = uniform.ty.unwrap();

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
            "impl ::tinygl::ShaderCommon for {} {{",
            wrapped_shader.shader_struct_name()
        )?;
        writeln!(wr, "    fn kind() -> u32 {{")?;
        writeln!(wr, "        ::tinygl::gl::{}", kind.constant_name)?;
        writeln!(wr, "    }}")?;
        writeln!(
            wr,
            "    fn name(&self) -> <::tinygl::glow::Context as ::tinygl::HasContext>::Shader {{"
        )?;
        writeln!(wr, "        self.name")?;
        writeln!(wr, "    }}")?;
        writeln!(wr, "}}")?;

        // Implement GlDrop
        writeln!(wr, "impl ::tinygl::GlDrop for {} {{", wrapped_shader.shader_struct_name())?;
        writeln!(wr, "    fn drop(&mut self, gl: &::tinygl::Context) {{")?;
        writeln!(wr, "        use ::tinygl::HasContext;")?;
        writeln!(wr, "        use ::tinygl::ShaderCommon;")?;
        writeln!(wr, "        unsafe {{ gl.delete_shader(self.name()) }};")?;
        writeln!(wr, "    }}")?;
        writeln!(wr, "}}")?;

        // Implement the right shader trait for the given output type
        if let TargetType::Glsl(_) = self.output_type {
            writeln!(
                wr,
                "impl ::tinygl::SourceShader<'static> for {} {{",
                wrapped_shader.shader_struct_name()
            )?;
            writeln!(wr, "    fn get_source() -> &'static str {{")?;
            writeln!(wr, "        include_str!(\"{}\")", shader_file_name)?;
            writeln!(wr, "    }}")?;
            writeln!(wr, "}}")?;
        } else {
            writeln!(
                wr,
                "impl ::tinygl::BinaryShader<'static> for {} {{",
                wrapped_shader.shader_struct_name()
            )?;
            writeln!(wr, "    fn get_binary() -> &'static [u8] {{")?;
            writeln!(wr, "        include_bytes!(\"{}\")", shader_file_name)?;
            writeln!(wr, "    }}")?;
            writeln!(wr, "}}")?;
        }

        Ok(())
    }

    pub fn wrap_shader(&mut self, source_path: impl AsRef<Path>) -> Result<()> {
        // Get full path to shader
        let source_path = std::fs::canonicalize(source_path)?;

        // Shader name
        let shader = source_path
            .file_name()
            .expect("source shader is not a file")
            .to_string_lossy()
            .to_owned();

        if !self.skip_cargo {
            // Notify cargo to rerun if the source changes
            println!("cargo:rerun-if-changed={}", source_path.display());
        }

        // Read GLSL source
        let source = std::fs::read_to_string(&source_path).unwrap();

        // Match shader type
        let kind = ShaderKindInfo::from_path(&source_path)
            .expect("no file extension on path, cannot determine shader type");

        let wrapped_shader_entry = {
            // Set callback
            let mut options = shaderc::CompileOptions::new().unwrap();

            // Default to OpenGL targets
            options.set_target_env(shaderc::TargetEnv::OpenGL, 0);

            // Set include callback
            let skip_cargo = self.skip_cargo;
            options.set_include_callback(move |name, _include_type, source, _depth| {
                // TODO: Circular includes?
                // TODO: Better include resolver?
                match std::fs::canonicalize(Path::new(&source).parent().unwrap().join(name)) {
                    Ok(full_path) => {
                        if !skip_cargo {
                            // Notify cargo to rerun if included file changed
                            println!("cargo:rerun-if-changed={}", full_path.display());
                        }

                        match std::fs::read_to_string(&full_path) {
                            Ok(content) => Ok(shaderc::ResolvedInclude {
                                resolved_name: full_path.to_string_lossy().to_string(),
                                content,
                            }),
                            Err(error) => Err(error.to_string()),
                        }
                    }
                    Err(error) => Err(error.to_string()),
                }
            });

            let compiler_result = if self.skip_spirv {
                // Only assemble source if we're skipping SPIR-V
                self.compiler.preprocess(
                    &source,
                    &source_path.to_string_lossy(),
                    "main",
                    Some(&options),
                )
            } else {
                // Compile into SPIR-V
                self.compiler.compile_into_spirv(
                    &source,
                    kind.shaderc_kind,
                    &source_path.to_string_lossy(),
                    "main",
                    Some(&options),
                )
            };

            match compiler_result {
                Ok(binary_result) => {
                    // TODO: Show compilation warnings from binary_result

                    // Base name to identify this shader
                    let base_name = shader.replace(".", "_");
                    let mut wrapped_shader = WrappedShader::new(&base_name);

                    // Write the shader binary before the rest of the parsing, for debugging
                    let shader_file_name = self.write_shader(&shader, &binary_result)?;

                    // Extract uniforms from SPIR-V representation
                    if !self.skip_spirv {
                        // Extract uniform data
                        let mut loader = rspirv::mr::Loader::new();
                        rspirv::binary::parse_words(binary_result.as_binary(), &mut loader)
                            .unwrap();
                        let module = loader.module();

                        wrapped_shader.uniforms =
                            crate::reflect::find_uniforms(&source_path.to_string_lossy(), &module)?;
                    }

                    self.write_rust_wrapper(&wrapped_shader, &shader, kind, &shader_file_name)?;

                    Ok(wrapped_shader)
                }
                Err(shaderc::Error::CompilationError(num_errors, errors)) => {
                    if !self.skip_cargo {
                        eprintln!("{}", errors);
                    }

                    Err(Error::CompilationError(num_errors as usize, errors))
                }
                Err(error) => panic!(error.to_string()),
            }
        };

        match wrapped_shader_entry {
            Ok(wrapped_shader) => {
                // Add to list of files to include
                self.wrapped_shaders.insert(source_path, wrapped_shader);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    pub fn wrap_program(&mut self, attached_shaders: &[&str], program_name: &str) -> Result<()> {
        // Find wrapped shader details
        let shaders: std::result::Result<Vec<_>, _> = attached_shaders
            .iter()
            .map(|name| {
                std::fs::canonicalize(name)
                    .map_err(|err| err.into())
                    .and_then(|path| {
                        self.wrapped_shaders
                            .get(&path)
                            .ok_or_else(|| Error::UnwrappedShader((*name).to_owned()))
                    })
            })
            .collect();

        // Unwrap to propagate errors
        let shaders = shaders?;
        let shaders_with_uniforms: Vec<_> =
            shaders.iter().filter(|s| !s.uniforms.is_empty()).collect();

        // Write Rust program code
        let rs_file_name = program_name.to_snake_case() + "_program.rs";
        let output_rs = File::create(&self.dest.join(&rs_file_name))?;
        let mut wr = BufWriter::new(output_rs);

        let program_struct_name = program_name.to_camel_case() + "Program";

        writeln!(wr, "pub struct {} {{", program_struct_name)?;
        // Program name handle
        writeln!(
            wr,
            "    name: <::tinygl::glow::Context as ::tinygl::HasContext>::Program,"
        )?;
        // Write uniform handles
        for shader in &shaders_with_uniforms {
            writeln!(
                wr,
                "    {}: {},",
                shader.uniform_locations_name(),
                shader.uniform_struct_name()
            )?;
        }
        writeln!(wr, "}}")?;

        writeln!(wr, "impl {} {{", program_struct_name)?;
        // Constructor function
        writeln!(wr, "    pub fn new(gl: &::tinygl::Context,")?;
        // Add shader parameters
        for shader in &shaders {
            writeln!(
                wr,
                "               {param_name}: &{param_type},",
                param_name = shader.shader_variable_name(),
                param_type = shader.shader_struct_name()
            )?;
        }
        writeln!(wr, "              ) -> Result<Self, String> {{")?;
        writeln!(wr, "        use ::tinygl::ShaderCommon;")?;
        writeln!(wr, "        use ::tinygl::HasContext;")?;
        writeln!(wr, "        unsafe {{")?;
        writeln!(wr, "            let program_name = gl.create_program()?;")?;
        for shader in &shaders {
            writeln!(
                wr,
                "            gl.attach_shader(program_name, {}.name());",
                shader.shader_variable_name()
            )?;
        }
        writeln!(wr, "            gl.link_program(program_name);")?;
        for shader in &shaders {
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
        for shader in &shaders_with_uniforms {
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
        writeln!(wr, "    pub fn build(gl: &::tinygl::Context) -> Result<Self, String> {{")?;
        for shader in &shaders {
            writeln!(wr, "        let {} = ::tinygl::GlHandle::new(gl, {}::build(gl)?);",
                shader.shader_variable_name(),
                shader.shader_struct_name())?;
        }
        writeln!(wr, "        Ok(Self::new(")?;
        writeln!(wr, "            gl,")?;
        for shader in &shaders {
            writeln!(
                wr,
                "            {name}.as_ref(),", name = shader.shader_variable_name(),
            )?;
        }
        writeln!(wr, "        )?)")?;
        writeln!(wr, "    }}")?;
        // Uniform setters for the included shaders
        for shader in &shaders_with_uniforms {
            for uniform in &shader.uniforms {
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
        writeln!(wr, "impl ::tinygl::ProgramCommon for {} {{", program_struct_name)?;
        // Name getter
        writeln!(wr, "    fn name(&self) -> <::tinygl::glow::Context as ::tinygl::HasContext>::Program {{")?;
        writeln!(wr, "        self.name")?;
        writeln!(wr, "    }}")?;
        writeln!(wr, "}}")?;

        // Implement GlDrop
        writeln!(wr, "impl ::tinygl::GlDrop for {} {{", program_struct_name)?;
        writeln!(wr, "    fn drop(&mut self, gl: &::tinygl::Context) {{")?;
        writeln!(wr, "        use ::tinygl::HasContext;")?;
        writeln!(wr, "        use ::tinygl::ProgramCommon;")?;
        writeln!(wr, "        unsafe {{ gl.delete_program(self.name()) }};")?;
        writeln!(wr, "    }}")?;
        writeln!(wr, "}}")?;


        // Add to list of wrapped programs
        self.wrapped_programs
            .insert(program_name.to_owned(), WrappedProgram { rs_file_name });

        Ok(())
    }

    pub fn write_root_include(&self) -> Result<()> {
        // Write master shaders.rs file
        let output_rs = File::create(&self.dest.join("shaders.rs"))?;
        let mut wr = BufWriter::new(output_rs);

        // Include shaders
        for (source_path, shader) in &self.wrapped_shaders {
            writeln!(wr, "// {}", source_path.to_string_lossy())?;
            writeln!(wr, "include!(\"{}\");", shader.rs_file_name)?;
        }

        // Include programs
        for (program_name, program) in &self.wrapped_programs {
            writeln!(wr, "// {}", program_name)?;
            writeln!(wr, "include!(\"{}\");", program.rs_file_name)?;
        }

        Ok(())
    }
}
