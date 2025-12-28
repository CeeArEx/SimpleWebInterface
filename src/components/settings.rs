use yew::prelude::*;
use web_sys::{HtmlInputElement, HtmlTextAreaElement, HtmlSelectElement};
use wasm_bindgen_futures::spawn_local;
use crate::services::llm::LlmService;

#[derive(Properties, PartialEq, Clone)]
pub struct SettingsProps {
    pub system_prompt: String,
    pub base_url: String,
    pub selected_model: String,
    pub stream_enabled: bool,
    pub on_save: Callback<(String, String, String, bool)>,
    pub on_close: Callback<()>,
    pub on_reset: Callback<()>,       // <--- We will use this
    pub on_clear_chats: Callback<()>, // <--- And this
}

#[function_component(SettingsModal)]
pub fn settings_modal(props: &SettingsProps) -> Html {
    let available_models = use_state(Vec::new);
    let error_msg = use_state(String::new);

    // ... (Keep existing input callbacks: on_prompt_change, on_url_input, etc.) ...
    let on_prompt_change = {
        let on_save = props.on_save.clone();
        let base_url = props.base_url.clone();
        let selected_model = props.selected_model.clone();
        let stream_enabled = props.stream_enabled;
        Callback::from(move |e: Event| {
            let input: HtmlTextAreaElement = e.target_unchecked_into();
            on_save.emit((input.value(), base_url.clone(), selected_model.clone(), stream_enabled));
        })
    };

    let on_url_input = {
        let on_save = props.on_save.clone();
        let system_prompt = props.system_prompt.clone();
        let selected_model = props.selected_model.clone();
        let stream_enabled = props.stream_enabled;
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            on_save.emit((system_prompt.clone(), input.value(), selected_model.clone(), stream_enabled));
        })
    };

    let on_model_change = {
        let on_save = props.on_save.clone();
        let system_prompt = props.system_prompt.clone();
        let base_url = props.base_url.clone();
        let stream_enabled = props.stream_enabled;
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            on_save.emit((system_prompt.clone(), base_url.clone(), select.value(), stream_enabled));
        })
    };

    let on_stream_change = {
        let on_save = props.on_save.clone();
        let system_prompt = props.system_prompt.clone();
        let base_url = props.base_url.clone();
        let selected_model = props.selected_model.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            on_save.emit((system_prompt.clone(), base_url.clone(), selected_model.clone(), input.checked()));
        })
    };

    let on_fetch = {
        let base_url = props.base_url.clone();
        let models = available_models.clone();
        let err = error_msg.clone();
        Callback::from(move |_| {
            let url = base_url.clone();
            let models = models.clone();
            let err = err.clone();
            spawn_local(async move {
                match LlmService::fetch_models(&url).await {
                    Ok(resp) => models.set(resp.data.into_iter().map(|m| m.id).collect()),
                    Err(e) => err.set(e.to_string()),
                }
            });
        })
    };

    // --- NEW: Wrappers to emit the reset/clear events ---
    let on_clear_click = {
        let cb = props.on_clear_chats.clone();
        Callback::from(move |_| cb.emit(()))
    };

    let on_reset_click = {
        let cb = props.on_reset.clone();
        Callback::from(move |_| cb.emit(()))
    };

    let css = r#"
        .settings-backdrop { position: absolute; top: 0; left: 0; width: 100%; height: 100%; background: rgba(255,255,255,0.6); backdrop-filter: blur(2px); z-index: 99; cursor: pointer; }
        .settings-panel { position: absolute; top: 60px; right: 20px; width: 340px; background: white; border: 1px solid var(--border-color); border-radius: 8px; box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.1); padding: 20px; z-index: 100; display: flex; flex-direction: column; gap: 15px; }
        .settings-header { display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid var(--border-color); padding-bottom: 10px; margin-bottom: 5px; }
        .settings-header h3 { margin: 0; font-size: 1.1rem; }
        .close-btn { background: none; border: none; font-size: 1.5rem; line-height: 1; cursor: pointer; color: var(--text-secondary); padding: 0 5px; }
        .close-btn:hover { color: var(--text-primary); }
        .form-label { display: block; font-size: 0.85rem; font-weight: 600; margin-bottom: 5px; color: var(--text-secondary); }
        .fetch-group { display: flex; gap: 8px; }
        .actions { margin-top: 10px; display: flex; flex-direction: column; gap: 8px; }
    "#;

    html! {
        <>
            <style>{ css }</style>
            <div class="settings-backdrop" onclick={props.on_close.reform(|_| ())}></div>

            <div class="settings-panel">
                <div class="settings-header">
                    <h3>{ "Configuration" }</h3>
                    <button class="close-btn" onclick={props.on_close.reform(|_| ())} title="Close">{"×"}</button>
                </div>

                <div>
                    <label class="form-label">{ "System Prompt" }</label>
                    <textarea class="form-textarea" value={props.system_prompt.clone()} onchange={on_prompt_change} style="height: 80px; resize: none;" />
                </div>

                <div>
                    <label class="form-label">{ "Server URL" }</label>
                    <div class="fetch-group">
                        <input class="form-input" type="text" value={props.base_url.clone()} oninput={on_url_input} style="margin-bottom:0;" />
                        <button class="btn" onclick={on_fetch} title="Refresh Models">{ "⟳" }</button>
                    </div>
                </div>

                <div>
                    <label class="form-label">{ "Model" }</label>
                    <select class="form-select" onchange={on_model_change}>
                        {
                            if available_models.is_empty() {
                                html! { <option value={props.selected_model.clone()} selected=true>{ &props.selected_model }</option> }
                            } else {
                                html! { for available_models.iter().map(|m| html! { <option value={m.clone()}>{m}</option> }) }
                            }
                        }
                    </select>
                </div>

                <label style="display: flex; gap: 8px; align-items: center; cursor: pointer; font-size: 0.9rem;">
                    <input type="checkbox" checked={props.stream_enabled} onchange={on_stream_change}/>
                    { "Stream Responses" }
                </label>

                <div class="actions">
                    <hr style="width: 100%; border: 0; border-top: 1px solid var(--border-color);" />
                    // --- UPDATED: Use the new handlers ---
                    <button class="btn btn-danger" onclick={on_clear_click}>{ "Delete All Chats" }</button>
                    <button class="btn" onclick={on_reset_click}>{ "Reset Settings" }</button>
                </div>
                if !error_msg.is_empty() { <div style="color: red; font-size: 0.8rem;">{ &*error_msg }</div> }
            </div>
        </>
    }
}