// cargo: dep = "yew"
// cargo: dep = "serde"
// cargo: dep = "reqwest"
// cargo: dep = "pulldown-cmark"
// cargo: dep = "futures-util"

mod utils;

use yew::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlSelectElement, HtmlElement, HtmlTextAreaElement, Event};
use pulldown_cmark::{Parser, Options, html, Event as MdEvent};
use futures_util::StreamExt;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

// -----------------------------------------------------------------------------
// Data Models
// -----------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Debug)]
pub struct ChatRequest {
    pub messages: Vec<Message>,
    pub model: String,
    pub temperature: f32,
    pub stream: bool,
}

#[derive(Deserialize, Debug)]
pub struct ChatChoice {
    pub message: Message,
}
#[derive(Deserialize, Debug)]
pub struct ChatResponse {
    pub choices: Vec<ChatChoice>,
}

#[derive(Deserialize, Debug)]
pub struct StreamDelta {
    pub content: Option<String>,
}
#[derive(Deserialize, Debug)]
pub struct StreamChoice {
    pub delta: StreamDelta,
}
#[derive(Deserialize, Debug)]
pub struct StreamResponse {
    pub choices: Vec<StreamChoice>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ModelInfo {
    pub id: String,
}
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ModelListResponse {
    pub data: Vec<ModelInfo>,
}

// -----------------------------------------------------------------------------
// Helper Functions
// -----------------------------------------------------------------------------

fn render_markdown(text: &str) -> Html {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);

