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
