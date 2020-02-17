use std::path::Path;

pub struct ShaderKindInfo {
    pub shaderc_kind: shaderc::ShaderKind,
    pub constant_name: &'static str,
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
