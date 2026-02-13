use yew::prelude::*;
use web_sys::{HtmlInputElement, HtmlTextAreaElement, HtmlSelectElement};
use wasm_bindgen_futures::spawn_local;
use uuid::Uuid;
use crate::services::llm::LlmService;
use crate::models::{AppSettings, SavedPrompt};

#[derive(Properties, PartialEq, Clone)]
pub struct SettingsProps {
    pub settings: AppSettings,
    pub on_save: Callback<AppSettings>,
    pub on_close: Callback<()>,
    pub on_reset: Callback<()>,
    pub on_clear_chats: Callback<()>,
}

#[function_component(SettingsModal)]
pub fn settings_modal(props: &SettingsProps) -> Html {
    let available_models = use_state(Vec::new);
    let error_msg = use_state(String::new);
    let prompt_name_input = use_state(String::new);

    // Generic helper to emit updates
    let update_settings = {
        let on_save = props.on_save.clone();
        let current_settings = props.settings.clone();
        move |new_settings: AppSettings| {
            on_save.emit(new_settings);
        }
    };

    // -- Existing Field Handlers --

    let on_prompt_change = {
        let settings = props.settings.clone();
        let updater = update_settings.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlTextAreaElement = e.target_unchecked_into();
            let mut s = settings.clone();
            s.system_prompt = input.value();
            updater(s);
        })
    };

    let on_url_input = {
        let settings = props.settings.clone();
        let updater = update_settings.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut s = settings.clone();
            s.base_url = input.value();
            updater(s);
        })
    };

    let on_model_change = {
        let settings = props.settings.clone();
        let updater = update_settings.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            let mut s = settings.clone();
            s.selected_model = select.value();
            updater(s);
        })
    };

    let on_stream_change = {
        let settings = props.settings.clone();
        let updater = update_settings.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let mut s = settings.clone();
            s.stream_enabled = input.checked();
            updater(s);
        })
    };

    let on_doc_context_mode_change = {
        let settings = props.settings.clone();
        let updater = update_settings.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            let mut s = settings.clone();
            s.document_context_mode = if select.value() == "rag" {
                crate::models::DocumentContextMode::RAG
            } else {
                crate::models::DocumentContextMode::Manual
            };
            updater(s);
        })
    };

    let on_fetch = {
        let base_url = props.settings.base_url.clone();
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

    // -- NEW: Prompt Library Handlers --

    // Fix: Explicitly define the input handler here to manage cloning
    let on_name_input = {
        let prompt_name_input = prompt_name_input.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            prompt_name_input.set(i.value());
        })
    };

    let on_save_prompt = {
        let settings = props.settings.clone();
        let updater = update_settings.clone();
        let name_state = prompt_name_input.clone();

        Callback::from(move |_| {
            let name = (*name_state).trim().to_string();
            if !name.is_empty() {
                let mut s = settings.clone();
                s.saved_prompts.push(SavedPrompt {
                    id: Uuid::new_v4().to_string(),
                    name: name,
                    content: s.system_prompt.clone(),
                });
                updater(s);
                name_state.set(String::new()); // Reset input
            }
        })
    };

    let on_load_prompt = {
        let settings = props.settings.clone();
        let updater = update_settings.clone();
        // We technically don't need this wrapper if we handle logic in the loop,
        // but it's kept here if you want to use a <select> in the future.
        // For the list UI, we used inline callbacks in the render loop below.
        Callback::from(move |_: Event| {})
    };

    let on_delete_prompt = {
        let settings = props.settings.clone();
        let updater = update_settings.clone();
        Callback::from(move |id: String| {
            let mut s = settings.clone();
            s.saved_prompts.retain(|p| p.id != id);
            updater(s);
        })
    };

    // Prepare the show_metrics handler outside of the html block
    let on_show_metrics_change = {
        let updater = update_settings.clone();
        let settings = props.settings.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let new_show_metrics = input.checked();
            let mut s = settings.clone();
            s.show_metrics = new_show_metrics;
            updater(s);
        })
    };

    let show_metrics_checked = props.settings.show_metrics;

    let css = r#"
        .settings-backdrop { position: absolute; top: 0; left: 0; width: 100%; height: 100%; background: rgba(255,255,255,0.6); backdrop-filter: blur(2px); z-index: 99; cursor: pointer; }
        .settings-panel { position: absolute; top: 60px; right: 20px; width: 400px; background: white; border: 1px solid var(--border-color); border-radius: 8px; box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.1); padding: 20px; z-index: 100; display: flex; flex-direction: column; gap: 15px; max-height: 80vh; overflow-y: auto; }
        .settings-header { display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid var(--border-color); padding-bottom: 10px; margin-bottom: 5px; }
        .settings-header h3 { margin: 0; font-size: 1.1rem; }
        .close-btn { background: none; border: none; font-size: 1.5rem; line-height: 1; cursor: pointer; color: var(--text-secondary); padding: 0 5px; }
        .close-btn:hover { color: var(--text-primary); }
        .form-label { display: block; font-size: 0.85rem; font-weight: 600; margin-bottom: 5px; color: var(--text-secondary); }
        .fetch-group { display: flex; gap: 8px; }
        .actions { margin-top: 10px; display: flex; flex-direction: column; gap: 8px; }

        /* New Styles for Prompt Library */
        .prompt-tools { display: flex; gap: 5px; margin-bottom: 8px; align-items: center; }
        .prompt-save-row { display: flex; gap: 5px; margin-top: 5px; }
        .mini-btn { padding: 4px 8px; font-size: 0.8rem; }
        .preset-list { display: flex; flex-direction: column; gap: 5px; margin-bottom: 10px; max-height: 100px; overflow-y: auto; border: 1px solid #eee; padding: 5px; border-radius: 4px; }
        .preset-item { display: flex; justify-content: space-between; align-items: center; font-size: 0.85rem; padding: 4px; background: #f9f9f9; border-radius: 4px; }
        .preset-item:hover { background: #eee; }
        .preset-name { cursor: pointer; flex-grow: 1; font-weight: 500; }
        .del-icon { cursor: pointer; color: #999; padding: 0 5px; }
        .del-icon:hover { color: red; }
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

                    // Saved Prompts List
                    if !props.settings.saved_prompts.is_empty() {
                        <div class="preset-list">
                            { for props.settings.saved_prompts.iter().map(|p| {
                                let id_del = p.id.clone();
                                let on_click_del = on_delete_prompt.clone();
                                let content = p.content.clone();
                                let updater = update_settings.clone();
                                let settings_c = props.settings.clone();

                                html! {
                                    <div class="preset-item">
                                        <span class="preset-name" title={content.clone()}
                                              onclick={Callback::from(move |_| {
                                                  let mut s = settings_c.clone();
                                                  s.system_prompt = content.clone();
                                                  updater(s);
                                              })}>
                                            { &p.name }
                                        </span>
                                        <span class="del-icon" onclick={Callback::from(move |_| on_click_del.emit(id_del.clone()))}>{"×"}</span>
                                    </div>
                                }
                            })}
                        </div>
                    }

                    <textarea
                        class="form-textarea"
                        value={props.settings.system_prompt.clone()}
                        oninput={on_prompt_change}
                        style="height: 100px; resize: none; margin-bottom: 5px;"
                    />

                    <div class="prompt-save-row">
                        <input
                            type="text"
                            class="form-input"
                            placeholder="Preset Name (e.g., 'Coder')"
                            style="margin-bottom:0; font-size: 0.9rem;"
                            value={(*prompt_name_input).clone()}
                            oninput={on_name_input} // Uses the pre-defined callback
                        />
                        <button class="btn mini-btn" disabled={prompt_name_input.is_empty()} onclick={on_save_prompt}>
                            { "Save" }
                        </button>
                    </div>
                </div>

                <div>
                    <label class="form-label">{ "Server URL" }</label>
                    <div class="fetch-group">
                        <input class="form-input" type="text" value={props.settings.base_url.clone()} oninput={on_url_input} style="margin-bottom:0;" />
                        <button class="btn" onclick={on_fetch} title="Refresh Models">{ "⟳" }</button>
                    </div>
                </div>

                <div>
                    <label class="form-label">{ "Model" }</label>
                    <select class="form-select" onchange={on_model_change}>
                        {
                            if available_models.is_empty() {
                                html! { <option value={props.settings.selected_model.clone()} selected=true>{ &props.settings.selected_model }</option> }
                            } else {
                                html! { for available_models.iter().map(|m| html! { <option value={m.clone()}>{m}</option> }) }
                            }
                        }
                    </select>
                </div>

                <label style="display: flex; gap: 8px; align-items: center; cursor: pointer; font-size: 0.9rem;">
                    <input type="checkbox" checked={props.settings.stream_enabled} onchange={on_stream_change}/>
                    { "Stream Responses" }
                </label>

                <label style="display: flex; gap: 8px; align-items: center; cursor: pointer; font-size: 0.9rem;">
                    <input type="checkbox" checked={show_metrics_checked} onchange={on_show_metrics_change}/>
                    { "Show Metrics" }
                </label>

                <div>
                    <label class="form-label">{ "Document Context Mode" }</label>
                    <select class="form-select" onchange={on_doc_context_mode_change}>
                        <option value="rag" selected={props.settings.document_context_mode == crate::models::DocumentContextMode::RAG}>{ "RAG (Automatic Context)" }</option>
                        <option value="manual" selected={props.settings.document_context_mode == crate::models::DocumentContextMode::Manual}>{ "Manual (Use @doc-id in prompts)" }</option>
                    </select>
                    <p style="font-size: 0.8rem; color: var(--text-secondary); margin-top: 5px;">
                        { "Choose how documents are used in conversations." }
                    </p>
                </div>

                <div class="actions">
                    <hr style="width: 100%; border: 0; border-top: 1px solid var(--border-color);" />
                    <button class="btn btn-danger" onclick={props.on_clear_chats.reform(|_| ())}>{ "Delete All Chats" }</button>
                    <button class="btn" onclick={props.on_reset.reform(|_| ())}>{ "Reset Settings" }</button>
                </div>
                if !error_msg.is_empty() { <div style="color: red; font-size: 0.8rem;">{ &*error_msg }</div> }
            </div>
        </>
    }
}