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

mod utils;

use yew::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlSelectElement, HtmlElement, HtmlTextAreaElement, HtmlInputElement};
use pulldown_cmark::{Parser, Options, html, Event as MdEvent};
use futures_util::StreamExt;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use uuid::Uuid;

// -----------------------------------------------------------------------------
// Storage Keys
// -----------------------------------------------------------------------------

const KEY_CHATS: &str = "llm_chats_v2";
const KEY_SETTINGS: &str = "chat_settings_v1";

// -----------------------------------------------------------------------------
// Storage Helper
// -----------------------------------------------------------------------------

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
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub created_at: f64,
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
    let parser = Parser::new_ext(text, options).map(|event| match event {
        MdEvent::SoftBreak => MdEvent::HardBreak,
        _ => event,
    });
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    let styled_html = format!(r#"<div class="markdown-body">{}</div>"#, html_output);
    Html::from_html_unchecked(AttrValue::from(styled_html))
}

fn create_new_chat(system_prompt: String) -> ChatSession {
    ChatSession {
        id: Uuid::new_v4().to_string(),
        title: "New Chat".to_string(),
        messages: vec![Message {
            role: "system".to_string(),
            content: system_prompt,
        }],
        created_at: js_sys::Date::now(),
    }
}

// -----------------------------------------------------------------------------
// Main Application
// -----------------------------------------------------------------------------

