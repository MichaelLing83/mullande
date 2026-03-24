//! Ollama API client implementation

use anyhow::{Result, anyhow};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub options: Option<ChatOptions>,
    pub stream: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatOptions {
    #[serde(rename = "num_ctx")]
    pub num_ctx: u32,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub message: ChatMessage,
}

#[derive(Debug, Deserialize)]
pub struct ListModelsResponse {
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ModelInfo {
    pub name: String,
}

pub struct OllamaClient {
    base_url: String,
    api_key: Option<String>,
}

impl OllamaClient {
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        let mut url = base_url.to_string();
        if !url.ends_with('/') {
            url.push('/');
        }
        Self {
            base_url: url,
            api_key,
        }
    }

    fn make_client(&self) -> Result<Client> {
        let mut builder = Client::builder();
        if let Some(key) = &self.api_key {
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", key).parse()?,
            );
            builder = builder.default_headers(headers);
        }
        Ok(builder.build()?)
    }

    pub fn chat(&self, model: &str, prompt: &str, context_window: u32) -> Result<String> {
        let client = self.make_client()?;
        let options = if context_window > 0 {
            Some(ChatOptions { num_ctx: context_window })
        } else {
            None
        };

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            options,
            stream: Some(false),
        };

        let url = format!("{}api/chat", self.base_url);
        let response = client.post(&url)
            .json(&request)
            .send()?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Ollama API error: {} - {}", status, text));
        }

        let chat_resp: ChatResponse = response.json()?;
        Ok(chat_resp.message.content)
    }

    pub fn list_models(&self) -> Result<Vec<String>> {
        let client = self.make_client()?;
        let url = format!("{}api/tags", self.base_url);
        let response = client.get(&url).send()?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Failed to list models: {} - {}", status, text));
        }

        let resp: ListModelsResponse = response.json()?;
        let models: Vec<String> = resp.models.into_iter()
            .map(|m| {
                let name = m.name;
                name.strip_suffix(":latest").unwrap_or(&name).to_string()
            })
            .collect();
        Ok(models)
    }
}
