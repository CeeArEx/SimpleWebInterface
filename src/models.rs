use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub metrics: MessageMetrics,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub created_at: f64,
}

impl ChatSession {
    pub fn new(system_prompt: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: "New Chat".to_string(),
            messages: vec![Message {
                role: "system".to_string(),
                content: system_prompt,
                metrics: MessageMetrics::default(),
            }],
            created_at: js_sys::Date::now(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct SavedPrompt {
    pub id: String,
    pub name: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct DocumentChunk {
    pub id: String,
    pub document_id: String,
    pub chunk_index: usize,
    pub content: String,
    pub created_at: f64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Document {
    pub id: String,
    pub filename: String,
    pub file_type: String,
    pub upload_date: f64,
    pub chunk_count: usize,
    pub total_tokens: usize,
    pub content_preview: String,
    pub full_content: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub enum DocumentContextMode {
    #[serde(rename = "manual")]
    Manual,  // User manually references documents
    #[default]
    RAG,     // Automatic retrieval of relevant chunks (default)
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct AppSettings {
    pub system_prompt: String,
    pub base_url: String,
    pub selected_model: String,
    pub stream_enabled: bool,
    #[serde(default)] // Ensures backward compatibility with existing localStorage data
    pub saved_prompts: Vec<SavedPrompt>,
    #[serde(default)] // Ensures backward compatibility with existing localStorage data
    pub document_context_mode: DocumentContextMode,
    #[serde(default)] // Ensures backward compatibility with existing localStorage data
    pub show_metrics: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            system_prompt: "You are a helpful assistant.".to_string(),
            base_url: "http://localhost:8080".to_string(),
            selected_model: "default".to_string(),
            stream_enabled: true,
            saved_prompts: Vec::new(),
            document_context_mode: DocumentContextMode::RAG,
            show_metrics: true, // Default to showing metrics
        }
    }
}

// API DTOs
#[derive(Serialize, Debug)]
pub struct ChatRequest {
    pub messages: Vec<Message>,
    pub model: String,
    pub temperature: f32,
    pub stream: bool,
}

#[derive(Deserialize, Debug)]
pub struct ChatResponse {
    pub choices: Vec<ChatChoice>,
    #[serde(default)]
    pub usage: Option<UsageInfo>,
    #[serde(default)]
    pub timings: Option<TimingsInfo>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub created: Option<i64>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub system_fingerprint: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ChatChoice {
    #[serde(default)]
    pub message: Message,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct StreamResponse {
    pub choices: Vec<StreamChoice>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub created: Option<i64>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub system_fingerprint: Option<String>,
    #[serde(default)]
    pub usage: Option<UsageInfo>,
    #[serde(default)]
    pub timings: Option<TimingsInfo>,
}

#[derive(Deserialize, Debug)]
pub struct StreamChoice {
    #[serde(default)]
    pub delta: StreamDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub index: Option<u32>,
}

#[derive(Deserialize, Debug, Default)]
pub struct StreamDelta {
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ModelListResponse {
    pub data: Vec<ModelInfo>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ModelInfo {
    pub id: String,
}

// Metrics and Timing Info
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct UsageInfo {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct TimingsInfo {
    pub cache_n: usize,
    pub prompt_n: usize,
    pub prompt_ms: f64,
    pub prompt_per_token_ms: f64,
    pub prompt_per_second: f64,
    pub predicted_n: usize,
    pub predicted_ms: f64,
    pub predicted_per_token_ms: f64,
    pub predicted_per_second: f64,
}

// Data stored with each message for display
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct MessageMetrics {
    pub usage: Option<UsageInfo>,
    pub timings: Option<TimingsInfo>,
    pub created: Option<i64>,
    pub id: Option<String>,
    pub model: Option<String>,
    pub system_fingerprint: Option<String>,
}

impl MessageMetrics {
    pub fn is_empty(&self) -> bool {
        self.usage.is_none() && self.timings.is_none() && self.created.is_none() && self.id.is_none() && self.model.is_none() && self.system_fingerprint.is_none()
    }
}