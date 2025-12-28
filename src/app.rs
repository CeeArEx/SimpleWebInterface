use yew::prelude::*;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use futures_util::StreamExt;
use wasm_bindgen_futures::spawn_local;

use crate::models::*;
use crate::services::{storage::LocalStorage, llm::LlmService};
use crate::components::{sidebar::Sidebar, settings::SettingsModal, chat_area::ChatArea};

const KEY_CHATS: &str = "llm_chats_v2";
const KEY_SETTINGS: &str = "chat_settings_v1";

// --- CSS STYLES ---
const GLOBAL_STYLES: &str = r#"
    :root {
        --bg-app: #ffffff;
        --bg-sidebar: #f9f9f9;
        --bg-user: #f4f4f4;
        --bg-assistant: #ffffff;
        --border-color: #e5e5e5;
        --text-primary: #333;
        --text-secondary: #666;
        --accent-color: #10a37f; /* ChatGPT Green */
        --accent-hover: #1a7f64;
        --danger-color: #ef4444;
    }

    * { box-sizing: border-box; }
    body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; color: var(--text-primary); }

    /* Layout */
    .app-container { display: flex; height: 100vh; overflow: hidden; }
    .main-content { flex-grow: 1; display: flex; flex-direction: column; position: relative; background: var(--bg-app); }
    .header { padding: 10px 20px; border-bottom: 1px solid var(--border-color); display: flex; justify-content: space-between; align-items: center; height: 60px; }
    .header h2 { font-size: 1rem; margin: 0; font-weight: 600; }

    /* Buttons */
    .btn { cursor: pointer; border: 1px solid var(--border-color); background: white; padding: 8px 12px; border-radius: 6px; font-size: 0.9rem; transition: all 0.2s; color: var(--text-primary); }
    .btn:hover { background: #f0f0f0; }
    .btn-primary { background: var(--accent-color); color: white; border-color: transparent; }
    .btn-primary:hover { background: var(--accent-hover); }
    .btn-danger { color: var(--danger-color); border-color: var(--danger-color); }
    .btn-danger:hover { background: #fef2f2; }
    .btn-icon { border: none; background: transparent; font-size: 1.2rem; padding: 5px; color: var(--text-secondary); }
    .btn-icon:hover { background: rgba(0,0,0,0.05); color: var(--text-primary); }

    /* Inputs */
    .form-input, .form-select, .form-textarea { width: 100%; padding: 8px; border: 1px solid var(--border-color); border-radius: 6px; font-family: inherit; margin-bottom: 10px; }
    .form-input:focus, .form-textarea:focus { outline: 2px solid var(--accent-color); border-color: transparent; }

    /* Markdown */
    .markdown-body { line-height: 1.6; font-size: 1rem; }
    .markdown-body pre { background: #2d2d2d; color: #fff; padding: 15px; border-radius: 6px; overflow-x: auto; }
    .markdown-body code { background: #f4f4f4; padding: 2px 4px; border-radius: 4px; font-family: monospace; font-size: 0.9em; }
    .markdown-body pre code { background: transparent; color: inherit; }
    .markdown-body p { margin-top: 0; margin-bottom: 1em; }
"#;

#[function_component(App)]
pub fn app() -> Html {
    // --- STATE SETUP (Same as before) ---
    let settings = use_state(|| LocalStorage::get::<AppSettings>(KEY_SETTINGS).unwrap_or_default());
    let chats = use_state(|| LocalStorage::get::<Vec<ChatSession>>(KEY_CHATS).unwrap_or_else(|| {
        vec![ChatSession::new("You are a helpful assistant".to_string())]
    }));
    let active_chat_id = use_state(|| chats.first().map(|c| c.id.clone()).unwrap_or_default());

    let sidebar_open = use_state(|| true);
    let show_settings = use_state(|| false);
    let is_loading = use_state(|| false);
    let cancellation_token = use_state(|| Arc::new(AtomicBool::new(false)));

    let current_chat = chats.iter().find(|c| c.id == *active_chat_id);
    let current_messages = current_chat.map(|c| c.messages.clone()).unwrap_or_default();

    // --- EFFECTS & HANDLERS (Same logic, compacted for brevity) ---
    {
        let chats = chats.clone();
        use_effect_with(chats, |c| LocalStorage::set(KEY_CHATS, &**c));
    }
    {
        let s = settings.clone();
        use_effect_with(s, |s| LocalStorage::set(KEY_SETTINGS, &**s));
    }

    let on_new_chat = {
        let chats = chats.clone();
        let active = active_chat_id.clone();
        let sys = settings.system_prompt.clone();
        Callback::from(move |_| {
            let new_chat = ChatSession::new(sys.clone());
            let mut curr = (*chats).clone();
            curr.insert(0, new_chat.clone());
            chats.set(curr);
            active.set(new_chat.id);
        })
    };

    let on_delete_chat = {
        let chats = chats.clone();
        Callback::from(move |(e, id): (MouseEvent, String)| {
            e.stop_propagation();
            let mut curr = (*chats).clone();
            curr.retain(|c| c.id != id);
            chats.set(curr);
        })
    };

    let run_chat = {
        let chats = chats.clone();
        let active_id = active_chat_id.clone();
        let loading = is_loading.clone();
        let settings = settings.clone();
        let token = cancellation_token.clone();

        Callback::from(move |msg_content: String| {
            let current_id = (*active_id).clone();
            loading.set(true);
            token.store(false, Ordering::Relaxed);

            let mut history = chats.iter().find(|c| c.id == current_id).map(|c| c.messages.clone()).unwrap_or_default();
            history.push(Message { role: "user".into(), content: msg_content });

            let mut all_chats = (*chats).clone();
            if let Some(c) = all_chats.iter_mut().find(|c| c.id == current_id) { c.messages = history.clone(); }
            chats.set(all_chats);

            let chats_state = chats.clone();
            let loading_state = loading.clone();
            let set = settings.clone();
            let cancel = token.clone();
            let cid = current_id.clone();

            spawn_local(async move {
                let req = ChatRequest {
                    messages: history.clone(),
                    model: set.selected_model.clone(),
                    temperature: 0.7,
                    stream: set.stream_enabled,
                };

                let update = |msgs: Vec<Message>| {
                    let mut all = (*chats_state).clone();
                    if let Some(c) = all.iter_mut().find(|c| c.id == cid) { c.messages = msgs; }
                    chats_state.set(all);
                };

                if let Ok(resp) = LlmService::chat_completion_request(&set.base_url, &req).await {
                    if set.stream_enabled {
                        history.push(Message { role: "assistant".into(), content: "".into() });
                        update(history.clone());
                        let mut stream = resp.bytes_stream();
                        let mut buffer = String::new();
                        while let Some(item) = stream.next().await {
                            if cancel.load(Ordering::Relaxed) { break; }
                            if let Ok(chunk) = item {
                                buffer.push_str(&String::from_utf8_lossy(&chunk));
                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer.drain(..pos+1);
                                    if line.starts_with("data: ") && line != "data: [DONE]" {
                                        if let Ok(json) = serde_json::from_str::<StreamResponse>(&line[6..]) {
                                            if let Some(txt) = json.choices[0].delta.content.as_ref() {
                                                if let Some(last) = history.last_mut() { last.content.push_str(txt); }
                                                update(history.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        if let Ok(json) = resp.json::<ChatResponse>().await {
                            if let Some(choice) = json.choices.first() {
                                history.push(choice.message.clone());
                                update(history);
                            }
                        }
                    }
                }
                loading_state.set(false);
            });
        })
    };

    let on_stop = {
        let token = cancellation_token.clone();
        let loading = is_loading.clone();
        Callback::from(move |_| {
            token.store(true, Ordering::Relaxed);
            loading.set(false);
        })
    };

    let on_settings_save = {
        let s = settings.clone();
        // Capture chats and active_chat_id to allow creating a new chat
        let chats = chats.clone();
        let active = active_chat_id.clone();

        Callback::from(move |(new_sys, url, model, stream): (String, String, String, bool)| {
            // 1. Check if the system prompt has actually changed
            let prompt_changed = new_sys != s.system_prompt;

            // 2. Update Settings
            s.set(AppSettings {
                system_prompt: new_sys.clone(), // Use the new value
                base_url: url,
                selected_model: model,
                stream_enabled: stream
            });

            // 3. If prompt changed, trigger New Chat logic
            if prompt_changed {
                let new_chat = ChatSession::new(new_sys);
                let mut curr = (*chats).clone();
                // Add to top
                curr.insert(0, new_chat.clone());
                chats.set(curr);
                // Switch to it
                active.set(new_chat.id);
            }
        })
    };

    let close_settings = {
        let show_settings = show_settings.clone();
        Callback::from(move |_| show_settings.set(false))
    };

    // 1. Logic to Reset Settings
    let on_reset_settings = {
        let settings = settings.clone();
        Callback::from(move |_| {
            if web_sys::window().unwrap().confirm_with_message("Reset all settings to default?").unwrap_or(false) {
                settings.set(AppSettings::default());
            }
        })
    };

    // 2. Logic to Clear All Chats
    let on_clear_all_chats = {
        let chats = chats.clone();
        let active_chat_id = active_chat_id.clone();
        let settings = settings.clone();
        Callback::from(move |_| {
            if web_sys::window().unwrap().confirm_with_message("Irreversibly delete ALL chat history?").unwrap_or(false) {
                // We must create at least one new empty chat
                let new_chat = ChatSession::new(settings.system_prompt.clone());
                chats.set(vec![new_chat.clone()]);
                active_chat_id.set(new_chat.id);
            }
        })
    };

    let toggle_settings = show_settings.clone();
    let toggle_sidebar = sidebar_open.clone();

    html! {
        <>
            <style>{ GLOBAL_STYLES }</style>
            <div class="app-container">
                <Sidebar
                    open={*sidebar_open}
                    chats={(*chats).clone()}
                    active_chat_id={(*active_chat_id).clone()}
                    on_select={Callback::from(move |id| active_chat_id.set(id))}
                    on_new={on_new_chat}
                    on_delete={on_delete_chat}
                />

                <div class="main-content">
                    <div class="header">
                        <div style="display: flex; gap: 10px; align-items: center;">
                            <button class="btn-icon" onclick={Callback::from(move |_| toggle_sidebar.set(!*toggle_sidebar))} title="Toggle Menu">
                                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="3" y1="12" x2="21" y2="12"></line><line x1="3" y1="6" x2="21" y2="6"></line><line x1="3" y1="18" x2="21" y2="18"></line></svg>
                            </button>
                            <h2>{ if let Some(c) = &current_chat { &c.title } else { "Local LLM" } }</h2>
                        </div>
                        <button class="btn-icon" onclick={Callback::from(move |_| toggle_settings.set(!*toggle_settings))} title="Settings">
                            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
                        </button>
                    </div>

                    if *show_settings {
                        <SettingsModal
                            system_prompt={settings.system_prompt.clone()}
                            base_url={settings.base_url.clone()}
                            selected_model={settings.selected_model.clone()}
                            stream_enabled={settings.stream_enabled}
                            on_save={on_settings_save}
                            on_close={close_settings}
                            on_reset={on_reset_settings}
                            on_clear_chats={on_clear_all_chats}
                        />
                    }

                    <ChatArea
                        messages={current_messages}
                        is_loading={*is_loading}
                        on_send={run_chat}
                        on_stop={on_stop}
                    />
                </div>
            </div>
        </>
    }
}