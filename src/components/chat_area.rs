use yew::prelude::*;
use web_sys::{HtmlTextAreaElement, HtmlElement};
use crate::models::Message;
use crate::utils::render_markdown;

#[derive(Properties, PartialEq)]
pub struct ChatAreaProps {
    pub messages: Vec<Message>,
    pub is_loading: bool,
    pub on_send: Callback<String>,
    pub on_stop: Callback<()>,
}

#[function_component(ChatArea)]
pub fn chat_area(props: &ChatAreaProps) -> Html {
    let input_text = use_state(String::new);
    let scroll_ref = use_node_ref();

    // Auto-scroll effect
    {
        let div_ref = scroll_ref.clone();
        let len = props.messages.len();
        use_effect_with(len, move |_| {
            if let Some(div) = div_ref.cast::<HtmlElement>() {
                div.set_scroll_top(div.scroll_height());
            }
        });
    }

    let on_submit = {
        let text = input_text.clone();
        let on_send = props.on_send.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            if !text.is_empty() {
                on_send.emit((*text).clone());
                text.set(String::new());
            }
        })
    };

    let on_input = {
        let text = input_text.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlTextAreaElement = e.target_unchecked_into();
            text.set(i.value());
        })
    };

    let on_keydown = {
        let text = input_text.clone();
        let on_send = props.on_send.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" && !e.shift_key() {
                e.prevent_default();
                if !text.is_empty() {
                    on_send.emit((*text).clone());
                    text.set(String::new());
                }
            }
        })
    };

    let css = r#"
        .messages-container { flex-grow: 1; overflow-y: auto; padding: 20px; display: flex; flex-direction: column; gap: 15px; background-color: #ffffff; }

        /* Row Layout */
        .message-row { display: flex; width: 100%; }
        .message-row.user { justify-content: flex-end; }
        .message-row.assistant { justify-content: flex-start; }
        .message-row.system { justify-content: center; margin: 10px 0; }

        /* Bubble Container */
        .bubble-group { display: flex; gap: 10px; max-width: 85%; align-items: flex-end; }
        .message-row.user .bubble-group { flex-direction: row-reverse; }

        /* Avatars (Icons now, no text) */
        .avatar { width: 32px; height: 32px; border-radius: 50%; display: flex; align-items: center; justify-content: center; flex-shrink: 0; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        .avatar.user { background: #555; color: white; }
        .avatar.assistant { background: var(--accent-color); color: white; }

        /* Text Bubble */
        .msg-bubble { padding: 10px 15px; border-radius: 12px; font-size: 0.95rem; line-height: 1.5; box-shadow: 0 1px 2px rgba(0,0,0,0.05); overflow-wrap: break-word; min-width: 0; }

        /* User Bubble */
        .message-row.user .msg-bubble {
            background-color: #e3f2fd;
            color: #1565c0;
            border-bottom-right-radius: 2px;
        }

        /* Assistant Bubble */
        .message-row.assistant .msg-bubble {
            background-color: #f5f5f5;
            color: #333;
            border-bottom-left-radius: 2px;
        }

        /* SYSTEM MESSAGE STYLE (Restored) */
        .system-bubble {
            background-color: #fff3cd;
            color: #666;
            padding: 8px 16px;
            border-radius: 20px;
            font-size: 0.85em;
            border: 1px dashed #ccc;
            text-align: center;
            max-width: 90%;
        }

        /* Input Area Styles */
        .input-wrapper { border-top: 1px solid var(--border-color); padding: 20px; display: flex; justify-content: center; background: white; }
        .input-container { width: 100%; max-width: 900px; position: relative; display: flex; flex-direction: column; }
        .chat-input { width: 100%; padding: 12px; padding-right: 45px; border: 1px solid var(--border-color); border-radius: 8px; box-shadow: 0 2px 5px rgba(0,0,0,0.05); resize: none; font-family: inherit; outline: none; transition: border 0.2s; }
        .chat-input:focus { border-color: var(--accent-color); box-shadow: 0 0 0 2px rgba(16, 163, 127, 0.1); }
        .send-btn { position: absolute; right: 8px; bottom: 8px; background: var(--accent-color); color: white; border: none; border-radius: 4px; padding: 6px 10px; cursor: pointer; transition: opacity 0.2s; }
        .send-btn:disabled { background: #ccc; cursor: default; }
        .send-btn:hover:not(:disabled) { background: var(--accent-hover); }
    "#;

    // SVGs for avatars
    let user_icon = html! { <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path><circle cx="12" cy="7" r="4"></circle></svg> };
    let bot_icon = html! { <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="10" rx="2"></rect><circle cx="12" cy="5" r="2"></circle><path d="M12 7v4"></path><line x1="8" y1="16" x2="8" y2="16"></line><line x1="16" y1="16" x2="16" y2="16"></line></svg> };

    html! {
        <>
            <style>{ css }</style>
            <div class="messages-container" ref={scroll_ref}>
                { for props.messages.iter().map(|msg| {
                    if msg.role == "system" {
                        html! {
                            <div class="message-row system">
                                <div class="system-bubble">
                                    { &msg.content }
                                </div>
                            </div>
                        }
                    } else {
                        let role_cls = msg.role.clone();
                        let (avatar_cls, icon) = if msg.role == "user" {
                            ("user", user_icon.clone())
                        } else {
                            ("assistant", bot_icon.clone())
                        };

                        html! {
                            <div class={format!("message-row {}", role_cls)}>
                                <div class="bubble-group">
                                    <div class={format!("avatar {}", avatar_cls)}>{ icon }</div>
                                    <div class="msg-bubble">
                                        { render_markdown(&msg.content) }
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
                    if props.is_loading {
                        <button type="button" class="send-btn" style="background: var(--danger-color);" onclick={props.on_stop.reform(|_| ())}>{"Stop"}</button>
                    } else {
                        <button type="submit" class="send-btn" disabled={input_text.is_empty()}>{"Send"}</button>
                    }
                </form>
            </div>
        </>
    }
}