#[function_component(App)]
pub fn app() -> Html {
    // --- STATE INITIALIZATION ---
    let initial_settings = LocalStorage::get::<AppSettings>(KEY_SETTINGS).unwrap_or_default();

    let system_prompt = use_state(|| initial_settings.system_prompt.clone());
    let base_url = use_state(|| initial_settings.base_url.clone());
    let selected_model = use_state(|| initial_settings.selected_model.clone());
    let stream_enabled = use_state(|| initial_settings.stream_enabled);

    let chats = use_state(|| {
        LocalStorage::get::<Vec<ChatSession>>(KEY_CHATS).unwrap_or_else(|| {
            vec![create_new_chat(initial_settings.system_prompt.clone())]
        })
    });

    let active_chat_id = use_state(|| {
        chats.first().map(|c| c.id.clone()).unwrap_or_default()
    });

    // New State to trigger title generation cleanly
    let title_check_queue = use_state(|| None::<String>);

    let sidebar_open = use_state(|| true);
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

    // --- COMPUTED: CURRENT CHAT ---
    let current_chat_idx = chats.iter().position(|c| c.id == *active_chat_id);
    let current_messages = if let Some(idx) = current_chat_idx {
        chats[idx].messages.clone()
    } else {
        vec![]
    };

    // --- EFFECTS: PERSISTENCE ---
    {
        let chats = chats.clone();
        use_effect_with(chats, |chats_state| {
            LocalStorage::set(KEY_CHATS, &**chats_state);
        });
    }

    {
        let sp = system_prompt.clone();
        let bu = base_url.clone();
        let sm = selected_model.clone();
        let se = stream_enabled.clone();
        use_effect_with((sp.clone(), bu.clone(), sm.clone(), se.clone()), move |(sp, bu, sm, se)| {
            let settings = AppSettings {
                system_prompt: (**sp).clone(),
                base_url: (**bu).clone(),
                selected_model: (**sm).clone(),
                stream_enabled: **se,
            };
            LocalStorage::set(KEY_SETTINGS, &settings);
        });
    }

    // Scroll to bottom
    {
        let chat_container_ref = chat_container_ref.clone();
        let should_auto_scroll = should_auto_scroll.clone();
        use_effect_with(chats.clone(), move |_| {
            if *should_auto_scroll {
                if let Some(div) = chat_container_ref.cast::<HtmlElement>() {
                    div.set_scroll_top(div.scroll_height());
                }
            }
        });
    }

    // --- LOGIC: TITLE GENERATION (Decoupled Effect) ---
    {
        let chats = chats.clone();
        let base_url = base_url.clone();
        let selected_model = selected_model.clone();
        let title_check_queue = title_check_queue.clone();

        use_effect_with(title_check_queue.clone(), move |queue_state| {
            if let Some(chat_id) = &**queue_state {
                let chat_id = chat_id.clone();
                let chats = chats.clone();
                let clean_url = base_url.trim_end_matches('/').to_string();
                let model_id = (*selected_model).clone();

                // 1. Check if we actually need to generate a title
                // We do this check synchronously to determine if we should spawn the task
                let need_title = if let Some(chat) = chats.iter().find(|c| c.id == chat_id) {
                    chat.messages.len() <= 4 && chat.title == "New Chat"
                } else {
                    false
                };

                if need_title {
                    let msgs_opt = chats.iter().find(|c| c.id == chat_id).map(|c| c.messages.clone());

                    if let Some(msgs) = msgs_opt {
                        spawn_local(async move {
                            let client = reqwest::Client::new();
                            let mut summary_messages = msgs.clone();
                            summary_messages.push(Message {
                                role: "user".into(),
                                content: "Based on the conversation above, generate a short, relevant title for this chat (max 4-6 words). Do not use quotes.".into()
                            });

                            let request_body = ChatRequest {
                                messages: summary_messages,
                                model: model_id,
                                temperature: 0.7,
                                stream: false,
                            };

                            if let Ok(resp) = client.post(format!("{}/v1/chat/completions", clean_url)).json(&request_body).send().await {
                                if let Ok(json) = resp.json::<ChatResponse>().await {
                                    if let Some(choice) = json.choices.first() {
                                        let new_title = choice.message.content.trim().to_string();

                                        // 2. Safe Update: Re-read state inside the async completion
                                        let mut current_chats = (*chats).clone();
                                        if let Some(chat) = current_chats.iter_mut().find(|c| c.id == chat_id) {
                                            // Only update if the title is still "New Chat" (avoid overwrites if user renamed it manually)
                                            if chat.title == "New Chat" {
                                                chat.title = new_title;
                                                chats.set(current_chats);
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    }
                }
            }
        });
    }


    // --- LOGIC: CHAT COMPLETION ---
    let run_chat_completion = {
        let chats = chats.clone();
        let active_chat_id = active_chat_id.clone();
        let is_loading = is_loading.clone();
        let base_url = base_url.clone();
        let selected_model = selected_model.clone();
        let stream_enabled = stream_enabled.clone();
        let cancellation_token = cancellation_token.clone();
        let title_check_queue = title_check_queue.clone();

        Callback::from(move |history_to_send: Vec<Message>| {
            let current_id = (*active_chat_id).clone();
            is_loading.set(true);
            cancellation_token.store(false, Ordering::Relaxed);

            // Optimistic Update
            {
                let mut current_chats = (*chats).clone();
                if let Some(chat) = current_chats.iter_mut().find(|c| c.id == current_id) {
                    chat.messages = history_to_send.clone();
                    chats.set(current_chats);
                }
            }

            let chats_state = chats.clone();
            let is_loading_state = is_loading.clone();
            let clean_url = base_url.trim_end_matches('/').to_string();
            let model_id = (*selected_model).clone();
            let is_stream = *stream_enabled;
            let cancel_flag = cancellation_token.clone();
            let target_chat_id = current_id.clone();
            let title_queue = title_check_queue.clone();

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

                            {
                                let mut current_chats = (*chats_state).clone();
                                if let Some(chat) = current_chats.iter_mut().find(|c| c.id == target_chat_id) {
                                    chat.messages = stream_history.clone();
                                    chats_state.set(current_chats);
                                }
                            }

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

                                                        // Update state repeatedly
                                                        let mut current_chats = (*chats_state).clone();
                                                        if let Some(chat) = current_chats.iter_mut().find(|c| c.id == target_chat_id) {
                                                            chat.messages = stream_history.clone();
                                                            chats_state.set(current_chats);
                                                        }
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

                                        let mut current_chats = (*chats_state).clone();
                                        if let Some(chat) = current_chats.iter_mut().find(|c| c.id == target_chat_id) {
                                            chat.messages = new_hist;
                                            chats_state.set(current_chats);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if !cancel_flag.load(Ordering::Relaxed) {
                            let mut error_hist = history_to_send;
                            error_hist.push(Message { role: "system".into(), content: format!("Error: {}", e) });
                            let mut current_chats = (*chats_state).clone();
                            if let Some(chat) = current_chats.iter_mut().find(|c| c.id == target_chat_id) {
                                chat.messages = error_hist;
                                chats_state.set(current_chats);
                            }
                        }
                    }
                }
                is_loading_state.set(false);

                // Trigger Title Generation via State Change
                // This ensures the effect runs in the main scope with fresh handles
                title_queue.set(Some(target_chat_id));
            });
        })
    };

    // --- ACTIONS ---

    let on_new_chat = {
        let chats = chats.clone();
        let active_chat_id = active_chat_id.clone();
        let system_prompt = system_prompt.clone();
        Callback::from(move |_| {
            let new_chat = create_new_chat((*system_prompt).clone());
            let new_id = new_chat.id.clone();
            let mut current = (*chats).clone();
            current.insert(0, new_chat);
            chats.set(current);
            active_chat_id.set(new_id);
        })
    };

    let on_select_chat = {
        let active_chat_id = active_chat_id.clone();
        Callback::from(move |id: String| {
            active_chat_id.set(id);
        })
    };

    let on_delete_chat = {
        let chats = chats.clone();
        let active_chat_id = active_chat_id.clone();
        let system_prompt = system_prompt.clone();
        Callback::from(move |(e, id): (MouseEvent, String)| {
            e.stop_propagation();
            if web_sys::window().unwrap().confirm_with_message("Delete this chat?").unwrap_or(false) {
                let mut current = (*chats).clone();
                current.retain(|c| c.id != id);

                if *active_chat_id == id {
                    if let Some(first) = current.first() {
                        active_chat_id.set(first.id.clone());
                    } else {
                        let new_chat = create_new_chat((*system_prompt).clone());
                        active_chat_id.set(new_chat.id.clone());
                        current.push(new_chat);
                    }
                }
                chats.set(current);
            }
        })
    };

    let on_clear_all_chats = {
        let chats = chats.clone();
        let active_chat_id = active_chat_id.clone();
        let system_prompt = system_prompt.clone();
        Callback::from(move |_| {
            if web_sys::window().unwrap().confirm_with_message("Irreversibly delete ALL chat history?").unwrap_or(false) {
                let new_chat = create_new_chat((*system_prompt).clone());
                chats.set(vec![new_chat.clone()]);
                active_chat_id.set(new_chat.id);
            }
        })
    };

    let perform_send = {
        let input_text = input_text.clone();
        let is_loading = is_loading.clone();
        let should_auto_scroll = should_auto_scroll.clone();
        let run_chat_completion = run_chat_completion.clone();
        let current_messages = current_messages.clone();

        Callback::from(move |_: ()| {
            if input_text.is_empty() || *is_loading { return; }
            should_auto_scroll.set(true);

            let mut history_to_send = current_messages.clone();
            history_to_send.push(Message { role: "user".into(), content: (*input_text).clone() });

            input_text.set(String::new());
            run_chat_completion.emit(history_to_send);
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

    let on_edit_click = {
        let editing_index = editing_index.clone();
        let edit_buffer = edit_buffer.clone();
        let msgs = current_messages.clone();
        Callback::from(move |idx: usize| {
            if let Some(msg) = msgs.get(idx) {
                editing_index.set(Some(idx));
                edit_buffer.set(msg.content.clone());
            }
        })
    };

    let on_edit_save = {
        let editing_index = editing_index.clone();
        let edit_buffer = edit_buffer.clone();
        let msgs = current_messages.clone();
        let run_chat_completion = run_chat_completion.clone();
        Callback::from(move |idx: usize| {
            let mut branched_history = msgs.clone();
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

    let on_edit_cancel = {
        let editing_index = editing_index.clone();
        Callback::from(move |_| editing_index.set(None))
    };

    let on_edit_input = {
        let edit_buffer = edit_buffer.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlTextAreaElement = e.target_unchecked_into();
            edit_buffer.set(input.value());
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
    let on_toggle_sidebar = {
        let sidebar_open = sidebar_open.clone();
        Callback::from(move |_| sidebar_open.set(!*sidebar_open))
    };

    let on_system_prompt_change = {
        let system_prompt = system_prompt.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlTextAreaElement = e.target_unchecked_into();
            system_prompt.set(input.value());
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

    let on_stop = {
        let cancellation_token = cancellation_token.clone();
        let is_loading = is_loading.clone();
        Callback::from(move |_| {
            cancellation_token.store(true, Ordering::Relaxed);
            is_loading.set(false);
        })
    };

    let on_reset_settings = {
        let system_prompt = system_prompt.clone();
        let base_url = base_url.clone();
        let selected_model = selected_model.clone();
        let stream_enabled = stream_enabled.clone();
        Callback::from(move |_| {
            if web_sys::window().unwrap().confirm_with_message("Reset settings?").unwrap_or(false) {
                let def = AppSettings::default();
                system_prompt.set(def.system_prompt);
                base_url.set(def.base_url);
                selected_model.set(def.selected_model);
                stream_enabled.set(def.stream_enabled);
            }
        })
    };

    // --- STYLES ---

    let app_style = "display: flex; height: 100vh; overflow: hidden; font-family: sans-serif; color: #333;";
    let sidebar_width = if *sidebar_open { "260px" } else { "0px" };
    let sidebar_style = format!(
        "width: {}; background-color: #f7f7f7; border-right: 1px solid #ddd; display: flex; flex-direction: column; transition: width 0.3s ease; overflow: hidden; flex-shrink: 0;",
        sidebar_width
    );
    let main_style = "flex-grow: 1; display: flex; flex-direction: column; position: relative; min-width: 0;";

    let global_styles = html! {
        <style>
            { "
            .markdown-body p { margin-bottom: 0.5em; margin-top: 0; }
            .markdown-body pre { background: #333; color: #fff; padding: 10px; border-radius: 4px; overflow-x: auto; }
            .markdown-body code { background: #eee; padding: 2px 4px; border-radius: 2px; font-family: monospace; }
            .markdown-body pre code { background: transparent; color: inherit; }
            .chat-scroll-container { scroll-behavior: smooth; }
            textarea { resize: none; overflow-y: auto; }
            .chat-item { padding: 10px; cursor: pointer; border-bottom: 1px solid #eee; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; display: flex; justify-content: space-between; align-items: center; }
            .chat-item:hover { background-color: #eee; }
            .chat-item.active { background-color: #e3f2fd; border-left: 4px solid #2196f3; }
            .chat-item .del-btn { opacity: 0; background: none; border: none; color: #999; font-weight: bold; cursor: pointer; padding: 0 5px; }
            .chat-item:hover .del-btn { opacity: 1; }
            .chat-item .del-btn:hover { color: red; }
            .btn-icon { background: none; border: none; cursor: pointer; font-size: 1.2rem; padding: 5px; }
            .new-chat-btn { margin: 10px; padding: 10px; background-color: #2196f3; color: white; border: none; border-radius: 4px; cursor: pointer; text-align: center; }
            .new-chat-btn:hover { background-color: #1976d2; }
            " }
        </style>
    };

    let get_bubble_style = |role: &str| {
        if role == "user" {
            "background-color: #e1f5fe; padding: 10px; border-radius: 10px 10px 0 10px; align-self: flex-end; max-width: 80%; box-shadow: 1px 1px 2px rgba(0,0,0,0.1);"
        } else if role == "system" {
            "background-color: #fff3cd; color: #666; padding: 8px; border-radius: 8px; align-self: center; font-size: 0.85em; width: 90%; border: 1px dashed #ccc;"
        } else {
            "background-color: #f1f1f1; padding: 10px; border-radius: 10px 10px 10px 0; align-self: flex-start; max-width: 80%; box-shadow: 1px 1px 2px rgba(0,0,0,0.1);"
        }
    };

    html! {
        <div style={app_style}>
            { global_styles }

            <div style={sidebar_style}>
                <button class="new-chat-btn" onclick={on_new_chat}>{ "+ New Chat" }</button>
                <div style="flex-grow: 1; overflow-y: auto;">
                    {
                        for chats.iter().map(|chat| {
                            let id = chat.id.clone();
                            let is_active = id == *active_chat_id;
                            let active_class = if is_active { "active" } else { "" };

                            let on_select = on_select_chat.clone();
                            let id_for_select = id.clone();

                            let on_del = on_delete_chat.clone();
                            let id_for_delete = id.clone();

                            html! {
                                <div
                                    class={format!("chat-item {}", active_class)}
                                    onclick={Callback::from(move |_| on_select.emit(id_for_select.clone()))}
                                >
                                    <span style="overflow: hidden; text-overflow: ellipsis;">{ &chat.title }</span>
                                    <button
                                        class="del-btn"
                                        onclick={Callback::from(move |e: MouseEvent| on_del.emit((e, id_for_delete.clone())))}
                                        title="Delete Chat"
                                    >
                                        { "×" }
                                    </button>
                                </div>
                            }
                        })
                    }
                </div>
            </div>

            <div style={main_style}>
                <div style="display: flex; justify-content: space-between; align-items: center; padding: 10px; border-bottom: 1px solid #ddd; background: white;">
                    <div style="display: flex; align-items: center; gap: 10px;">
                        <button class="btn-icon" onclick={on_toggle_sidebar} title="Toggle Sidebar">{ "☰" }</button>
                        <h2 style="margin: 0; font-size: 1.2rem;">{ "Local LLM" }</h2>
                    </div>
                    <button class="btn-icon" onclick={on_toggle_settings}>{ "⚙" }</button>
                </div>

                if *show_settings {
                    <div style="position: absolute; top: 50px; right: 10px; width: 300px; background: white; border: 1px solid #ccc; box-shadow: 0 4px 10px rgba(0,0,0,0.1); padding: 15px; border-radius: 8px; z-index: 100;">
                        <h3>{ "Configuration" }</h3>
                        <label style="display: block; font-size: 0.9em; margin-bottom: 5px;">{ "System Prompt:" }</label>
                        <textarea value={(*system_prompt).clone()} oninput={on_system_prompt_change} style="width: 100%; height: 60px; margin-bottom: 10px;" />

                        <label style="display: block; font-size: 0.9em; margin-bottom: 5px;">{ "Server URL:" }</label>
                        <div style="display: flex; gap: 5px; margin-bottom: 10px;">
                            <input type="text" value={(*base_url).clone()} oninput={on_url_change} style="flex-grow: 1;" />
                            <button onclick={on_fetch_models}>{ "⟳" }</button>
                        </div>

                        <label style="display: block; font-size: 0.9em; margin-bottom: 5px;">{ "Model:" }</label>
                        <select onchange={on_model_select} style="width: 100%; margin-bottom: 10px;">
                            {
                                if available_models.is_empty() { html! { <option value="default">{ "Default" }</option> } }
                                else { html! { for available_models.iter().map(|m| { let sel = m == &*selected_model; html! { <option value={m.clone()} selected={sel}>{ m }</option> } }) } }
                            }
                        </select>
                        <label style="display: flex; gap: 5px; align-items: center; margin-bottom: 15px;">
                            <input type="checkbox" checked={*stream_enabled} onchange={on_stream_change} />
                            { "Stream Responses" }
                        </label>
                        <hr />
                        <div style="display: flex; flex-direction: column; gap: 5px; margin-top: 10px;">
                            <button onclick={on_clear_all_chats} style="background: #ffebee; color: #c62828; border: 1px solid #c62828; padding: 5px; border-radius: 4px; cursor: pointer;">{ "Delete All Chats" }</button>
                            <button onclick={on_reset_settings} style="background: #fff; border: 1px solid #ccc; padding: 5px; border-radius: 4px; cursor: pointer;">{ "Reset Settings" }</button>
                        </div>
                        if !settings_error.is_empty() { <div style="color: red; font-size: 0.8em; margin-top: 5px;">{ &*settings_error }</div> }
                    </div>
                }

                <div class="chat-scroll-container" style="flex-grow: 1; overflow-y: auto; padding: 20px; display: flex; flex-direction: column; gap: 15px;" ref={chat_container_ref} onscroll={on_scroll_chat}>
                    {
                        for current_messages.iter().enumerate().map(|(idx, msg)| {
                            let is_user = msg.role == "user";
                            let is_editing = *editing_index == Some(idx);
                            let on_save = on_edit_save.clone();
                            let on_cancel = on_edit_cancel.clone();
                            let on_open = on_edit_click.clone();

                            html! {
                                <div class="msg-container" style={get_bubble_style(&msg.role)}>
                                    <div style="font-weight: bold; font-size: 0.8em; opacity: 0.7; margin-bottom: 5px;">{ msg.role.to_uppercase() }</div>
                                    if is_editing {
                                        <textarea value={(*edit_buffer).clone()} oninput={on_edit_input.clone()} style="width: 100%; height: 100px; display: block;" />
                                        <div style="margin-top: 5px; text-align: right;">
                                            <button onclick={on_cancel} style="margin-right: 5px;">{"Cancel"}</button>
                                            <button onclick={Callback::from(move |_| on_save.emit(idx))} style="background: #4caf50; color: white; border: none; padding: 5px 10px;">{"Save"}</button>
                                        </div>
                                    } else {
                                        { render_markdown(&msg.content) }
                                        if is_user && !*is_loading {
                                            <div style="text-align: right; margin-top: 5px;">
                                                <button class="btn-edit" style="color: #666; cursor: pointer; border: none; background: none; font-size: 0.8em; text-decoration: underline;" onclick={Callback::from(move |_| on_open.emit(idx))}>{"Edit"}</button>
                                            </div>
                                        }
                                    }
                                </div>
                            }
                        })
                    }
                    if *is_loading && !*stream_enabled {
                        <div style="color: #888; font-style: italic; margin-left: 20px;">{ "Thinking..." }</div>
                    }
                </div>

                <div style="padding: 20px; background: white; border-top: 1px solid #eee;">
                    <form onsubmit={on_submit_click} style="display: flex; gap: 10px;">
                        <textarea
                            value={(*input_text).clone()}
                            oninput={on_input_text}
                            onkeydown={on_keydown}
                            disabled={*is_loading}
                            style="flex-grow: 1; padding: 10px; height: 50px; font-family: sans-serif; border: 1px solid #ccc; border-radius: 4px;"
                            placeholder="Type a message..."
                        />
                        if *is_loading {
                            <button type="button" onclick={on_stop} style="padding: 0 20px; background-color: #ffcdd2; color: #c62828; border: 1px solid #ef5350; border-radius: 4px; cursor: pointer;">{ "Stop" }</button>
                        } else {
                            <button type="submit" style="padding: 0 20px; background-color: #2196f3; color: white; border: none; border-radius: 4px; cursor: pointer;">{ "Send" }</button>
                        }
                    </form>
                </div>
            </div>
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    utils::set_panic_hook();
    yew::Renderer::<App>::new().render();
}