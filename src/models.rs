use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
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

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct AppSettings {
    pub system_prompt: String,
    pub base_url: String,
    pub selected_model: String,
    pub stream_enabled: bool,
    #[serde(default)] // Ensures backward compatibility with existing localStorage data
    pub saved_prompts: Vec<SavedPrompt>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            system_prompt: "You are a helpful assistant.".to_string(),
            base_url: "http://localhost:8080".to_string(),
            selected_model: "default".to_string(),
            stream_enabled: true,
            saved_prompts: Vec::new(),
        }
    }
}

// API DTOs (Unchanged)
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
}

#[derive(Deserialize, Debug)]
pub struct ChatChoice {
    pub message: Message,
}

#[derive(Deserialize, Debug)]
pub struct StreamResponse {
    pub choices: Vec<StreamChoice>,
}

#[derive(Deserialize, Debug)]
pub struct StreamChoice {
    pub delta: StreamDelta,
}

#[derive(Deserialize, Debug)]
pub struct StreamDelta {
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