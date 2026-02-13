use yew::prelude::*;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use futures_util::StreamExt;
use wasm_bindgen_futures::spawn_local;

use crate::models::*;
use crate::services::{storage::LocalStorage, llm::LlmService, document_service::DocumentService};
use crate::components::{sidebar::Sidebar, settings::SettingsModal, chat_area::ChatArea};

const KEY_CHATS: &str = "llm_chats_v2";
const KEY_SETTINGS: &str = "chat_settings_v1";

const GLOBAL_STYLES: &str = r#"
    :root {
        --bg-app: #ffffff;
        --bg-sidebar: #f9f9f9;
        --bg-user: #f4f4f4;
        --bg-assistant: #ffffff;
        --border-color: #e5e5e5;
        --text-primary: #333;
        --text-secondary: #666;
        --accent-color: #10a37f;
        --accent-hover: #1a7f64;
        --danger-color: #ef4444;
    }

    * { box-sizing: border-box; }
    body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; color: var(--text-primary); }

    .app-container { display: flex; height: 100vh; overflow: hidden; }
    .main-content { flex-grow: 1; display: flex; flex-direction: column; position: relative; background: var(--bg-app); }
    .header { padding: 10px 20px; border-bottom: 1px solid var(--border-color); display: flex; justify-content: space-between; align-items: center; height: 60px; }
    .header h2 { font-size: 1rem; margin: 0; font-weight: 600; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; max-width: 500px; }

    .btn { cursor: pointer; border: 1px solid var(--border-color); background: white; padding: 8px 12px; border-radius: 6px; font-size: 0.9rem; transition: all 0.2s; color: var(--text-primary); }
    .btn:hover { background: #f0f0f0; }
    .btn-primary { background: var(--accent-color); color: white; border-color: transparent; }
    .btn-primary:hover { background: var(--accent-hover); }
    .btn-danger { color: var(--danger-color); border-color: var(--danger-color); }
    .btn-danger:hover { background: #fef2f2; }
    .btn-icon { border: none; background: transparent; font-size: 1.2rem; padding: 5px; color: var(--text-secondary); }
    .btn-icon:hover { background: rgba(0,0,0,0.05); color: var(--text-primary); }

    .form-input, .form-select, .form-textarea { width: 100%; padding: 8px; border: 1px solid var(--border-color); border-radius: 6px; font-family: inherit; margin-bottom: 10px; }
    .form-input:focus, .form-textarea:focus { outline: 2px solid var(--accent-color); border-color: transparent; }

    .markdown-body { line-height: 1.6; font-size: 1rem; }
    .markdown-body pre { background: #2d2d2d; color: #fff; padding: 15px; border-radius: 6px; overflow-x: auto; }
    .markdown-body code { background: #f4f4f4; padding: 2px 4px; border-radius: 4px; font-family: monospace; font-size: 0.9em; }
    .markdown-body pre code { background: transparent; color: inherit; }
    .markdown-body p { margin-top: 0; margin-bottom: 1em; }
"#;

#[function_component(App)]
pub fn app() -> Html {
    let settings = use_state(|| LocalStorage::get::<AppSettings>(KEY_SETTINGS).unwrap_or_default());
    let chats = use_state(|| LocalStorage::get::<Vec<ChatSession>>(KEY_CHATS).unwrap_or_else(|| {
        vec![ChatSession::new("You are a helpful assistant".to_string())]
    }));
    let active_chat_id = use_state(|| chats.first().map(|c| c.id.clone()).unwrap_or_default());

    let sidebar_open = use_state(|| true);
    let show_settings = use_state(|| false);
    let is_loading = use_state(|| false);
    let cancellation_token = use_state(|| Arc::new(AtomicBool::new(false)));
    let available_models = use_state(Vec::new);

    let current_chat = chats.iter().find(|c| c.id == *active_chat_id);
    let current_messages = current_chat.map(|c| c.messages.clone()).unwrap_or_default();

    // --- EFFECTS ---

    // Fetch models on startup if base_url is not default
    {
        let models = available_models.clone();
        let settings = settings.clone();
        use_effect_with(settings.clone(), move |settings_ref| {
            let base_url = settings_ref.base_url.clone();
            if base_url != "http://localhost:8080" {
                let url = base_url.clone();
                let models = models.clone();
                let settings = settings.clone();
                spawn_local(async move {
                    match LlmService::fetch_models(&url).await {
                        Ok(resp) => {
                            let model_list: Vec<String> = resp.data.into_iter().map(|m| m.id).collect();
                            models.set(model_list.clone());
                            // If the saved model exists in the list, keep it; otherwise use the first one
                            let current_settings: AppSettings = (*settings).clone();
                            let saved_model = current_settings.selected_model.clone();
                            if model_list.contains(&saved_model) {
                                // Keep the saved model
                            } else if let Some(first_model) = model_list.first().cloned() {
                                // Update settings with the first available model
                                let mut new_settings = current_settings.clone();
                                new_settings.selected_model = first_model;
                                settings.set(new_settings);
                            }
                        }
                        Err(_) => {
                            // If fetch fails, keep using the saved model
                        }
                    }
                });
            }
        });
    }

    // --- EFFECTS ---
    {
        let chats = chats.clone();
        use_effect_with(chats, |c| LocalStorage::set(KEY_CHATS, &**c));
    }
    {
        let s = settings.clone();
        use_effect_with(s, |s| LocalStorage::set(KEY_SETTINGS, &**s));
    }

    // --- ACTIONS ---

    let on_new_chat = {
        let chats = chats.clone();
        let active_id = active_chat_id.clone();
        let sys = settings.system_prompt.clone();
        Callback::from(move |_| {
            let current_id = (*active_id).clone();
            let mut current_list = (*chats).clone();

            let current_is_empty = if let Some(curr) = current_list.iter().find(|c| c.id == current_id) {
                curr.messages.len() == 1 && curr.messages[0].role == "system"
            } else {
                false
            };

            if current_is_empty {
                return;
            }

            let new_chat = ChatSession::new(sys.clone());
            current_list.insert(0, new_chat.clone());
            chats.set(current_list);
            active_id.set(new_chat.id);
        })
    };

    let on_select_chat = {
        let chats = chats.clone();
        let active_id = active_chat_id.clone();
        Callback::from(move |target_id: String| {
            let current_id = (*active_id).clone();
            if current_id == target_id { return; }

            let mut list = (*chats).clone();
            let should_delete_prev = if let Some(prev) = list.iter().find(|c| c.id == current_id) {
                prev.messages.len() == 1 && prev.messages[0].role == "system"
            } else {
                false
            };

            if should_delete_prev {
                list.retain(|c| c.id != current_id);
            }

            chats.set(list);
            active_id.set(target_id);
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

    let on_settings_save = {
        let s = settings.clone();
        let chats = chats.clone();
        let active = active_chat_id.clone();

        Callback::from(move |new_settings: AppSettings| {
            let prompt_changed = new_settings.system_prompt != s.system_prompt;
            s.set(new_settings.clone());

            if prompt_changed {
                let current_id = (*active).clone();
                let mut list = (*chats).clone();
                let mut handled = false;
                if let Some(curr) = list.iter_mut().find(|c| c.id == current_id) {
                    if curr.messages.len() == 1 && curr.messages[0].role == "system" {
                        curr.messages[0].content = new_settings.system_prompt.clone();
                        handled = true;
                    }
                }
                if handled {
                    chats.set(list);
                } else {
                    let new_chat = ChatSession::new(new_settings.system_prompt);
                    list.insert(0, new_chat.clone());
                    chats.set(list);
                    active.set(new_chat.id);
                }
            }
        })
    };

    // --- MAIN CHAT LOGIC ---
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
            history.push(Message { role: "user".into(), content: msg_content.clone(), metrics: MessageMetrics::default() });

            // 1. Calculate Title if needed
            let mut new_title_opt = None;
            if history.len() == 2 {
                let first_line = msg_content.lines().next().unwrap_or("New Chat");
                let mut t: String = first_line.chars().take(40).collect();
                if first_line.chars().count() > 40 { t.push_str("..."); }
                new_title_opt = Some(t);
            }

            // 2. Update Immediate UI (so user sees it instantly)
            let mut all_chats = (*chats).clone();
            if let Some(c) = all_chats.iter_mut().find(|c| c.id == current_id) {
                if let Some(t) = &new_title_opt {
                    c.title = t.clone();
                }
                c.messages = history.clone();
            }
            chats.set(all_chats);

            // 3. Prepare for Async
            let chats_state = chats.clone();
            let loading_state = loading.clone();
            let set = settings.clone();
            let cancel = token.clone();
            let cid = current_id.clone();
            let title_override = new_title_opt.clone(); // <--- Pass the new title into the async block

            // Spawn async task with document context
            spawn_local(async move {
                // Get document context based on mode
                let service = DocumentService::default();
                
                // For manual mode, we need both the context for LLM and clean display
                let (doc_context, display_message) = if set.document_context_mode == DocumentContextMode::Manual {
                    service.build_manual_context_with_display(&msg_content).await
                } else {
                    // For RAG mode, use the original context builder
                    let ctx = service.build_context(&msg_content, 3).await;
                    (ctx.clone(), ctx + "User message:\n" + &msg_content)
                };

                // Update history with the display message (clean version)
                if let Some(last_msg) = history.last_mut() {
                    if last_msg.role == "user" {
                        last_msg.content = display_message.clone();
                    }
                }

                // Create messages with full context for LLM
                let mut llm_messages = history.clone();
                if !doc_context.is_empty() {
                    // For LLM, prepend document context and keep original message
                    let llm_user_content = format!("{}User message:\n{}", doc_context, msg_content);
                    llm_messages.pop();
                    llm_messages.push(Message {
                        role: "user".to_string(),
                        content: llm_user_content,
                        metrics: MessageMetrics::default()
                    });
                }

                let req = ChatRequest {
                    messages: llm_messages,
                    model: "/root/models/Strand-Rust-Coder-14B-v1".to_string(),//set.selected_model.clone(),
                    temperature: 0.7,
                    stream: set.stream_enabled,
                };

                // Define update closure that preserves the title
                let update = move |msgs: Vec<Message>| {
                    let mut all = (*chats_state).clone(); // <--- This handle might still hold the old "New Chat" title
                    if let Some(c) = all.iter_mut().find(|c| c.id == cid) {
                        c.messages = msgs;
                        // FORCE the title back if we changed it in this session
                        if let Some(t) = &title_override {
                            c.title = t.clone();
                        }
                    }
                    chats_state.set(all);
                };

                if let Ok(resp) = LlmService::chat_completion_request(&set.base_url, &req).await {
                    if set.stream_enabled {
                        // For streaming, we need to capture metrics from the final response
                        // We'll use a wrapper response that also contains timing info
                        let mut stream = resp.bytes_stream();
                        let mut buffer = String::new();
                        let mut final_usage: Option<UsageInfo> = None;
                        let mut final_timings: Option<TimingsInfo> = None;
                        let mut final_id: Option<String> = None;
                        let mut final_created: Option<i64> = None;
                        let mut final_model: Option<String> = None;
                        let mut final_fingerprint: Option<String> = None;
                        
                        history.push(Message { role: "assistant".into(), content: "".into(), metrics: MessageMetrics::default() });
                        update(history.clone());
                        
                        while let Some(item) = stream.next().await {
                            if cancel.load(Ordering::Relaxed) { break; }
                            if let Ok(chunk) = item {
                                buffer.push_str(&String::from_utf8_lossy(&chunk));
                                while let Some(pos) = buffer.find('\n') {
                                    let line = buffer[..pos].trim().to_string();
                                    buffer.drain(..pos+1);
                                    if line.starts_with("data: ") && line != "data: [DONE]" {
                                        if let Ok(json) = serde_json::from_str::<StreamResponse>(&line[6..]) {
                                            // Extract timing info if available in chunk
                                            final_id = json.id.clone();
                                            final_created = json.created;
                                            final_model = json.model.clone();
                                            final_fingerprint = json.system_fingerprint.clone();
                                            
                                            // Update usage and timings if present in the chunk
                                            if let Some(usage) = json.usage {
                                                final_usage = Some(usage);
                                            }
                                            if let Some(timings) = json.timings {
                                                final_timings = Some(timings);
                                            }
                                            
                                            if let Some(txt) = json.choices[0].delta.content.as_ref() {
                                                if let Some(last) = history.last_mut() { 
                                                    last.content.push_str(txt); 
                                                }
                                                update(history.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Store the metrics we captured during streaming
                        if let Some(last) = history.last_mut() {
                            last.metrics = MessageMetrics {
                                usage: final_usage,
                                timings: final_timings,
                                created: final_created,
                                id: final_id,
                                model: final_model,
                                system_fingerprint: final_fingerprint,
                            };
                        }
                        // Update the UI with the final metrics
                        update(history.clone());
                        
                        // Some APIs (like Ollama) return usage in a final non-data chunk after streaming
                        // Let's try to parse any remaining content in the buffer
                        if !buffer.is_empty() {
                            if let Ok(json) = serde_json::from_str::<StreamResponse>(&buffer) {
                                if let Some(usage) = json.usage {
                                    if let Some(last) = history.last_mut() {
                                        last.metrics.usage = Some(usage);
                                    }
                                }
                                if let Some(timings) = json.timings {
                                    if let Some(last) = history.last_mut() {
                                        last.metrics.timings = Some(timings);
                                    }
                                }
                                // Update if we got new data
                                update(history.clone());
                            }
                        }
                    } else {
                        // Non-streaming: extract all metrics from the full response
                        if let Ok(json) = resp.json::<ChatResponse>().await {
                            if let Some(choice) = json.choices.first() {
                                let metrics = MessageMetrics {
                                    usage: json.usage.clone(),
                                    timings: json.timings.clone(),
                                    created: json.created,
                                    id: json.id.clone(),
                                    model: json.model.clone(),
                                    system_fingerprint: json.system_fingerprint.clone(),
                                };
                                let mut msg = choice.message.clone();
                                msg.metrics = metrics;
                                history.push(msg);
                                update(history);
                            }
                        }
                    }
                }
                loading_state.set(false);
            });
        })
    };
    // -------------------------

    let on_stop = {
        let token = cancellation_token.clone();
        let loading = is_loading.clone();
        Callback::from(move |_| {
            token.store(true, Ordering::Relaxed);
            loading.set(false);
        })
    };

    let on_reset_settings = {
        let settings = settings.clone();
        Callback::from(move |_| {
            if web_sys::window().unwrap().confirm_with_message("Reset all settings to default?").unwrap_or(false) {
                settings.set(AppSettings::default());
            }
        })
    };

    let on_clear_all_chats = {
        let chats = chats.clone();
        let active_chat_id = active_chat_id.clone();
        let settings = settings.clone();
        Callback::from(move |_| {
            if web_sys::window().unwrap().confirm_with_message("Irreversibly delete ALL chat history?").unwrap_or(false) {
                let new_chat = ChatSession::new(settings.system_prompt.clone());
                chats.set(vec![new_chat.clone()]);
                active_chat_id.set(new_chat.id);
            }
        })
    };

    let close_settings = {
        let show_settings = show_settings.clone();
        Callback::from(move |_| show_settings.set(false))
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
                    on_select={on_select_chat}
                    on_new={on_new_chat}
                    on_delete={on_delete_chat}
                />

                <div class="main-content">
                    <div class="header">
                        <div style="display: flex; gap: 10px; align-items: center; min-width: 0;">
                            <button class="btn-icon" onclick={Callback::from(move |_| toggle_sidebar.set(!*toggle_sidebar))} title="Toggle Menu">
                                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="3" y1="12" x2="21" y2="12"></line><line x1="3" y1="6" x2="21" y2="6"></line><line x1="3" y1="18" x2="21" y2="18"></line></svg>
                            </button>
                            <h2>{ if let Some(c) = &current_chat { &c.title } else { "Local LLM" } }</h2>
                        </div>
                        <button class="btn-icon" onclick={Callback::from(move |_| toggle_settings.set(!*toggle_settings))} title="Settings">
                            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06-.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06-.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06-.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
                        </button>
                    </div>

                    if *show_settings {
                        <SettingsModal
                            settings={(*settings).clone()}
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
                        show_metrics={(*settings).clone()}
                    />
                </div>
            </div>
        </>
    }
}