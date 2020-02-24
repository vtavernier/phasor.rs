use crate::context::HasContext;

pub trait ProgramCommon {
    fn name(&self) -> <glow::Context as HasContext>::Program;

    fn use_program(&self, gl: &crate::Context) {
        unsafe { gl.use_program(Some(self.name())) };
    }
}
