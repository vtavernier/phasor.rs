use std::rc::Rc;

pub fn run<T>(
    canvas: web_sys::HtmlCanvasElement,
) -> Result<(Rc<crate::Context>, T, T::State), wasm_bindgen::JsValue>
where
    T: super::Demo + Default + 'static,
    T::Error: std::fmt::Debug,
{
    use wasm_bindgen::JsCast;

    // Create a context from a WebGL2 context on wasm32 targets
    let gl = {
        let webgl2_context = canvas
            .get_context("webgl2")?
            .unwrap()
            .dyn_into::<web_sys::WebGl2RenderingContext>()?;

        Rc::new(crate::Context::from_webgl2_context(webgl2_context))
    };

    // Initialize demo
    let mut demo = T::default();
    let state = demo.init(&gl).expect("failed to initialize demo");

    // Create state with GL context and in-view demo object
    Ok((gl, demo, state))
}

#[macro_export]
macro_rules! impl_web_demo {
    ($e:ty) => {
        use ::tinygl::boilerplate::Demo;
        use wasm_bindgen::{prelude::*, JsCast};

        #[wasm_bindgen]
        pub struct WebState {
            gl: Rc<::tinygl::Context>,
            demo: $e,
            state: <$e as ::tinygl::boilerplate::Demo>::State,
        }

        #[wasm_bindgen]
        pub fn render(mut state: WebState) {
            state.demo.render(&state.gl, &mut state.state);
        }

        #[wasm_bindgen]
        pub fn init(canvas: JsValue) -> Result<WebState, JsValue> {
            // Cast object into canvas object
            let canvas = canvas
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .expect("canvas element is required");

            ::tinygl::boilerplate::web::run(canvas).map(|(gl, demo, state)| WebState {
                gl,
                demo,
                state,
            })
        }
    };
}
