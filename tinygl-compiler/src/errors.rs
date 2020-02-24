use super::TargetType;
use std::error;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    CompilationError(usize, String),
    InvalidTargetType(TargetType),
    InvalidSkipSpirV,
    SpirVCrossError(spirv_cross::ErrorCode),
    UnwrappedShader(String),
    UnwrappedProgram(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "i/o error: {}", error),
            Self::CompilationError(_num_errors, errors) => {
                write!(f, "compilation error: {}", errors)
            }
            Self::InvalidTargetType(target_type) => {
                write!(f, "invalid target type for current arch: {:?}", target_type)
            }
            Self::InvalidSkipSpirV => write!(
                f,
                "cannot skip SPIR-V generation when the target is explicitely SPIR-V"
            ),
            Self::SpirVCrossError(error) => write!(f, "spirv_cross error: {:?}", error),
            Self::UnwrappedShader(name) => write!(f, "shader {} was not wrapped before building the program, call Compiler::wrap_shader first", name),
            Self::UnwrappedProgram(name) => write!(f, "program {} was not wrapped before building the uniform set, call Compiler::wrap_program first", name),
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

impl From<spirv_cross::ErrorCode> for Error {
    fn from(error: spirv_cross::ErrorCode) -> Self {
        Self::SpirVCrossError(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
