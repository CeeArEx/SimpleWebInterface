use yew::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, HtmlTextAreaElement, Element};

use crate::models::{Message, AppSettings};
use crate::services::document_service::DocumentService;
use crate::utils::render_markdown;

#[derive(Properties, PartialEq)]
pub struct ChatAreaProps {
    pub messages: Vec<Message>,
    pub is_loading: bool,
    pub on_send: Callback<String>,
    pub on_stop: Callback<()>,
    pub show_metrics: AppSettings,
}

#[function_component(ChatArea)]
pub fn chat_area(props: &ChatAreaProps) -> Html {
    let input_text = use_state(String::new);
    let documents = use_state(|| vec![]);
    let scroll_ref = use_node_ref();

    // Track if the user is currently at the bottom of the chat
    let is_at_bottom = use_state(|| true);

    // @ mention dropdown state
    let mention_position = use_state(|| None::<(i32, i32)>); // Some((x, y)) in viewport coords
    let mention_query = use_state(|| String::new());

    // Auto-scroll effect
    {
        let div_ref = scroll_ref.clone();
        let is_at_bottom_val = *is_at_bottom;
        let last_len = props.messages.last().map(|m| m.content.len()).unwrap_or(0);
        let len = props.messages.len();

        use_effect_with((len, last_len), move |_| {
            if is_at_bottom_val {
                if let Some(div) = div_ref.cast::<HtmlElement>() {
                    div.set_scroll_top(div.scroll_height());
                }
            }
        });
    }

    // Scroll Event Handler
    let on_scroll = {
        let is_at_bottom = is_at_bottom.clone();
        Callback::from(move |e: Event| {
            let div: HtmlElement = e.target_unchecked_into();
            let distance_from_bottom = div.scroll_height() - div.scroll_top() - div.client_height();
            let currently_at_bottom = distance_from_bottom < 35;

            if *is_at_bottom != currently_at_bottom {
                is_at_bottom.set(currently_at_bottom);
            }
        })
    };

    let on_submit = {
        let text = input_text.clone();
        let on_send = props.on_send.clone();
        let is_at_bottom = is_at_bottom.clone();
        let mention_pos = mention_position.clone();
        let mention_q = mention_query.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            if !text.is_empty() {
                // Clear mention state before sending
                mention_pos.set(None);
                mention_q.set(String::new());

                on_send.emit((*text).clone());
                text.set(String::new());
                is_at_bottom.set(true);
            }
        })
    };

    // Load documents on mount
    {
        let docs = documents.clone();
        use_effect_with(() as (), move |_| {
            let loaded_docs = DocumentService::get_documents();
            docs.set(loaded_docs);
        });
    }

    let on_keydown = {
        let text = input_text.clone();
        let on_send = props.on_send.clone();
        let is_at_bottom = is_at_bottom.clone();
        let mention_pos = mention_position.clone();
        let mention_q = mention_query.clone();

        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" && !e.shift_key() {
                e.prevent_default();
                if !text.is_empty() {
                    // Clear mention state before sending
                    mention_pos.set(None);
                    mention_q.set(String::new());

                    on_send.emit((*text).clone());
                    text.set(String::new());
                    is_at_bottom.set(true);
                }
            }
        })
    };

    let on_input = {
        let text = input_text.clone();
        let mention_pos = mention_position.clone();
        let mention_q = mention_query.clone();
        let documents_for_set = documents.clone();

        Callback::from(move |e: InputEvent| {
            let i: HtmlTextAreaElement = e.target_unchecked_into();
            let val = i.value();
            text.set(val.clone());

            // Check for @ mention
            if let Some(pos) = val.rfind('@') {
                let after_at = &val[pos + 1..];

                // Only treat as mention if no whitespace/newline after '@'
                if !after_at.contains(' ') && !after_at.contains('\n') {
                    // Update mention query
                    let query = after_at.to_string();
                    mention_q.set(query.clone());

                    // Refresh docs from localStorage
                    let loaded_docs = DocumentService::get_documents();

                    // Count filtered docs for positioning/height decisions
                    let after_at_lc = after_at.to_lowercase();
                    let filtered_count = loaded_docs
                        .iter()
                        .filter(|d| d.filename.to_lowercase().contains(&after_at_lc))
                        .count();

                    // Update state for rendering (avoid reading localStorage again in render)
                    documents_for_set.set(loaded_docs);

                    // Calculate position for dropdown (viewport coordinates)
                    let element: Element = i.unchecked_into();
                    let rect = Element::get_bounding_client_rect(&element);

                    // Sizing parameters (match your CSS/item layout)
                    let row_h: i32 = 44; // approx item height
                    let padding: i32 = 8;
                    let max_visible_rows: usize = 5;

                    let visible_rows = filtered_count.min(max_visible_rows) as i32;
                    let dropdown_h = (visible_rows * row_h) + padding;

                    let textarea_bottom = (rect.top() + rect.height()) as i32;
                    let x = rect.left() as i32;

                    // - 0/1 result: show below
                    // - >1 results: grow upward
                    let gap: i32 = 5;

                    // % of viewport height
                    let window = web_sys::window().unwrap();
                    let vh = window
                        .inner_height()
                        .ok()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(800.0);

                    let lift_pct: f64 = 0.10; // 10% of viewport height
                    let lift_px: i32 = (vh * lift_pct).round() as i32;

                    let mut y = if filtered_count <= 1 {
                        (textarea_bottom + gap) - lift_px
                    } else {
                        ((textarea_bottom + gap) - dropdown_h) - lift_px
                    };

                    // Clamp to viewport top a bit
                    if y < 5 {
                        y = 5;
                    }

                    mention_pos.set(Some((x, y)));
                    return;
                }
            }

            // Clear mention state if not valid
            mention_pos.set(None);
            mention_q.set(String::new());
        })
    };

    let on_select_document = {
        let text = input_text.clone();
        let mention_pos = mention_position.clone();
        let mention_query_handle = mention_query.clone();

        Callback::from(move |doc_id: String| {
            let current_text = text.to_string();
            if let Some(pos) = current_text.rfind('@') {
                let before_at = current_text[..pos].to_string();

                // IMPORTANT: get the actual query string from the state handle
                let current_query = (*mention_query_handle).clone();

                // Safely slice after query
                let start = pos + 1 + current_query.len();
                let after_query = if start <= current_text.len() {
                    &current_text[start..]
                } else {
                    ""
                };

                let new_text = format!("{}@{}{}", before_at, doc_id, after_query);
                text.set(new_text);

                mention_pos.set(None);
                mention_query_handle.set(String::new());
            }
        })
    };

    let format_metrics = |metrics: &crate::models::MessageMetrics| -> String {
        let mut parts = Vec::new();
        
        // Format usage info (token counts)
        if let Some(usage) = &metrics.usage {
            parts.push(format!("input: {} | output: {} | total: {}",
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.total_tokens
            ));
        }
        
        // Format timing info
        if let Some(timings) = &metrics.timings {
            // Format: prompt: 90ms 243t/s | gen: 472ms 49t/s
            let prompt_time = format!("{}ms", timings.prompt_ms.round() as usize);
            let gen_time = format!("{}ms", timings.predicted_ms.round() as usize);
            let prompt_speed = format!("{}t/s", timings.prompt_per_second.round() as usize);
            let gen_speed = format!("{}t/s", timings.predicted_per_second.round() as usize);
            
            parts.push(format!("prompt: {} {} | gen: {} {}", prompt_time, prompt_speed, gen_time, gen_speed));
        }
        
        // If no usage/timings but we have some metadata, show what we have
        if parts.is_empty() {
            if let Some(created) = metrics.created {
                parts.push(format!("Created: {}", created));
            }
            if let Some(model) = &metrics.model {
                parts.push(format!("Model: {}", model));
            }
            if let Some(id) = &metrics.id {
                parts.push(format!("ID: {}", id));
            }
            if let Some(fingerprint) = &metrics.system_fingerprint {
                parts.push(format!("Fingerprint: {}", fingerprint));
            }
        }
        
        if parts.is_empty() {
            "No metrics available".to_string()
        } else {
            parts.join(" | ")
        }
    };

    let css = r#"
        .messages-container {
            flex-grow: 1;
            overflow-y: auto;
            padding: 20px;
            display: flex;
            flex-direction: column;
            gap: 15px;
            background-color: #ffffff;
            scroll-behavior: smooth;
        }

        /* Row Layout */
        .message-row { display: flex; width: 100%; }
        .message-row.user { justify-content: flex-end; }
        .message-row.assistant { justify-content: flex-start; }
        .message-row.system { justify-content: center; margin: 10px 0; }

        /* Bubble Container */
        .bubble-group { display: flex; gap: 10px; max-width: 85%; align-items: flex-end; }
        .message-row.user .bubble-group { flex-direction: row-reverse; }

        /* Avatars */
        .avatar { width: 32px; height: 32px; border-radius: 50%; display: flex; align-items: center; justify-content: center; flex-shrink: 0; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .avatar.user { background: #555; color: white; }
        .avatar.assistant { background: var(--accent-color); color: white; }

        /* Text Bubble */
        .msg-bubble {
            padding: 10px 15px;
            border-radius: 12px;
            font-size: 0.95rem;
            line-height: 1.5;
            box-shadow: 0 1px 2px rgba(0,0,0,0.05);
            min-width: 0;
            overflow-wrap: anywhere;
            word-break: break-word;
            max-width: 100%;
        }

        .message-row.user .msg-bubble { background-color: #e3f2fd; color: #1565c0; border-bottom-right-radius: 2px; }
        .message-row.assistant .msg-bubble { background-color: #f5f5f5; color: #333; border-bottom-left-radius: 2px; }

        /* SYSTEM MESSAGE STYLE */
        .system-bubble {
            background-color: #fff3cd;
            color: #666;
            padding: 8px 16px;
            border-radius: 20px;
            font-size: 0.85em;
            border: 1px dashed #ccc;
            text-align: center;
            max-width: 90%;
            overflow-wrap: anywhere;
        }

        /* Metrics Display */
        .msg-metrics {
            margin-top: 6px;
            padding: 6px 12px;
            background-color: #f0f0f0;
            border-radius: 6px;
            font-size: 0.75rem;
            color: #666;
            font-family: monospace;
            max-width: 85%;
            display: flex;
            gap: 10px;
            flex-wrap: wrap;
        }

        .msg-metrics span {
            display: inline-flex;
            align-items: center;
            gap: 4px;
        }

        /* Input Area Styles */
        .input-wrapper { border-top: 1px solid var(--border-color); padding: 20px; display: flex; justify-content: center; background: white; position: relative; }
        .input-container { width: 100%; max-width: 900px; position: relative; display: flex; flex-direction: column; }
        .chat-input { width: 100%; padding: 12px; padding-right: 45px; border: 1px solid var(--border-color); border-radius: 8px; box-shadow: 0 2px 5px rgba(0,0,0,0.05); resize: none; font-family: inherit; outline: none; transition: border 0.2s; }
        .chat-input:focus { border-color: var(--accent-color); box-shadow: 0 0 0 2px rgba(16, 163, 127, 0.1); }
        .send-btn { position: absolute; right: 8px; bottom: 8px; background: var(--accent-color); color: white; border: none; border-radius: 4px; padding: 6px 10px; cursor: pointer; transition: opacity 0.2s; }
        .send-btn:disabled { background: #ccc; cursor: default; }
        .send-btn:hover:not(:disabled) { background: var(--accent-hover); }

        /* Document Mention Dropdown */
        .document-mention-dropdown {
            position: fixed; /* IMPORTANT: use viewport coordinates */
            background: white;
            border: 1px solid var(--border-color);
            border-radius: 8px;
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
            overflow: hidden;
            width: 250px;
            z-index: 100;
        }
        .document-mention-dropdown.scrollable {
            max-height: 228px;
            overflow-y: auto;
        }
        .document-mention-dropdown.no-scrollbar {
            max-height: 228px;
            overflow-y: hidden;
        }
        .document-mention-item {
            padding: 10px 12px;
            cursor: pointer;
            display: flex;
            flex-direction: column;
            gap: 4px;
            border-bottom: 1px solid #f0f0f0;
        }
        .document-mention-item:last-child { border-bottom: none; }
        .document-mention-item:hover { background: #f5f5f5; }
        .document-mention-name {
            font-size: 0.9rem;
            font-weight: 500;
            color: var(--text-primary);
        }
        .document-mention-meta {
            font-size: 0.75rem;
            color: var(--text-secondary);
        }
        .document-mention-no-results {
            padding: 10px 12px;
            color: var(--text-secondary);
            text-align: center;
        }
    "#;

    let user_icon = html! {
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
            <circle cx="12" cy="7" r="4"></circle>
        </svg>
    };
    let bot_icon = html! {
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="11" width="18" height="10" rx="2"></rect>
            <circle cx="12" cy="5" r="2"></circle>
            <path d="M12 7v4"></path>
            <line x1="8" y1="16" x2="8" y2="16"></line>
            <line x1="16" y1="16" x2="16" y2="16"></line>
        </svg>
    };

    // Document mention dropdown (use documents state, not localStorage each render)
    let mention_dropdown = {
        let mention_pos = *mention_position;
        let query = (*mention_query).clone();
        let docs = (*documents).clone();
        let on_select_document = on_select_document.clone();

        if let Some((x, y)) = mention_pos {
            let query_lc = query.to_lowercase();
            let filtered_docs: Vec<_> = docs
                .iter()
                .filter(|d| d.filename.to_lowercase().contains(&query_lc))
                .collect();

            let style_val = format!("left: {}px; top: {}px;", x, y);

            if !filtered_docs.is_empty() {
                let scrollbar_class = if filtered_docs.len() > 5 { "scrollable" } else { "no-scrollbar" };

                html! {
                    <div class={format!("document-mention-dropdown {}", scrollbar_class)} style={style_val}>
                        { for filtered_docs.iter().map(|doc| {
                            let doc_id = doc.id.clone();
                            let doc_name = doc.filename.clone();
                            let chunk_count = doc.chunk_count;
                            let on_select = on_select_document.clone();

                            html! {
                                <div
                                    class="document-mention-item"
                                    onclick={Callback::from(move |_| on_select.emit(doc_id.clone()))}
                                >
                                    <div class="document-mention-name">{ &doc_name }</div>
                                    <div class="document-mention-meta">{ format!("{} chunks", chunk_count) }</div>
                                </div>
                            }
                        }) }
                    </div>
                }
            } else {
                html! {
                    <div class="document-mention-dropdown no-scrollbar" style={style_val}>
                        <div class="document-mention-no-results">{ "No documents found" }</div>
                    </div>
                }
            }
        } else {
            html! { <></> }
        }
    };

    html! {
        <>
            <style>{ css }</style>

            <div class="messages-container" ref={scroll_ref} onscroll={on_scroll}>
                { for props.messages.iter().map(|msg| {
                    if msg.role == "system" {
                        html! {
                            <div class="message-row system">
                                <div class="system-bubble">{ &msg.content }</div>
                            </div>
                        }
                    } else {
                        let role_cls = msg.role.clone();
                        let (avatar_cls, icon) = if msg.role == "user" {
                            ("user", user_icon.clone())
                        } else {
                            ("assistant", bot_icon.clone())
                        };

                        let metrics_html = if props.show_metrics.show_metrics && msg.role == "assistant" && !msg.metrics.is_empty() {
                            let metrics_text = format_metrics(&msg.metrics);
                            html! { <div class="msg-metrics">{ metrics_text }</div> }
                        } else {
                            html! { <></> }
                        };

                        html! {
                            <div class={format!("message-row {}", role_cls)}>
                                <div class="bubble-group">
                                    <div class={format!("avatar {}", avatar_cls)}>{ icon }</div>
                                    <div>
                                        <div class="msg-bubble">{ render_markdown(&msg.content) }</div>
                                        { metrics_html }
                                    </div>
                                </div>
                            </div>
                        }
                    }
                })}

                if props.is_loading {
                    <div class="message-row assistant">
                        <div class="bubble-group">
                            <div class="avatar assistant">{ bot_icon.clone() }</div>
                            <div class="msg-bubble" style="color: #888; font-style: italic;">
                                { "Thinking..." }
                            </div>
                        </div>
                    </div>
                }
            </div>

            <div class="input-wrapper">
                <form class="input-container" onsubmit={on_submit}>
                    <textarea
                        class="chat-input"
                        rows="1"
                        placeholder="Message Local LLM..."
                        value={(*input_text).clone()}
                        oninput={on_input}
                        onkeydown={on_keydown}
                        disabled={props.is_loading}
                        style="height: 50px; overflow-y: hidden;"
                    />
                    { mention_dropdown }

                    if props.is_loading {
                        <button
                            type="button"
                            class="send-btn"
                            style="background: var(--danger-color);"
                            onclick={props.on_stop.reform(|_| ())}
                        >
                            { "Stop" }
                        </button>
                    } else {
                        <button type="submit" class="send-btn" disabled={input_text.is_empty()}>
                            { "Send" }
                        </button>
                    }
                </form>
            </div>
        </>
    }
}