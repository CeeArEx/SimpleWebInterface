use yew::prelude::*;
use wasm_bindgen::closure:: Closure;
use wasm_bindgen::{JsValue, JsCast};
use web_sys::{window, HtmlInputElement, Event, FileReader, console};

use crate::services::document_service::DocumentService;

#[derive(Properties, PartialEq)]
pub struct DocumentsProps {
    pub on_document_selected: Callback<String>,
}

#[function_component(Documents)]
pub fn documents(props: &DocumentsProps) -> Html {
    let documents = use_state(|| vec![]);
    let selected_doc_id = use_state(|| String::new());
    let is_expanded = use_state(|| false);

    // Load documents on mount
    {
        let docs = documents.clone();
        use_effect_with(() as (), move |_| {
            let loaded_docs = DocumentService::get_documents();
            docs.set(loaded_docs);
        });
    }

    let on_file_change = {
        let docs = documents.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let files = input.files();
            
            console::log_1(&format!("File change event, files: {:?}", files).into());
            
            if let Some(files) = files {
                if let Some(file) = files.get(0) {
                    let name = file.name();
                    console::log_1(&format!("Selected file: {}", name).into());
                    
                    // Clone Rc for the async task
                    let docs_clone = docs.clone();
                    let file_clone = file.clone();
                    
                    // Create a FileReader
                    match FileReader::new() {
                        Ok(reader) => {
                            console::log_1(&"FileReader created successfully".into());
                            
                            // Create a closure to handle the file reading completion
                            // Clone name so the closure can be Fn instead of FnOnce
                            let name_clone = name.clone();
                            let onload_closure = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
                                console::log_1(&"FileReader.onload called".into());
                                
                                // Get the FileReader from the event target
                                let target = event_target_as_file_reader(&event);
                                if let Some(reader) = target {
                                    console::log_1(&"FileReader found in event target".into());
                                    
                                    // Get the result - it's a Result<JsValue, JsValue>
                                    match reader.result() {
                                        Ok(result) => {
                                            console::log_1(&format!("File result: {:?}", result).into());
                                            
                                            // Get the ArrayBuffer from the result
                                            if let Some(array_buffer) = result.dyn_ref::<js_sys::ArrayBuffer>() {
                                                console::log_1(&format!("Array buffer length: {}", array_buffer.byte_length()).into());
                                                // Create a Uint8Array view over the ArrayBuffer
                                                let uint8_array = js_sys::Uint8Array::new(&array_buffer);
                                                console::log_1(&format!("Uint8Array length: {}", uint8_array.length()).into());
                                                let mut bytes = vec![0; uint8_array.length() as usize];
                                                uint8_array.copy_to(&mut bytes[..]);
                                                console::log_1(&format!("Bytes read: {} bytes", bytes.len()).into());
                                                
                                                // Clone the name again for the async task
                                                let process_name = name_clone.clone();
                                                let process_docs = docs_clone.clone();
                                                
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    console::log_1(&"Starting document processing".into());
                                                    match DocumentService::process_document(&process_name, &bytes).await {
                                                        Ok(_) => {
                                                            console::log_1(&"Document processed successfully".into());
                                                            let loaded_docs = DocumentService::get_documents();
                                                            console::log_1(&format!("Loaded docs count: {}", loaded_docs.len()).into());
                                                            process_docs.set(loaded_docs);
                                                        }
                                                        Err(err) => {
                                                            console::log_1(&format!("Error processing document: {}", err).into());
                                                            if let Some(window) = window() {
                                                                window.alert_with_message(&format!("Error processing document: {}", err)).ok();
                                                            }
                                                        }
                                                    }
                                                });
                                            } else {
                                                console::log_1(&"Failed to get ArrayBuffer from result".into());
                                            }
                                        }
                                        Err(e) => {
                                            console::log_1(&format!("Error getting result: {:?}", e).into());
                                            if let Some(window) = window() {
                                                window.alert_with_message(&format!("Error reading file: {:?}", e)).ok();
                                            }
                                        }
                                    }
                                } else {
                                    console::log_1(&"FileReader not found in event target".into());
                                }
                            });
                            
                            // Set up the onload callback
                            reader.set_onload(Some(onload_closure.as_ref().unchecked_ref()));
                            onload_closure.forget();
                            
                            // Read the file as an array buffer
                            match reader.read_as_array_buffer(&file_clone) {
                                Ok(_) => console::log_1(&"read_as_array_buffer called successfully".into()),
                                Err(err) => {
                                    console::log_1(&format!("Error calling read_as_array_buffer: {:?}", err).into());
                                    if let Some(window) = window() {
                                        window.alert_with_message(&format!("Error reading file: {:?}", err)).ok();
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            console::log_1(&format!("Failed to create FileReader: {:?}", err).into());
                        }
                    }
                }
            }
            
            // Clear the input
            input.set_value("");
        })
    };

    let toggle_expand = {
        let expanded = is_expanded.clone();
        Callback::from(move |_| {
            expanded.set(!*expanded);
        })
    };

    let on_delete_document = {
        let docs = documents.clone();
        Callback::from(move |doc_id: String| {
            DocumentService::delete_document(&doc_id);
            let loaded_docs = DocumentService::get_documents();
            docs.set(loaded_docs);
        })
    };

    let get_file_type_icon = |file_type: &str| -> Html {
        match file_type.to_uppercase().as_str() {
            "PDF" => html! {
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#e74c3c" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                    <polyline points="14 2 14 8 20 8"></polyline>
                    <line x1="16" y1="13" x2="8" y2="13"></line>
                    <line x1="16" y1="17" x2="8" y2="17"></line>
                    <polyline points="10 9 9 9 8 9"></polyline>
                </svg>
            },
            "TXT" => html! {
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#3498db" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                    <polyline points="14 2 14 8 20 8"></polyline>
                    <line x1="16" y1="13" x2="8" y2="13"></line>
                    <line x1="16" y1="17" x2="8" y2="17"></line>
                    <polyline points="10 9 9 9 8 9"></polyline>
                </svg>
            },
            "MD" => html! {
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#27ae60" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                    <polyline points="14 2 14 8 20 8"></polyline>
                    <line x1="16" y1="13" x2="8" y2="13"></line>
                    <line x1="16" y1="17" x2="8" y2="17"></line>
                    <polyline points="10 9 9 9 8 9"></polyline>
                </svg>
            },
            _ => html! {
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#95a5a6" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                    <polyline points="14 2 14 8 20 8"></polyline>
                    <line x1="16" y1="13" x2="8" y2="13"></line>
                    <line x1="16" y1="17" x2="8" y2="17"></line>
                </svg>
            }
        }
    };

    let documents_list = {
        let on_doc_selected = props.on_document_selected.clone();
        let on_del = on_delete_document.clone();
        
        (*documents).iter().map(|doc| {
            let is_selected = (*selected_doc_id) == doc.id;
            let select_class = if is_selected { "document-item selected" } else { "document-item" };
            let doc_id = doc.id.clone();
            let on_sel = on_doc_selected.clone();
            let on_del = on_del.clone();
            let file_type = doc.file_type.clone();

            let doc_id_for_click = doc_id.clone();
            html! {
                <div class={select_class} onclick={Callback::from(move |_| {
                    let _ = on_sel.emit(doc_id_for_click.clone());
                })}>
                    <div class="document-content">
                        { get_file_type_icon(&file_type) }
                        <div class="document-info">
                            <span class="document-name">{ &doc.filename }</span>
                            <div class="document-meta">
                                <span class="document-chunks">{ doc.chunk_count } { "chunks" }</span>
                                <span class="document-separator">{ "•" }</span>
                                <span class="document-tokens">{ format_tokens(doc.total_tokens) }</span>
                            </div>
                        </div>
                    </div>
                    <button class="document-delete-btn" onclick={Callback::from(move |_| on_del.emit(doc_id.clone()))} title="Delete document">
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"></path><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"></path><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"></path></svg>
                    </button>
                </div>
            }
        }).collect::<Vec<_>>()
    };

    html! {
        <div class="documents-section">
            <div class="documents-header" onclick={toggle_expand}>
                <h3>{ "Documents" }</h3>
                <div class="expand-icon-wrapper">
                    <svg class={if *is_expanded { "expand-icon rotated" } else { "expand-icon" }} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="6 9 12 15 18 9"></polyline>
                    </svg>
                </div>
            </div>
            
            if *is_expanded {
                <>
                    <div class="document-upload">
                        <input
                            type="file"
                            accept=".pdf,.txt,.md"
                            onchange={on_file_change}
                            style="display: none;"
                            id="document-upload-input"
                        />
                        <label for="document-upload-input" class="upload-btn">
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
                            <span>{ "Upload Document" }</span>
                        </label>
                    </div>
                    
                    <div class="documents-list">
                        { for documents_list }
                    </div>
                    
                    if documents.is_empty() {
                        <div class="no-documents">
                            <div class="no-documents-icon">
                                <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                                    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                                    <polyline points="14 2 14 8 20 8"></polyline>
                                    <line x1="16" y1="13" x2="8" y2="13"></line>
                                    <line x1="16" y1="17" x2="8" y2="17"></line>
                                    <polyline points="10 9 9 9 8 9"></polyline>
                                </svg>
                            </div>
                            <p>{ "No documents uploaded yet." }</p>
                            <p class="hint">{ "Upload PDF, TXT, or MD files to use as context." }</p>
                        </div>
                    }
                </>
            }
        </div>
    }
}

fn format_tokens(tokens: usize) -> String {
    if tokens >= 1000 {
        format!("{}k", tokens / 1000)
    } else {
        format!("{} tokens", tokens)
    }
}

// Helper function to get FileReader from event target
fn event_target_as_file_reader(event: &JsValue) -> Option<FileReader> {
    let target = event.dyn_ref::<web_sys::Event>()?.target()?;
    target.dyn_ref::<FileReader>().cloned()
}