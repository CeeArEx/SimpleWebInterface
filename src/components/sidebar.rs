use yew::prelude::*;
use crate::models::ChatSession;
use crate::components::documents::Documents;

#[derive(Properties, PartialEq)]
pub struct SidebarProps {
    pub open: bool,
    pub chats: Vec<ChatSession>,
    pub active_chat_id: String,
    pub on_select: Callback<String>,
    pub on_delete: Callback<(MouseEvent, String)>,
    pub on_new: Callback<()>,
}

#[function_component(Sidebar)]
pub fn sidebar(props: &SidebarProps) -> Html {
    let width = if props.open { "260px" } else { "0px" };

    // CSS for this specific component
    let css = r#"
        .sidebar { background: var(--bg-sidebar); border-right: 1px solid var(--border-color); display: flex; flex-direction: column; transition: width 0.3s cubic-bezier(0.25, 0.8, 0.25, 1); overflow: hidden; flex-shrink: 0; }
        .sidebar-content { width: 260px; height: 100%; display: flex; flex-direction: column; padding: 10px; }
        .chat-list { flex-grow: 1; overflow-y: auto; margin-top: 10px; }
        .chat-item { padding: 10px; border-radius: 6px; cursor: pointer; display: flex; justify-content: space-between; align-items: center; margin-bottom: 2px; font-size: 0.9rem; color: var(--text-primary); }
        .chat-item:hover { background: #eaeaeb; }
        .chat-item.active { background: #e0e0e0; font-weight: 500; }
        .chat-item .del-btn { opacity: 0; border: none; background: none; color: #999; cursor: pointer; padding: 2px 6px; border-radius: 4px; }
        .chat-item:hover .del-btn { opacity: 1; }
        .chat-item .del-btn:hover { background: #dcdcdc; color: #d32f2f; }
        .new-chat-btn { width: 100%; padding: 10px; border: 1px solid var(--border-color); background: white; border-radius: 6px; cursor: pointer; text-align: left; display: flex; gap: 10px; transition: background 0.2s; }
        .new-chat-btn:hover { background: #f0f0f0; }

        /* Documents Section */
        .documents-section { margin-top: 15px; }
        .documents-header { display: flex; justify-content: space-between; align-items: center; padding: 8px 12px; cursor: pointer; border-radius: 6px; transition: background 0.2s; }
        .documents-header:hover { background: #eaeaeb; }
        .documents-header h3 { font-size: 0.85rem; font-weight: 600; color: var(--text-secondary); margin: 0; text-transform: uppercase; letter-spacing: 0.5px; }
        .expand-icon-wrapper { display: flex; align-items: center; }
        .expand-icon { transition: transform 0.3s ease; width: 16px; height: 16px; color: var(--text-secondary); }
        .expand-icon.rotated { transform: rotate(180deg); }

        /* Document List */
        .documents-list { display: flex; flex-direction: column; gap: 6px; margin-top: 12px; }
        .document-item { padding: 10px; border-radius: 8px; cursor: pointer; display: flex; align-items: center; gap: 10px; transition: all 0.2s; background: white; border: 1px solid var(--border-color); }
        .document-item:hover { border-color: var(--accent-color); box-shadow: 0 2px 6px rgba(0,0,0,0.05); }
        .document-item.selected { background: #f0f8f5; border-color: var(--accent-color); box-shadow: 0 2px 6px rgba(16,163,127,0.15); }
        .document-content { display: flex; align-items: center; gap: 10px; flex: 1; min-width: 0; }
        .document-info { display: flex; flex-direction: column; min-width: 0; }
        .document-name { font-size: 0.9rem; font-weight: 500; color: var(--text-primary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
        .document-meta { display: flex; align-items: center; gap: 6px; margin-top: 2px; font-size: 0.75rem; color: var(--text-secondary); }
        .document-separator { color: #d0d0d0; }
        .document-chunks, .document-tokens { color: var(--text-secondary); }
        .document-delete-btn { border: 1px solid var(--border-color); background: transparent; padding: 6px; border-radius: 4px; cursor: pointer; opacity: 0; transition: all 0.2s; color: var(--text-secondary); }
        .document-delete-btn:hover { background: #fee2e2; border-color: var(--danger-color); color: var(--danger-color); }
        .document-item:hover .document-delete-btn { opacity: 1; }

        /* Upload Button */
        .document-upload { padding: 8px 0; }
        .upload-btn { display: flex; align-items: center; justify-content: center; gap: 8px; width: 100%; padding: 10px; border: 2px dashed var(--border-color); background: white; border-radius: 8px; cursor: pointer; transition: all 0.2s; font-size: 0.9rem; color: var(--text-primary); }
        .upload-btn:hover { border-color: var(--accent-color); background: #f9fffc; }
        .upload-btn svg { color: var(--accent-color); }

        /* No Documents State */
        .no-documents { display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 30px 20px; text-align: center; border-radius: 8px; border: 2px dashed var(--border-color); background: #fafafa; }
        .no-documents-icon { margin-bottom: 12px; color: var(--text-secondary); opacity: 0.6; }
        .no-documents p { margin: 8px 0 0 0; font-size: 0.85rem; color: var(--text-secondary); line-height: 1.4; }
        .no-documents .hint { font-size: 0.75rem; color: var(--text-secondary); opacity: 0.7; }

        /* Sidebar separator */
        .documents-section::before { content: ""; display: block; height: 1px; background: var(--border-color); margin-bottom: 15px; }
    "#;

    html! {
        <>
            <style>{ css }</style>
            <div class="sidebar" style={format!("width: {};", width)}>
                <div class="sidebar-content">
                    <button class="new-chat-btn" onclick={props.on_new.reform(|_| ())}>
                        <span>{ "+" }</span>
                        <span>{ "New Chat" }</span>
                    </button>
                    <div class="chat-list">
                        { for props.chats.iter().map(|chat| {
                            let id = chat.id.clone();
                            let is_active = id == props.active_chat_id;
                            let active_class = if is_active { "active" } else { "" };
                            let on_sel = props.on_select.clone();
                            let on_del = props.on_delete.clone();
                            let id_c = id.clone();

                            html! {
                                <div class={format!("chat-item {}", active_class)} onclick={Callback::from(move |_| on_sel.emit(id.clone()))}>
                                    <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{ &chat.title }</span>
                                    <button class="del-btn" onclick={Callback::from(move |e| on_del.emit((e, id_c.clone())))}>{ "×" }</button>
                                </div>
                            }
                        })}
                    </div>

                    <Documents on_document_selected={Callback::from(|id: String| { let _ = id; })} />
                </div>
            </div>
        </>
    }
}
