use crate::context::HasContext;

pub trait ProgramCommon {
    fn name(&self) -> <glow::Context as HasContext>::Program;
}
