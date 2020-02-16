use std::error;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use heck::CamelCase;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    CompilationError(usize, String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "i/o error: {}", error),
            Self::CompilationError(_num_errors, errors) => {
                write!(f, "compilation error: {}", errors)
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // So we don't have to box everything
        None
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Default)]
pub struct CompilerBuilder {
    skip_cargo: bool,
}

impl CompilerBuilder {
    pub fn skip_cargo(self, skip_cargo: bool) -> Self {
        Self { skip_cargo, ..self }
    }

    pub fn build(self) -> Compiler {
        Compiler {
            compiler: shaderc::Compiler::new().unwrap(),
            skip_cargo: self.skip_cargo,
            shader_names: Vec::new(),
        }
    }
}

pub struct Compiler {
    compiler: shaderc::Compiler,
    shader_names: Vec<String>,
    skip_cargo: bool,
}

struct ShaderKindInfo {
    shaderc_kind: shaderc::ShaderKind,
    constant_name: &'static str,
}

impl ShaderKindInfo {
    pub fn from_path(p: impl AsRef<Path>) -> Option<Self> {
        if let Some(ext) = p.as_ref().extension() {
            return Some(match ext.to_str() {
                Some("vert") => Self {
                    shaderc_kind: shaderc::ShaderKind::Vertex,
                    constant_name: "VERTEX_SHADER",
                },
                Some("comp") => Self {
                    shaderc_kind: shaderc::ShaderKind::Compute,
                    constant_name: "COMPUTE_SHADER",
                },
                Some("frag") => Self {
                    shaderc_kind: shaderc::ShaderKind::Fragment,
                    constant_name: "FRAGMENT_SHADER",
                },

                // TODO: Add other shader types
                _ => panic!("{}: unknown shader type", p.as_ref().to_string_lossy()),
            });
        }

        None
    }
}

impl Compiler {
    pub fn wrap_shader(
        &mut self,
        dest: impl AsRef<Path>,
        source_path: impl AsRef<Path>,
        force_source: bool,
    ) -> Result<()> {
        // Path to destination
        let dest = PathBuf::from(&dest.as_ref());

        // Get full path to shader
        let source_path = std::fs::canonicalize(source_path)?;

        // Shader name
        let shader = source_path
            .file_name()
            .expect("source shader is not a file")
            .to_string_lossy()
            .to_owned();

        // Are we building for WASM?
        let is_wasm = std::env::var("TARGET")
            .map(|v| v.starts_with("wasm32"))
            .unwrap_or(false);

        // If building for WASM, force source
        let force_source = if is_wasm { true } else { force_source };

        if !self.skip_cargo {
            // Notify cargo to rerun if the source changes
            println!("cargo:rerun-if-changed={}", source_path.display());
        }

        // Read GLSL source
        let source = std::fs::read_to_string(&source_path).unwrap();

        // Add preamble
        // TODO: Configurable preamble
        let source = (if is_wasm {
            include_str!("preamble-webgl.glsl")
        } else {
            include_str!("preamble-desktop.glsl")
        })
        .to_owned()
            + &source;

        // Match shader type
        let kind = ShaderKindInfo::from_path(&source_path)
            .expect("no file extension on path, cannot determine shader type");

        let (res, rs_file_name) = {
            // Set callback
            let mut options = shaderc::CompileOptions::new().unwrap();

            // Force the compiler to work when running for WASM
            if is_wasm {
                options.set_forced_version_profile(450, shaderc::GlslProfile::Core);
            }

            // Default to OpenGL targets
            options.set_target_env(shaderc::TargetEnv::OpenGL, 0);

            // Set include callback
            let skip_cargo = self.skip_cargo;
            options.set_include_callback(move |name, _include_type, source, _depth| {
                // TODO: Circular includes?
                // TODO: Better include resolver?
                match std::fs::canonicalize(Path::new(&source).parent().unwrap().join(name)) {
                    Ok(full_path) => {
                        if skip_cargo {
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

            // Compile into SPIR-V
            let compiler_result = if force_source {
                self.compiler.preprocess(
                    &source,
                    &source_path.to_string_lossy(),
                    "main",
                    Some(&options),
                )
            } else {
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
                    // Base name to identify this shader
                    let base_name = shader.replace(".", "_");

                    // TODO: Show compilation warnings from binary_result
                    let shader_file_name =
                        format!("{}{}", shader, if force_source { "" } else { ".spv" });
                    let rs_file_name = base_name.clone() + ".rs";

                    // Write Rust interface code
                    let output_rs = File::create(&dest.join(&rs_file_name)).unwrap();
                    let mut wr = BufWriter::new(output_rs);

                    let struct_name = (base_name + "_shader").to_camel_case();

                    writeln!(wr, "/// {} Rust wrapper", shader)?;
                    writeln!(wr, "pub struct {} {{}}", struct_name)?;
                    writeln!(wr, "impl ::tinygl::ShaderCommon for {} {{", struct_name)?;
                    writeln!(wr, "    fn get_kind() -> u32 {{")?;
                    writeln!(wr, "        ::tinygl::gl::{}", kind.constant_name)?;
                    writeln!(wr, "    }}")?;
                    writeln!(wr, "}}")?;
                    if force_source {
                        writeln!(
                            wr,
                            "impl ::tinygl::SourceShader<'static> for {} {{",
                            struct_name
                        )?;
                        writeln!(wr, "    fn get_source() -> &'static str {{")?;
                        writeln!(wr, "        include_str!(\"{}\")", shader_file_name)?;
                        writeln!(wr, "    }}")?;
                        writeln!(wr, "}}")?;
                    } else {
                        writeln!(
                            wr,
                            "impl ::tinygl::BinaryShader<'static> for {} {{",
                            struct_name
                        )?;
                        writeln!(wr, "    fn get_binary() -> &'static [u8] {{")?;
                        writeln!(wr, "        include_bytes!(\"{}\")", shader_file_name)?;
                        writeln!(wr, "    }}")?;
                        writeln!(wr, "}}")?;
                    }
                    writeln!(wr, "")?;

                    // Write binary to .spv/.glsl file
                    let mut output = File::create(&Path::new(&dest).join(shader_file_name))?;
                    if force_source {
                        if is_wasm {
                            // WebGL is more sensitive to leftovers from includes and stuff
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
                            write!(output, "{}", binary_result.as_text())?;
                        }
                    } else {
                        output.write_all(binary_result.as_binary_u8())?;
                    }

                    (Ok(()), Some(rs_file_name))
                }
                Err(shaderc::Error::CompilationError(num_errors, errors)) => {
                    if !self.skip_cargo {
                        eprintln!("{}", errors);
                    }

                    (
                        Err(Error::CompilationError(num_errors as usize, errors)),
                        None,
                    )
                }
                Err(error) => panic!(error.to_string()),
            }
        };

        if let Some(rs_file_name) = rs_file_name {
            // Add to list of files to include
            self.shader_names.push(rs_file_name);
        }

        res
    }

    pub fn write_root_include(&self, dest: impl AsRef<Path>) -> Result<()> {
        // Write master shaders.rs file
        let output_rs = File::create(&dest.as_ref().join("shaders.rs"))?;
        let mut wr = BufWriter::new(output_rs);

        for shader in &self.shader_names {
            writeln!(wr, "include!(\"{}\");", shader)?;
        }

        Ok(())
    }
}
