static mut LOG_INITIALIZED: bool = false;

pub fn init() {
    unsafe {
        if !LOG_INITIALIZED {
            LOG_INITIALIZED = true;

            env_logger::init_from_env(
                env_logger::Env::new()
                    .filter_or("PHASOR_LOG", "opengl=debug,phasor=debug,tinygl=debug")
                    .write_style("PHASOR_LOG_STYLE"),
            );
        }
    }
}
