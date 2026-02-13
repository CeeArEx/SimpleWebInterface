use crate::models::{ChatRequest, ChatResponse, Message, MessageMetrics, ModelListResponse};
use anyhow::Result;
use reqwest::{Client, Response};

pub struct LlmService;

impl LlmService {
    fn get_clean_url(base: &str) -> String {
        base.trim_end_matches('/').to_string()
    }

    pub async fn fetch_models(base_url: &str) -> Result<ModelListResponse> {
        let client = Client::new();
        let url = format!("{}/v1/models", Self::get_clean_url(base_url));
        let resp = client.get(url).send().await?;
        let data = resp.json::<ModelListResponse>().await?;
        Ok(data)
    }

    pub async fn chat_completion_request(
        base_url: &str,
        request: &ChatRequest,
    ) -> Result<Response> {
        let client = Client::new();
        let url = format!("{}/v1/chat/completions", Self::get_clean_url(base_url));

        let resp = client
            .post(url)
            .json(request)
            .send()
            .await?;

        // We return the raw reqwest::Response here to allow
        // the caller to decide between .bytes_stream() or .json()
        Ok(resp)
    }

    /// Helper to generate a title summary
    pub async fn generate_title(base_url: &str, model: &str, messages: &[Message]) -> Result<String> {
        let mut summary_messages = messages.to_vec();
        summary_messages.push(Message {
            role: "user".into(),
            content: "Generate a short title (4-6 words) for this chat. No quotes.".into(),
            metrics: MessageMetrics::default()
        });

        let req = ChatRequest {
            messages: summary_messages,
            model: model.to_string(),
            temperature: 0.7,
            stream: false,
        };

        let resp = Self::chat_completion_request(base_url, &req).await?;
        let json: ChatResponse = resp.json().await?;

        Ok(json.choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .unwrap_or_else(|| "New Chat".to_string()))
    }
}