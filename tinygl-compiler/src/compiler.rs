use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use crate::{shader_kind::ShaderKindInfo, Error, Result};

mod target_type;
pub use target_type::TargetType;

mod uniform_set;
use uniform_set::*;

mod wrapped_shader;
use wrapped_shader::*;

mod wrapped_program;
use wrapped_program::*;

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
            wrapped_uniform_sets: HashMap::new(),
            dest: self.dest.expect(
                "dest was not specified for the compiler and the OUT_DIR variable was not defined",
            ),
            skip_spirv: self.skip_spirv,
            output_type,
        })
    }
}

pub struct Compiler {
    compiler: shaderc::Compiler,
    wrapped_shaders: HashMap<PathBuf, WrappedShader>,
    wrapped_programs: HashMap<String, WrappedProgram>,
    wrapped_uniform_sets: HashMap<String, WrappedUniformSet>,
    skip_cargo: bool,
    dest: PathBuf,
    skip_spirv: bool,
    output_type: TargetType,
}

impl Compiler {
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
                    let mut wrapped_shader = WrappedShader::new(&shader, kind, &source_path);

                    // Write the shader binary before the rest of the parsing, for debugging
                    let shader_file_name = wrapped_shader.write_shader(
                        &self.dest,
                        &binary_result,
                        self.output_type,
                        self.skip_spirv,
                    )?;

                    // Extract uniforms from SPIR-V representation
                    if !self.skip_spirv {
                        wrapped_shader.reflect_uniforms(binary_result.as_binary())?;
                    }

                    wrapped_shader.write_rust_wrapper(
                        &self.dest,
                        self.output_type,
                        &shader_file_name,
                    )?;

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
        let wrapped_program = WrappedProgram::new(&program_name, attached_shaders);

        // Resolve uniforms
        let uniform_data = wrapped_program.resolve_shaders(&self.wrapped_shaders)?;

        // Write Rust wrapper for program
        wrapped_program.write_rust_wrapper(&self.dest, uniform_data)?;

        // Add to list of wrapped programs
        self.wrapped_programs.insert(wrapped_program.id().to_owned(), wrapped_program);

        Ok(())
    }

    pub fn wrap_uniforms(&mut self, programs: &[&str], set_name: &str) -> Result<()> {
        let uniform_set = WrappedUniformSet::new(&set_name);

        // Resolve programs
        let uniform_data = uniform_set.resolve_programs(programs, &self.wrapped_programs, &self.wrapped_shaders)?;

        // Prepare Rust wrapper
        uniform_set.write_rust_wrapper(&self.dest, uniform_data)?;

        // Add to list of wrapped sets
        self.wrapped_uniform_sets.insert(uniform_set.id().to_owned(), uniform_set);

        Ok(())
    }

    pub fn write_root_include(&self) -> Result<()> {
        // Write master shaders.rs file
        let output_rs = File::create(&self.dest.join("shaders.rs"))?;
        let mut wr = BufWriter::new(output_rs);

        // Include shaders
        for (_source_path, shader) in &self.wrapped_shaders {
            shader.write_root_include(&mut wr)?;
        }

        // Include programs
        for (_program_name, program) in &self.wrapped_programs {
            program.write_root_include(&mut wr)?;
        }

        // Write program wrappers
        for (_uniform_set_name, uniform_set) in &self.wrapped_uniform_sets {
            uniform_set.write_root_include(&mut wr)?;
        }

        Ok(())
    }
}
