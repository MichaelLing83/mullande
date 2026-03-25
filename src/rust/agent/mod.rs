//! Core Agent system implementation for mullande

use std::time::Instant;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use crate::config::{Config, ModelConfig};
use crate::performance::PerformanceCollector;
use crate::workspace::{WorkspaceManager, Memory};
use crate::agent::ollama::OllamaClient;

pub mod ollama;

#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub content: String,
}

#[derive(Debug)]
pub struct ProcessResult {
    pub content: String,
    pub model: String,
    pub input_tokens: usize,
    pub duration_seconds: f64,
}

pub struct AgentSystem {
    pub config: Config,
    pub requested_model: Option<String>,
    conversation_history: Vec<String>,
}

impl AgentSystem {
    pub fn new(requested_model: Option<String>) -> Self {
        let workspace = WorkspaceManager::default();
        let config = crate::config::get_config(&workspace.mullande_dir).unwrap();
        let model_config = config.get_model_config(requested_model.as_deref());
        Self {
            config,
            requested_model,
            conversation_history: Vec::new(),
        }
    }

    pub fn effective_model_id(&self) -> String {
        match &self.requested_model {
            None => {
                if let Some(model_id) = self.model_config().model_id.clone() {
                    model_id
                } else {
                    "unknown".to_string()
                }
            }
            Some(req) => {
                if let Some(models) = &self.config.data.models {
                    if let Some(model_config) = models.get(req) {
                        if let Some(model_id) = model_config.model_id.clone() {
                            return model_id;
                        }
                    }
                }
                req.clone()
            }
        }
    }

    pub fn model_config(&self) -> ModelConfig {
        self.config.get_model_config(self.requested_model.as_deref())
    }

    pub fn get_context_window(&self) -> u32 {
        self.config.get_context_window(self.requested_model.as_deref())
    }

    pub fn get_api_key(&self) -> Option<String> {
        self.config.get_api_key(self.requested_model.as_deref())
    }

    pub fn process(&mut self, input_text: &str) -> Result<ProcessResult> {
        self.conversation_history.push(input_text.to_string());

        let provider = self.model_config().provider.clone();
        let model_id = self.effective_model_id();
        let context_window = self.get_context_window();
        let api_key = self.get_api_key();

        let start = std::time::Instant::now();
        let result = match provider.as_str() {
            "ollama" => {
                self.call_ollama(input_text, &model_id, context_window, api_key)
            }
            "volcengine" | "copilot" => {
                Ok(format!("Provider {} not implemented yet.\nConfiguration:\n- Provider: {}\n- Model: {}\n- Context window: {}",
                    provider, provider, model_id, context_window))
            }
            _ => Ok(format!("Unknown provider: {}", provider)),
        };
        let duration = start.elapsed().as_secs_f64();

        let result = match result {
            Ok(r) => r,
            Err(e) => format!("Error: {}", e),
        };

        let input_tokens = input_text.len() / 4;

        self.save_conversation(input_text, &result, &model_id);
        Ok(ProcessResult {
            content: result,
            model: model_id,
            input_tokens,
            duration_seconds: duration,
        })
    }

    fn call_ollama(&self, prompt: &str, model: &str, context_window: u32, api_key: Option<String>) -> Result<String> {
        let base_url = self.model_config().base_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string());
        let client = OllamaClient::new(&base_url, api_key);

        let start = Instant::now();
        let result = match client.chat(model, prompt, context_window) {
            Ok(r) => r,
            Err(e) => {
                return Err(anyhow!("Error connecting to ollama: {}\nPlease ensure ollama is running and the model '{}' is pulled.\nHint: Run 'ollama pull {}' to download the model first.",
                    e, model, model))
            }
        };
        let duration = start.elapsed().as_secs_f64();

        let mut collector = PerformanceCollector::new();
        let _ = collector.record_call(model, prompt, &result, duration);

        Ok(result)
    }

    fn save_conversation(&mut self, user_input: &str, agent_response: &str, model: &str) {
        let mut memory = Memory::new(None);
        let conversation_path = "CONVERSATIONS.md";

        let timestamp = Utc::now().to_rfc3339();
        let entry = format!("\n\n---\n\n**[{}]** Model: `{}`\n\n**User:** {}\n\n**Agent:** {}\n",
            timestamp, model, user_input, agent_response);

        let mut existing_content = String::new();
        if memory.exists(conversation_path) {
            existing_content = memory.read(conversation_path).unwrap_or_default();
        } else {
            existing_content = "# Mullande Conversation Log\n\nThis file stores all conversations from mullande run and mullande chat.\n".to_string();
        }

        let new_content = existing_content + &entry;
        let _ = memory.write_one(conversation_path, &new_content,
            &format!("Add conversation turn using model {}: {} chars input", model, user_input.len()));
    }
}