    let parser = Parser::new_ext(text, options)
        .map(|event| match event {
            // Map the "SoftBreak" (newline) to a "HardBreak" (<br>)
            MdEvent::SoftBreak => MdEvent::HardBreak,
            _ => event,
        });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    let styled_html = format!(r#"<div class="markdown-body">{}</div>"#, html_output);
    Html::from_html_unchecked(AttrValue::from(styled_html))
}

// -----------------------------------------------------------------------------
// Main Application
// -----------------------------------------------------------------------------

#[function_component(App)]
pub fn app() -> Html {
    // --- STATE ---
    let default_sys = "You are a helpful assistant.";
    let system_prompt = use_state(|| default_sys.to_string());

    let messages = use_state(|| vec![
        Message {
            role: "system".to_string(),
            content: default_sys.to_string(),
        }
    ]);

    let input_text = use_state(|| String::new());
    let is_loading = use_state(|| false);

    let cancellation_token = use_state(|| Arc::new(AtomicBool::new(false)));

    // Settings
    let show_settings = use_state(|| false);
    let base_url = use_state(|| "http://localhost:8080".to_string());
    let available_models = use_state(|| Vec::<String>::new());
    let selected_model = use_state(|| "default".to_string());
    let stream_enabled = use_state(|| true);
    let settings_error = use_state(|| String::new());

    // Scroll
    let chat_container_ref = use_node_ref();
    let should_auto_scroll = use_state(|| true);

    // --- EFFECTS ---
    {
        let chat_container_ref = chat_container_ref.clone();
        let should_auto_scroll = should_auto_scroll.clone();

        use_effect_with(messages.clone(), move |_| {
            if *should_auto_scroll {
                if let Some(div) = chat_container_ref.cast::<HtmlElement>() {
                    div.set_scroll_top(div.scroll_height());
                }
            }
        });
    }

    // --- CORE LOGIC: SEND MESSAGE ---
    // Defined as a reusable callback so it can be called by Button OR Keypress
    let perform_send = {
        let messages = messages.clone();
        let input_text = input_text.clone();
        let is_loading = is_loading.clone();
        let base_url = base_url.clone();
        let selected_model = selected_model.clone();
        let stream_enabled = stream_enabled.clone();
        let cancellation_token = cancellation_token.clone();
        let should_auto_scroll = should_auto_scroll.clone();

        Callback::from(move |_: ()| {
            // Basic validation
            if input_text.is_empty() || *is_loading { return; }

            cancellation_token.store(false, Ordering::Relaxed);
            should_auto_scroll.set(true);

            // 1. Add User Message
            let mut current_history = (*messages).clone();
            current_history.push(Message { role: "user".into(), content: (*input_text).clone() });
            messages.set(current_history.clone());

            // 2. Clear Input & Set Loading
            input_text.set(String::new());
            is_loading.set(true);

            // 3. Prepare Async Task
            let messages_state = messages.clone();
            let is_loading_state = is_loading.clone();
            let clean_url = base_url.trim_end_matches('/').to_string();
            let model_id = (*selected_model).clone();
            let is_stream = *stream_enabled;
            let cancel_flag = cancellation_token.clone();

            spawn_local(async move {
                let client = reqwest::Client::new();
                let request_body = ChatRequest {
                    messages: current_history.clone(),
                    model: model_id,
                    temperature: 0.7,
                    stream: is_stream,
                };

                let response_result = client.post(format!("{}/v1/chat/completions", clean_url))
                    .json(&request_body)
                    .send()
                    .await;

                match response_result {
                    Ok(response) => {
                        if is_stream {
                            // Streaming Logic
                            let mut stream_history = current_history.clone();
                            stream_history.push(Message { role: "assistant".into(), content: "".into() });
                            messages_state.set(stream_history.clone());

                            let mut stream = response.bytes_stream();
                            let mut buffer = String::new();

                            while let Some(item) = stream.next().await {
                                if cancel_flag.load(Ordering::Relaxed) { break; }

                                if let Ok(chunk_bytes) = item {
                                    let chunk_str = String::from_utf8_lossy(&chunk_bytes);
                                    buffer.push_str(&chunk_str);

                                    while let Some(pos) = buffer.find('\n') {
                                        let line = buffer[..pos].to_string();
                                        buffer.drain(..pos + 1);

                                        let trimmed = line.trim();
                                        if trimmed.starts_with("data: ") {
                                            let json_str = &trimmed[6..];
                                            if json_str == "[DONE]" { break; }
                                            if let Ok(json) = serde_json::from_str::<StreamResponse>(json_str) {
                                                if let Some(choice) = json.choices.first() {
                                                    if let Some(content) = &choice.delta.content {
                                                        if let Some(last_msg) = stream_history.last_mut() {
                                                            last_msg.content.push_str(content);
                                                        }
                                                        messages_state.set(stream_history.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // Standard Logic
                            if let Ok(json) = response.json::<ChatResponse>().await {
                                if !cancel_flag.load(Ordering::Relaxed) {
                                    if let Some(choice) = json.choices.first() {
                                        let mut new_hist = current_history;
                                        new_hist.push(choice.message.clone());
                                        messages_state.set(new_hist);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if !cancel_flag.load(Ordering::Relaxed) {
                            let mut error_hist = current_history;
                            error_hist.push(Message {
                                role: "system".into(),
                                content: format!("Error: {}", e)
                            });
                            messages_state.set(error_hist);
                        }
                    }
                }
                is_loading_state.set(false);
            });
        })
    };

    // --- EVENT HANDLERS ---

    // 1. On Submit Button Click
    let on_submit_click = {
        let perform_send = perform_send.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default(); // Prevent form reload
            perform_send.emit(());
        })
    };

    // 2. On Enter Key in Textarea
    let on_keydown = {
        let perform_send = perform_send.clone();
        Callback::from(move |e: KeyboardEvent| {
            // Check if Enter is pressed
            if e.key() == "Enter" {
                // If Shift or Ctrl is NOT pressed, send message.
                // If Shift/Ctrl IS pressed, allow default behavior (new line).
                if !e.shift_key() && !e.ctrl_key() {
                    e.prevent_default(); // Prevent new line
                    perform_send.emit(());
                }
            }
        })
    };

    // 3. Input Handling (Textarea)
    let on_input_text = {
        let input_text = input_text.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlTextAreaElement = e.target_unchecked_into();
            input_text.set(input.value());
        })
    };

    // ... (Existing Callbacks: Scroll, Settings, etc. - kept same) ...
    let on_scroll_chat = {
        let should_auto_scroll = should_auto_scroll.clone();
        Callback::from(move |e: Event| {
            let div: HtmlElement = e.target_unchecked_into();
            let at_bottom = (div.scroll_height() - div.scroll_top() - div.client_height()) < 50;
            should_auto_scroll.set(at_bottom);
        })
    };
    let on_toggle_settings = {
        let show_settings = show_settings.clone();
        Callback::from(move |_| show_settings.set(!*show_settings))
    };
    let on_system_prompt_change = {
        let system_prompt = system_prompt.clone();
        let messages = messages.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlTextAreaElement = e.target_unchecked_into();
            let val = input.value();
            system_prompt.set(val.clone());
            let mut current_msgs = (*messages).clone();
            if let Some(first) = current_msgs.first_mut() {
                if first.role == "system" {
                    first.content = val;
                    messages.set(current_msgs);
                }
            }
        })
    };
    let on_url_change = {
        let base_url = base_url.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            base_url.set(input.value());
        })
    };
    let on_stream_change = {
        let stream_enabled = stream_enabled.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            stream_enabled.set(input.checked());
        })
    };
    let on_model_select = {
        let selected_model = selected_model.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            selected_model.set(select.value());
        })
    };
    let on_stop = {
        let cancellation_token = cancellation_token.clone();
        let is_loading = is_loading.clone();
        Callback::from(move |_| {
            cancellation_token.store(true, Ordering::Relaxed);
            is_loading.set(false);
        })
    };
    let on_fetch_models = {
        let base_url = base_url.clone();
        let available_models = available_models.clone();
        let selected_model = selected_model.clone();
        let settings_error = settings_error.clone();
        Callback::from(move |_| {
            let base_url = (*base_url).clone();
            let available_models = available_models.clone();
            let selected_model = selected_model.clone();
            let settings_error = settings_error.clone();
            spawn_local(async move {
                settings_error.set(String::new());
                let client = reqwest::Client::new();
                let clean_url = base_url.trim_end_matches('/');
                let url = format!("{}/v1/models", clean_url);
                match client.get(&url).send().await {
                    Ok(resp) => {
                        match resp.json::<ModelListResponse>().await {
                            Ok(json) => {
                                let names: Vec<String> = json.data.into_iter().map(|m| m.id).collect();
                                if names.len() == 1 { selected_model.set(names[0].clone()); }
                                else if !names.is_empty() && !names.contains(&*selected_model) { selected_model.set(names[0].clone()); }
                                available_models.set(names);
                            }
                            Err(_) => settings_error.set("Failed to parse models JSON.".into()),
                        }
                    }
                    Err(e) => settings_error.set(format!("Connection failed: {}", e)),
                }
            });
        })
    };

    // --- STYLES ---
    let global_styles = html! {
        <style>
            { "
            .markdown-body p { margin-bottom: 0.5em; margin-top: 0; }
            .markdown-body pre { background: #333; color: #fff; padding: 10px; border-radius: 4px; overflow-x: auto; }
            .markdown-body code { background: #eee; padding: 2px 4px; border-radius: 2px; font-family: monospace; }
            .markdown-body pre code { background: transparent; color: inherit; }
            .markdown-body ul, .markdown-body ol { padding-left: 20px; }

            .chat-scroll-container { scroll-behavior: smooth; }

            /* Custom Scrollbar for Textarea */
            textarea { resize: none; overflow-y: auto; }
            " }
        </style>
    };

    let container_style = "font-family: sans-serif; max_width: 800px; margin: 0 auto; padding: 20px; position: relative;";
    let chat_box_style = "border: 1px solid #ccc; padding: 10px; height: 500px; overflow-y: auto; margin-bottom: 10px; border-radius: 4px; display: flex; flex-direction: column; gap: 10px;";
    let settings_modal_style = "position: absolute; top: 60px; right: 20px; width: 320px; background: white; border: 1px solid #aaa; box-shadow: 0 4px 8px rgba(0,0,0,0.1); padding: 15px; border-radius: 8px; z-index: 10;";

    let get_bubble_style = |role: &str| {
        if role == "user" {
            "background-color: #e1f5fe; padding: 10px; text-align: right; border-radius: 10px 10px 0 10px; align-self: flex-end; max-width: 80%; box-shadow: 1px 1px 2px rgba(0,0,0,0.1);"
        } else if role == "system" {
            "background-color: #f8f9fa; color: #666; padding: 8px; border-radius: 8px; align-self: center; font-size: 0.85em; width: 90%; border: 1px dashed #ccc;"
        } else {
            "background-color: #f1f1f1; padding: 10px; text-align: left; border-radius: 10px 10px 10px 0; align-self: flex-start; max-width: 80%; box-shadow: 1px 1px 2px rgba(0,0,0,0.1);"
        }
    };

    html! {
        <div style={container_style}>
            { global_styles }
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px;">
                <h1 style="margin: 0;">{ "Local LLM Chat" }</h1>
                <button onclick={on_toggle_settings} style="padding: 5px 10px; cursor: pointer;">
                    { if *show_settings { "Close Settings" } else { "⚙ Settings" } }
                </button>
            </div>

            if *show_settings {
                <div style={settings_modal_style}>
                    <h3>{ "Configuration" }</h3>
                    <label style="display: block; margin-bottom: 5px; font-weight: bold;">{ "System Prompt:" }</label>
                    <textarea value={(*system_prompt).clone()} oninput={on_system_prompt_change} style="width: 100%; height: 80px; margin-bottom: 15px; padding: 5px; font-family: sans-serif; resize: vertical;" />
                    <label style="display: block; margin-bottom: 5px;">{ "Server URL:" }</label>
                    <div style="display: flex; gap: 5px; margin-bottom: 15px;">
                        <input type="text" value={(*base_url).clone()} oninput={on_url_change} style="flex-grow: 1;" />
                        <button onclick={on_fetch_models}>{ "⟳" }</button>
                    </div>
                    <label style="display: block; margin-bottom: 5px;">{ "Select Model:" }</label>
                    <select onchange={on_model_select} style="width: 100%; margin-bottom: 15px; padding: 5px;">
                        {
                            if available_models.is_empty() { html! { <option value="default">{ "Default (Manual)" }</option> } }
                            else { html! { for available_models.iter().map(|m| { let selected = m == &*selected_model; html! { <option value={m.clone()} selected={selected}>{ m }</option> } }) } }
                        }
                    </select>
                    <label style="display: flex; align-items: center; gap: 8px; cursor: pointer;">
                        <input type="checkbox" checked={*stream_enabled} onchange={on_stream_change} />
                        { "Enable Streaming" }
                    </label>
                    if !settings_error.is_empty() { <div style="color: red; font-size: 0.8em; margin-top: 10px;">{ &*settings_error }</div> }
                </div>
            }

            <div class="chat-scroll-container" style={chat_box_style} ref={chat_container_ref} onscroll={on_scroll_chat}>
                {
                    for messages.iter().map(|msg| {
                        html! {
                            <div style={get_bubble_style(&msg.role)}>
                                <strong>{ format!("{}: ", msg.role.to_uppercase()) }</strong>
                                { render_markdown(&msg.content) }
                            </div>
                        }
                    })
                }
                if *is_loading && !*stream_enabled { <div style="color: gray; font-style: italic; margin-left: 10px;">{ "Thinking..." }</div> }
            </div>

            // --- CHANGED: Input Area ---
            <form onsubmit={on_submit_click} style="display: flex; flex-direction: column;">
                <div style="display: flex; gap: 10px; align-items: flex-start;">
                    <textarea
                        value={(*input_text).clone()}
                        oninput={on_input_text}
                        onkeydown={on_keydown} // Attach Key Handler
                        disabled={*is_loading}
                        style="flex-grow: 1; padding: 10px; height: 50px; font-family: sans-serif;"
                        placeholder="Type a message... (Shift+Enter for new line)"
                    />

                    if *is_loading {
                        <button type="button" onclick={on_stop} style="height: 50px; padding: 0 20px; background-color: #ffcdd2; border: 1px solid #e57373; cursor: pointer;">
                            { "Stop" }
                        </button>
                    } else {
                        <button type="submit" style="height: 50px; padding: 0 20px; cursor: pointer;">
                            { "Send" }
                        </button>
                    }
                </div>
                <div style="font-size: 0.8em; color: #666; margin-top: 5px; text-align: right;">
                    { "Model: " } <strong>{ &*selected_model }</strong>
                    { if *stream_enabled { " (Streaming)" } else { "" } }
                </div>
            </form>
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    utils::set_panic_hook();
    yew::Renderer::<App>::new().render();
}