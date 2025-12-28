use pulldown_cmark::{Parser, Options, html, Event as MdEvent};
use yew::{Html, AttrValue};
use std::panic;

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

pub fn render_markdown(text: &str) -> Html {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);

    let parser = Parser::new_ext(text, options).map(|event| match event {
        MdEvent::SoftBreak => MdEvent::HardBreak,
        _ => event,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    let styled_html = format!(r#"<div class="markdown-body">{}</div>"#, html_output);
    Html::from_html_unchecked(AttrValue::from(styled_html))
}