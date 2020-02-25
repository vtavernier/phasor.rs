#[derive(Debug, Clone, Copy)]
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

impl TargetType {
    pub fn is_source(&self) -> bool {
        match self {
            TargetType::Automatic => {
                panic!("TargetType::Automatic cannot be classified as source or not")
            }
            TargetType::Glsl(_) => true,
            TargetType::SpirV => false,
        }
    }
}
