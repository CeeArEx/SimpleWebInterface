// cargo: dep = "yew"
// cargo: dep = "serde"
// cargo: dep = "serde_json"
// cargo: dep = "reqwest"
// cargo: dep = "pulldown-cmark"
// cargo: dep = "futures-util"
// cargo: dep = "wasm-bindgen"
// cargo: dep = "wasm-bindgen-futures"
// cargo: dep = "web-sys"
// Note: enable web-sys features: ["HtmlSelectElement", "HtmlElement", "HtmlTextAreaElement", "HtmlInputElement", "Window", "Storage"]

mod utils;

use yew::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlSelectElement, HtmlElement, HtmlTextAreaElement, Event, HtmlInputElement};
use pulldown_cmark::{Parser, Options, html, Event as MdEvent};
use futures_util::StreamExt;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

// -----------------------------------------------------------------------------
// Storage & Persistence Helpers
// -----------------------------------------------------------------------------

const KEY_MESSAGES: &str = "chat_history_v1";
const KEY_SETTINGS: &str = "chat_settings_v1";

struct LocalStorage;

impl LocalStorage {
    fn get<T: for<'de> Deserialize<'de>>(key: &str) -> Option<T> {
        let window = web_sys::window()?;
        let storage = window.local_storage().ok()??;
        let json = storage.get_item(key).ok()??;
        serde_json::from_str(&json).ok()
    }

