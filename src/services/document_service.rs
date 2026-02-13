use crate::models::{Document, DocumentChunk, DocumentContextMode};
use anyhow::Result;
use tiktoken_rs::cl100k_base;
use std::collections::HashSet;
use crate::services::storage::LocalStorage;

const CHUNK_SIZE: usize = 1000;
const CHUNK_OVERLAP: usize = 200;

#[derive(Clone, Default)]
pub struct DocumentService;

impl DocumentService {
    const KEY_DOCUMENTS: &'static str = "documents_v1";
    const KEY_CHUNKS: &'static str = "document_chunks_v1";

    /// Get file type from filename
    fn get_file_type(filename: &str) -> String {
        filename
            .split('.')
            .last()
            .unwrap_or("txt")
            .to_lowercase()
    }

    /// Parse a document file (PDF or text) and convert it to markdown chunks
    pub async fn process_document(filename: &str, content: &[u8]) -> Result<Document> {
        let file_type = Self::get_file_type(filename);
        let markdown_content = match file_type.as_str() {
            "pdf" => Self::pdf_to_markdown(content).await?,
            "txt" | "md" => String::from_utf8_lossy(content).to_string(),
            _ => return Err(anyhow::anyhow!("Unsupported file type: {}", file_type)),
        };

        let chunks = Self::chunk_text(&markdown_content);
        let total_tokens = Self::count_tokens(&markdown_content);

        let document = Document {
            id: uuid::Uuid::new_v4().to_string(),
            filename: filename.to_string(),
            file_type,
            upload_date: js_sys::Date::now(),
            chunk_count: chunks.len(),
            total_tokens,
            content_preview: markdown_content.chars().take(200).collect(),
            full_content: markdown_content,
        };

        // Store document metadata
        let mut documents: Vec<Document> = LocalStorage::get_vec(Self::KEY_DOCUMENTS);
        documents.push(document.clone());
        LocalStorage::set(Self::KEY_DOCUMENTS, &documents);

        // Store chunks
        Self::store_chunks(&document.id, &chunks).await;

        Ok(document)
    }

    /// Convert PDF to markdown
    /// Since pdf2md requires file paths, we'll extract text from PDF bytes
    async fn pdf_to_markdown(content: &[u8]) -> Result<String> {
        // For WASM environment without pdf2md support, extract plain text
        // In a real implementation, you would use a PDF parsing library
        // For now, return a simplified representation
        Ok(format!(
            "[PDF Document - Text extraction from PDF bytes]\n\nFile size: {} bytes\nNote: Full PDF parsing requires backend processing.\n\nRaw content preview:\n{}",
            content.len(),
            String::from_utf8_lossy(&content[..std::cmp::min(content.len(), 500)])
        ))
    }

    /// Chunk text into manageable pieces with overlap
    fn chunk_text(text: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let total_len = chars.len();
        
        if total_len <= CHUNK_SIZE {
            chunks.push(text.to_string());
            return chunks;
        }

        let mut start = 0;
        while start < total_len {
            let end = std::cmp::min(start + CHUNK_SIZE, total_len);
            let chunk: String = chars[start..end].iter().collect();
            chunks.push(chunk);
            
            if end == total_len {
                break;
            }
            
            start = end - CHUNK_OVERLAP;
            if start >= total_len {
                break;
            }
        }
        
        chunks
    }

    /// Count tokens in text using cl100k_base tokenizer
    fn count_tokens(text: &str) -> usize {
        match cl100k_base() {
            Ok(tokenizer) => {
                let tokens = tokenizer.encode(text, HashSet::new());
                tokens.len()
            }
            Err(_) => text.split_whitespace().count(),
        }
    }

    /// Store document chunks in local storage
    async fn store_chunks(document_id: &str, chunks: &[String]) {
        let chunk_list: Vec<DocumentChunk> = chunks
            .iter()
            .enumerate()
            .map(|(idx, content)| DocumentChunk {
                id: uuid::Uuid::new_v4().to_string(),
                document_id: document_id.to_string(),
                chunk_index: idx,
                content: content.clone(),
                created_at: js_sys::Date::now(),
            })
            .collect();

        // For the first document, set chunks directly; for others, get and extend
        let existing_chunks: Vec<DocumentChunk> = LocalStorage::get_vec(Self::KEY_CHUNKS);
        let mut all_chunks: Vec<DocumentChunk> = existing_chunks;
        all_chunks.extend(chunk_list);
        LocalStorage::set(Self::KEY_CHUNKS, &all_chunks);
    }

    /// Get all documents
    pub fn get_documents() -> Vec<Document> {
        LocalStorage::get_vec(Self::KEY_DOCUMENTS)
    }

    /// Get chunks for a specific document
    pub fn get_document_chunks(document_id: &str) -> Vec<DocumentChunk> {
        let all_chunks: Vec<DocumentChunk> = LocalStorage::get_vec(Self::KEY_CHUNKS);
        all_chunks
            .into_iter()
            .filter(|c| c.document_id == document_id)
            .collect()
    }

    /// Delete a document and its chunks
    pub fn delete_document(document_id: &str) {
        // Remove document
        let mut documents: Vec<Document> = LocalStorage::get_vec(Self::KEY_DOCUMENTS);
        documents.retain(|d| d.id != document_id);
        LocalStorage::set(Self::KEY_DOCUMENTS, &documents);

        // Remove chunks
        let mut chunks: Vec<DocumentChunk> = LocalStorage::get_vec(Self::KEY_CHUNKS);
        chunks.retain(|c| c.document_id != document_id);
        LocalStorage::set(Self::KEY_CHUNKS, &chunks);
    }

