pub fn run<'a, T: super::Demo<'a> + Default>(
    canvas: web_sys::HtmlCanvasElement,
) -> Result<(crate::Context, T), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;

    // Create a context from a WebGL2 context on wasm32 targets
    let gl = {
        let webgl2_context = canvas
            .get_context("webgl2")?
            .unwrap()
            .dyn_into::<web_sys::WebGl2RenderingContext>()?;

        crate::Context::from_webgl2_context(webgl2_context)
    };

    // Initialize demo
    let mut demo = T::default();
    demo.init(&gl);

    // Create state with GL context and in-view demo object
    Ok((gl, demo))
}

#[macro_export]
macro_rules! impl_web_demo {
    ($e:ty) => {
        use tinygl::boilerplate::Demo;
        use wasm_bindgen::{prelude::*, JsCast};

        #[wasm_bindgen]
        pub struct State {
            gl: tinygl::Context,
            demo: $e,
        }

        #[wasm_bindgen]
        pub fn render(mut state: State) {
            state.demo.render(&state.gl);
        }

        #[wasm_bindgen]
        pub fn init(canvas: JsValue) -> Result<State, JsValue> {
            // Cast object into canvas object
            let canvas = canvas
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .expect("canvas element is required");

            tinygl::boilerplate::web::run(canvas).map(|(gl, demo)| State { gl, demo })
        }
    };
}