    fn set<T: Serialize>(key: &str, value: &T) {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(json) = serde_json::to_string(value) {
                    let _ = storage.set_item(key, &json);
                }
            }
        }
    }

    fn remove(key: &str) {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                let _ = storage.remove_item(key);
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Data Models
// -----------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct AppSettings {
    pub system_prompt: String,
    pub base_url: String,
    pub selected_model: String,
    pub stream_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            system_prompt: "You are a helpful assistant.".to_string(),
            base_url: "http://localhost:8080".to_string(),
            selected_model: "default".to_string(),
            stream_enabled: true,
        }
    }
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
    // --- STATE INITIALIZATION (Load from Storage) ---

    // 1. Settings
    // We load the struct once, then distribute to individual states for UI convenience
    let initial_settings = LocalStorage::get::<AppSettings>(KEY_SETTINGS).unwrap_or_default();

    let system_prompt = use_state(|| initial_settings.system_prompt.clone());
    let base_url = use_state(|| initial_settings.base_url.clone());
    let selected_model = use_state(|| initial_settings.selected_model.clone());
    let stream_enabled = use_state(|| initial_settings.stream_enabled);

    // 2. Messages
    let messages = use_state(|| {
        LocalStorage::get::<Vec<Message>>(KEY_MESSAGES).unwrap_or_else(|| vec![
            Message {
                role: "system".to_string(),
                content: initial_settings.system_prompt.clone(),
            }
        ])
    });

    // 3. Transient UI State (Not saved)
    let input_text = use_state(|| String::new());
    let editing_index = use_state(|| None::<usize>);
    let edit_buffer = use_state(|| String::new());
    let is_loading = use_state(|| false);
    let cancellation_token = use_state(|| Arc::new(AtomicBool::new(false)));
    let show_settings = use_state(|| false);
    let available_models = use_state(|| Vec::<String>::new());
    let settings_error = use_state(|| String::new());
    let chat_container_ref = use_node_ref();
    let should_auto_scroll = use_state(|| true);

    // --- EFFECTS: AUTO-SAVE ---

    // Save Messages whenever they change
    {
        let messages = messages.clone();
        use_effect_with(messages, |msgs| {
            LocalStorage::set(KEY_MESSAGES, &**msgs);
        });
    }

    // Save Settings whenever any config changes
    {
        let sp = system_prompt.clone();
        let bu = base_url.clone();
        let sm = selected_model.clone();
        let se = stream_enabled.clone();

        use_effect_with(
            (sp.clone(), bu.clone(), sm.clone(), se.clone()),
            move |(sp, bu, sm, se)| {
                let settings = AppSettings {
                    system_prompt: (**sp).clone(),
                    base_url: (**bu).clone(),
                    selected_model: (**sm).clone(),
                    stream_enabled: **se,
                };
                LocalStorage::set(KEY_SETTINGS, &settings);
            }
        );
    }

    // --- EFFECTS: SCROLLING ---
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

    // --- LOGIC: CHAT COMPLETION ---
    let run_chat_completion = {
        let messages_state = messages.clone();
        let is_loading = is_loading.clone();
        let base_url = base_url.clone();
        let selected_model = selected_model.clone();
        let stream_enabled = stream_enabled.clone();
        let cancellation_token = cancellation_token.clone();

        Callback::from(move |history_to_send: Vec<Message>| {
            is_loading.set(true);
            cancellation_token.store(false, Ordering::Relaxed);
            messages_state.set(history_to_send.clone());

            let messages_state = messages_state.clone();
            let is_loading_state = is_loading.clone();
            let clean_url = base_url.trim_end_matches('/').to_string();
            let model_id = (*selected_model).clone();
            let is_stream = *stream_enabled;
            let cancel_flag = cancellation_token.clone();

            spawn_local(async move {
                let client = reqwest::Client::new();
                let request_body = ChatRequest {
                    messages: history_to_send.clone(),
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
                            let mut stream_history = history_to_send.clone();
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
                            if let Ok(json) = response.json::<ChatResponse>().await {
                                if !cancel_flag.load(Ordering::Relaxed) {
                                    if let Some(choice) = json.choices.first() {
                                        let mut new_hist = history_to_send;
                                        new_hist.push(choice.message.clone());
                                        messages_state.set(new_hist);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if !cancel_flag.load(Ordering::Relaxed) {
                            let mut error_hist = history_to_send;
                            error_hist.push(Message { role: "system".into(), content: format!("Error: {}", e) });
                            messages_state.set(error_hist);
                        }
                    }
                }
                is_loading_state.set(false);
            });
        })
    };

    // --- HANDLERS: INPUTS & ACTIONS ---

    let perform_send = {
        let messages = messages.clone();
        let input_text = input_text.clone();
        let is_loading = is_loading.clone();
        let should_auto_scroll = should_auto_scroll.clone();
        let run_chat_completion = run_chat_completion.clone();

        Callback::from(move |_: ()| {
            if input_text.is_empty() || *is_loading { return; }
            should_auto_scroll.set(true);
            let mut new_history = (*messages).clone();
            new_history.push(Message { role: "user".into(), content: (*input_text).clone() });
            input_text.set(String::new());
            run_chat_completion.emit(new_history);
        })
    };

    let on_edit_click = {
        let editing_index = editing_index.clone();
        let edit_buffer = edit_buffer.clone();
        let messages = messages.clone();
        Callback::from(move |idx: usize| {
            if let Some(msg) = messages.get(idx) {
                editing_index.set(Some(idx));
                edit_buffer.set(msg.content.clone());
            }
        })
    };

    let on_edit_cancel = {
        let editing_index = editing_index.clone();
        let edit_buffer = edit_buffer.clone();
        Callback::from(move |_| {
            editing_index.set(None);
            edit_buffer.set(String::new());
        })
    };

    let on_edit_save = {
        let editing_index = editing_index.clone();
        let edit_buffer = edit_buffer.clone();
        let messages = messages.clone();
        let run_chat_completion = run_chat_completion.clone();
        let should_auto_scroll = should_auto_scroll.clone();

        Callback::from(move |idx: usize| {
            should_auto_scroll.set(true);
            let mut branched_history = (*messages).clone();
            if idx < branched_history.len() {
                branched_history.truncate(idx + 1);
            }
            if let Some(msg) = branched_history.last_mut() {
                msg.content = (*edit_buffer).clone();
            }
            editing_index.set(None);
            edit_buffer.set(String::new());
            run_chat_completion.emit(branched_history);
        })
    };

    let on_edit_input = {
        let edit_buffer = edit_buffer.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlTextAreaElement = e.target_unchecked_into();
            edit_buffer.set(input.value());
        })
    };

    let on_submit_click = {
        let perform_send = perform_send.clone();
        Callback::from(move |e: SubmitEvent| { e.prevent_default(); perform_send.emit(()); })
    };
    let on_keydown = {
        let perform_send = perform_send.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" && !e.shift_key() && !e.ctrl_key() {
                e.prevent_default();
                perform_send.emit(());
            }
        })
    };
    let on_input_text = {
        let input_text = input_text.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlTextAreaElement = e.target_unchecked_into();
            input_text.set(input.value());
        })
    };

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

    // --- SETTINGS HANDLERS ---

    let on_system_prompt_change = {
        let system_prompt = system_prompt.clone();
        let messages = messages.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlTextAreaElement = e.target_unchecked_into();
            let val = input.value();
            system_prompt.set(val.clone());
            // Update the live system message if it's the first one
            let mut current_msgs = (*messages).clone();
            if let Some(first) = current_msgs.first_mut() {
                if first.role == "system" { first.content = val; messages.set(current_msgs); }
            }
        })
    };
    let on_url_change = {
        let base_url = base_url.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            base_url.set(input.value());
        })
    };
    let on_stream_change = {
        let stream_enabled = stream_enabled.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
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

    // --- CLEANUP HANDLERS (Wipe Data) ---

    let on_clear_history = {
        let messages = messages.clone();
        let system_prompt = system_prompt.clone();
        Callback::from(move |_| {
            if web_sys::window().unwrap().confirm_with_message("Delete all chat history?").unwrap_or(false) {
                messages.set(vec![Message { role: "system".into(), content: (*system_prompt).clone() }]);
                LocalStorage::remove(KEY_MESSAGES);
            }
        })
    };

    let on_wipe_settings = {
        let system_prompt = system_prompt.clone();
        let base_url = base_url.clone();
        let selected_model = selected_model.clone();
        let stream_enabled = stream_enabled.clone();
        let messages = messages.clone();
        let available_models = available_models.clone();

        Callback::from(move |_| {
            if web_sys::window().unwrap().confirm_with_message("Reset all settings to default?").unwrap_or(false) {
                LocalStorage::remove(KEY_SETTINGS);
                let def = AppSettings::default();

                system_prompt.set(def.system_prompt.clone());
                base_url.set(def.base_url);
                selected_model.set(def.selected_model);
                stream_enabled.set(def.stream_enabled);
                available_models.set(Vec::new());

                // Reset chat history logic to use the default prompt
                let mut current_msgs = (*messages).clone();
                if let Some(first) = current_msgs.first_mut() {
                    if first.role == "system" {
                        first.content = def.system_prompt;
                        messages.set(current_msgs);
                    }
                }
            }
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
                match client.get(format!("{}/v1/models", clean_url)).send().await {
                    Ok(resp) => {
                        match resp.json::<ModelListResponse>().await {
                            Ok(json) => {
                                let names: Vec<String> = json.data.into_iter().map(|m| m.id).collect();
                                if names.len() == 1 || (!names.is_empty() && !names.contains(&*selected_model)) {
                                    selected_model.set(names[0].clone());
                                }
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
            .chat-scroll-container { scroll-behavior: smooth; }
            textarea { resize: none; overflow-y: auto; }
            .msg-actions { opacity: 0; transition: opacity 0.2s; margin-top: 5px; font-size: 0.8em; }
            .msg-container:hover .msg-actions { opacity: 1; }
            .btn-edit { background: none; border: none; color: #666; cursor: pointer; text-decoration: underline; padding: 0; }
            .btn-edit:hover { color: #000; }
            .danger-btn { background-color: #ffebee; color: #c62828; border: 1px solid #c62828; padding: 5px 10px; border-radius: 4px; cursor: pointer; font-size: 0.9em; margin-top: 5px; width: 100%; }
            .danger-btn:hover { background-color: #ffcdd2; }
            " }
        </style>
    };

    let container_style = "font-family: sans-serif; max_width: 800px; margin: 0 auto; padding: 20px; position: relative;";
    let chat_box_style = "border: 1px solid #ccc; padding: 10px; height: 500px; overflow-y: auto; margin-bottom: 10px; border-radius: 4px; display: flex; flex-direction: column; gap: 10px;";

    let get_bubble_style = |role: &str| {
        if role == "user" {
            "background-color: #e1f5fe; padding: 10px; text-align: left; border-radius: 10px 10px 0 10px; align-self: flex-end; max-width: 80%; box-shadow: 1px 1px 2px rgba(0,0,0,0.1); position: relative;"
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
                <div style="position: absolute; top: 60px; right: 20px; width: 320px; background: white; border: 1px solid #aaa; box-shadow: 0 4px 8px rgba(0,0,0,0.1); padding: 15px; border-radius: 8px; z-index: 10;">
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

                    <label style="display: flex; align-items: center; gap: 8px; cursor: pointer; margin-bottom: 15px;">
                        <input type="checkbox" checked={*stream_enabled} onchange={on_stream_change} />
                        { "Enable Streaming" }
                    </label>

                    <hr style="border: 0; border-top: 1px solid #eee; margin: 15px 0;" />

                    <div style="display: flex; flex-direction: column; gap: 10px;">
                        <button onclick={on_clear_history} class="danger-btn">
                            { "🗑 Delete All Chats" }
                        </button>
                        <button onclick={on_wipe_settings} class="danger-btn">
                            { "⚠ Reset All Settings" }
                        </button>
                    </div>

                    if !settings_error.is_empty() { <div style="color: red; font-size: 0.8em; margin-top: 10px;">{ &*settings_error }</div> }
                </div>
            }

            <div class="chat-scroll-container" style={chat_box_style} ref={chat_container_ref} onscroll={on_scroll_chat}>
                {
                    for messages.iter().enumerate().map(|(idx, msg)| {
                        let is_user = msg.role == "user";
                        let is_editing = *editing_index == Some(idx);
                        let on_save_click = on_edit_save.clone();
                        let on_edit_open = on_edit_click.clone();

                        html! {
                            <div class="msg-container" style={get_bubble_style(&msg.role)}>
                                <strong>{ format!("{}: ", msg.role.to_uppercase()) }</strong>

                                if is_editing {
                                    <div style="margin-top: 5px;">
                                        <textarea
                                            value={(*edit_buffer).clone()}
                                            oninput={on_edit_input.clone()}
                                            style="width: 100%; height: 80px; padding: 5px; box-sizing: border-box; font-family: sans-serif; display: block;"
                                        />
                                        <div style="margin-top: 5px; display: flex; gap: 5px; justify-content: flex-end;">
                                            <button
                                                onclick={on_edit_cancel.clone()}
                                                style="padding: 3px 8px; cursor: pointer;"
                                            >
                                                { "Cancel" }
                                            </button>
                                            <button
                                                onclick={Callback::from(move |_| on_save_click.emit(idx))}
                                                style="padding: 3px 8px; cursor: pointer; background-color: #4caf50; color: white; border: none; border-radius: 3px;"
                                            >
                                                { "Save & Branch" }
                                            </button>
                                        </div>
                                    </div>
                                } else {
                                    { render_markdown(&msg.content) }

                                    if is_user && !(*is_loading) {
                                        <div class="msg-actions" style="text-align: right;">
                                            <button class="btn-edit" onclick={Callback::from(move |_| on_edit_open.emit(idx))}>
                                                { "✏ Edit" }
                                            </button>
                                        </div>
                                    }
                                }
                            </div>
                        }
                    })
                }
                if *is_loading && !*stream_enabled { <div style="color: gray; font-style: italic; margin-left: 10px;">{ "Thinking..." }</div> }
            </div>

            <form onsubmit={on_submit_click} style="display: flex; flex-direction: column;">
                <div style="display: flex; gap: 10px; align-items: flex-start;">
                    <textarea
                        value={(*input_text).clone()}
                        oninput={on_input_text}
                        onkeydown={on_keydown}
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
            </form>
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    utils::set_panic_hook();
    yew::Renderer::<App>::new().render();
}