    /// Get the context mode from settings
    pub fn get_context_mode() -> DocumentContextMode {
        let settings: Option<crate::models::AppSettings> = LocalStorage::get("chat_settings_v1");
        settings
            .map(|s| s.document_context_mode)
            .unwrap_or(DocumentContextMode::RAG)
    }

    /// Get document content by document ID
    pub fn get_document_content_by_id(document_id: &str) -> Option<String> {
        let documents = Self::get_documents();
        for doc in documents {
            if doc.id == document_id {
                return Some(doc.full_content);
            }
        }
        None
    }

    /// Build context from documents for the chat
    pub async fn build_context(&self, query: &str, _limit: usize) -> String {
        let mode = Self::get_context_mode();
        
        match mode {
            DocumentContextMode::RAG => {
                // For RAG mode, return all documents as a simple implementation
                Self::get_all_documents_text()
            }
            DocumentContextMode::Manual => {
                // In manual mode, documents are referenced via @doc-id in prompts
                // We need to extract those references and build context from them
                Self::build_manual_context(query)
            }
        }
    }

    /// Build context for manual mode by extracting @doc-id references from the query
    /// Returns both the context (for LLM) and the cleaned message (for display)
    pub async fn build_manual_context_with_display(&self, query: &str) -> (String, String) {
        let documents = Self::get_documents();
        
        if documents.is_empty() {
            return (String::new(), query.to_string());
        }

        // Find all @doc-id patterns in the query
        let mut referenced_docs: Vec<String> = Vec::new();
        let mut current_query = query.to_string();
        
        for doc in &documents {
            let doc_ref = format!("@{}", doc.id);
            if query.contains(&doc_ref) && !referenced_docs.contains(&doc.id) {
                referenced_docs.push(doc.id.clone());
                
                // Replace @doc-id with a cleaner placeholder for display
                current_query = current_query.replace(&doc_ref, &format!("[Document: {}]", doc.filename));
            }
        }

        // Build the context with referenced document content (for LLM)
        let mut context = String::from("Document context:\n\n");
        for doc_id in &referenced_docs {
            if let Some(doc_content) = Self::get_document_content_by_id(doc_id) {
                if let Some(doc) = documents.iter().find(|d| d.id == *doc_id) {
                    context.push_str(&format!(
                        "=== Document: {} (Type: {}, Chunks: {}) ===\n{}\n\n",
                        doc.filename, doc.file_type, doc.chunk_count, doc_content
                    ));
                }
            }
        }

        // If no documents were referenced, return empty context and original query
        if referenced_docs.is_empty() {
            return (String::new(), query.to_string());
        }

        (context, current_query)
    }

    /// Build context for manual mode by extracting @doc-id references from the query
    fn build_manual_context(query: &str) -> String {
        let documents = Self::get_documents();
        
        if documents.is_empty() {
            return String::new();
        }

        // Find all @doc-id patterns in the query
        let mut referenced_docs: Vec<String> = Vec::new();
        let mut current_query = query.to_string();
        
        for doc in &documents {
            let doc_ref = format!("@{}", doc.id);
            if query.contains(&doc_ref) && !referenced_docs.contains(&doc.id) {
                referenced_docs.push(doc.id.clone());
                
                // Replace @doc-id with a placeholder that we can replace later
                current_query = current_query.replace(&doc_ref, &format!("[Document: {}]", doc.filename));
            }
        }

        // Build the context with referenced document content
        let mut context = String::from("Document context:\n\n");
        for doc_id in &referenced_docs {
            if let Some(doc_content) = Self::get_document_content_by_id(doc_id) {
                if let Some(doc) = documents.iter().find(|d| d.id == *doc_id) {
                    context.push_str(&format!(
                        "=== Document: {} (Type: {}, Chunks: {}) ===\n{}\n\n",
                        doc.filename, doc.file_type, doc.chunk_count, doc_content
                    ));
                }
            }
        }

        // If no documents were referenced, return empty context
        if referenced_docs.is_empty() {
            return String::new();
        }

        context
    }

    /// Get a list of documents for manual reference (e.g., @doc-id format)
    fn get_document_list_for_reference() -> String {
        let documents = Self::get_documents();
        
        if documents.is_empty() {
            return String::new();
        }

        let mut list = String::from("Available documents for reference:\n\n");
        for doc in documents {
            list.push_str(&format!("- @{}: {} (Type: {}, {} chunks)\n", doc.id, doc.filename, doc.file_type, doc.chunk_count));
        }
        
        list
    }

    /// Get all document text for RAG context
    fn get_all_documents_text() -> String {
        let documents = Self::get_documents();
        
        if documents.is_empty() {
            return String::new();
        }

        let mut context = String::from("Relevant documents:\n\n");
        for doc in documents {
            context.push_str(&format!(
                "=== Document: {} (Type: {}, Chunks: {}) ===\n",
                doc.filename, doc.file_type, doc.chunk_count
            ));
            context.push_str(&doc.full_content);
            context.push_str("\n\n");
        }
        
        context
    }
}
