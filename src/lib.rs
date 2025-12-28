// cargo: dep = "yew"
// cargo: dep = "serde"
// cargo: dep = "serde_json"
// cargo: dep = "reqwest"
// cargo: dep = "pulldown-cmark"
// cargo: dep = "futures-util"
// cargo: dep = "wasm-bindgen"
// cargo: dep = "wasm-bindgen-futures"
// cargo: dep = "web-sys"
// cargo: dep = "uuid"
// cargo: dep = "js-sys"
// cargo: dep = "anyhow"
// cargo: dep = "console_error_panic_hook"

mod components;
mod services;
mod models;
mod utils;
mod app;

use wasm_bindgen::prelude::*;
use app::App;

#[wasm_bindgen(start)]
pub fn run_app() {
    utils::set_panic_hook();
    yew::Renderer::<App>::new().render();
}