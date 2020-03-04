use super::shared;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OptimizationMode {
    None,
    Optimize,
    Average,
    Hybrid,
}

impl OptimizationMode {
    pub fn as_mode(&self) -> i32 {
        match self {
            Self::None => -1,
            Self::Optimize => shared::OM_OPTIMIZE as i32,
            Self::Average => shared::OM_AVERAGE as i32,
            Self::Hybrid => shared::OM_HYBRID as i32,
        }
    }

    pub fn toggle_and_switch(
        &mut self,
        active_mode: &mut OptimizationMode,
        target_mode: OptimizationMode,
    ) {
        *active_mode = target_mode;
        if *self == target_mode {
            *self = Self::None;
        } else {
            *self = target_mode;
        }
    }

    pub fn toggle(&mut self, active_mode: &mut OptimizationMode) {
        match self {
            Self::None => *self = *active_mode,
            Self::Optimize | Self::Average | Self::Hybrid => {
                *active_mode = *self;
                *self = Self::None;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        match self {
            Self::None => false,
            _ => true,
        }
    }
}

impl Default for OptimizationMode {
    fn default() -> Self {
        Self::None
    }
}

impl From<i32> for OptimizationMode {
    fn from(value: i32) -> Self {
        use std::convert::TryFrom;

        match u32::try_from(value) {
            Ok(shared::OM_OPTIMIZE) => Self::Optimize,
            Ok(shared::OM_AVERAGE) => Self::Average,
            Ok(shared::OM_HYBRID) => Self::Hybrid,
            _ => Self::None,
        }
    }
}
