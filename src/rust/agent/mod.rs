//! Core Agent system implementation for mullande

use std::time::Instant;
use anyhow::{Result, anyhow};
use crate::config::{Config, ModelConfig};
use crate::performance::PerformanceCollector;
use crate::workspace::WorkspaceManager;
use crate::memory::Memory;
use crate::logging::Logger;
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
    timeout: Option<std::time::Duration>,
    verbose: bool,
}

impl AgentSystem {
    pub fn new(requested_model: Option<String>) -> Self {
        let workspace = WorkspaceManager::default();
        let config = crate::config::get_config(&workspace.mullande_dir).unwrap();
        let memory = Memory::new(Some(workspace.clone()));
        let conversation_history = memory.load_conversation_history().unwrap_or_else(|_| Vec::new());
        Self {
            config,
            requested_model,
            conversation_history,
            timeout: None,
            verbose: false,
        }
    }

    pub fn set_timeout(&mut self, timeout: std::time::Duration) {
        self.timeout = Some(timeout);
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
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
        let provider = self.model_config().provider.clone();
        let model_id = self.effective_model_id();
        let context_window = self.get_context_window();
        let api_key = self.get_api_key();

        // Build full prompt with conversation history
        let full_prompt = self.build_full_prompt(input_text);

        let start = std::time::Instant::now();
         let result = match provider.as_str() {
             "ollama" => {
                 self.call_ollama(&full_prompt, &model_id, context_window, api_key)
             }
             "volcengine" | "copilot" => {
                 Ok(format!("Provider {} not implemented yet.\nConfiguration:\n- Provider: {}\n- Model: {}\n- Context window: {}",
                     provider, provider, model_id, context_window))
             }
             _ => Ok(format!("Unknown provider: {}", provider)),
         };
         let duration = start.elapsed().as_secs_f64();

         match result {
             Ok(result) => {
                 // Only add to conversation history if call succeeded
                 self.conversation_history.push(input_text.to_string());
                 self.conversation_history.push(result.clone());

                 let input_tokens = full_prompt.len() / 4;

                 self.save_conversation(input_text, &full_prompt, &result, &model_id);
                 Ok(ProcessResult {
                     content: result,
                     model: model_id,
                     input_tokens,
                     duration_seconds: duration,
                 })
             }
             Err(e) => {
                 // Do NOT add failed interaction to conversation history
                 eprintln!("Debug: Full error - {}", e);
                 let mut current = e.source();
                 while let Some(source) = current {
                     eprintln!("  Caused by: {}", source);
                     current = source.source();
                 }
                 Err(e)
             }
         }
    }

    fn build_full_prompt(&self, _new_input: &str) -> String {
        let mut full = String::new();

        // Build conversation history in a format that works well with LLMs
        for (i, turn) in self.conversation_history.iter().enumerate() {
            if i % 2 == 0 {
                full.push_str("### User:\n");
            } else {
                full.push_str("### Assistant:\n");
            }
            full.push_str(turn);
            full.push_str("\n\n");
        }

        // Add the new input (it's already in conversation_history)
        if self.conversation_history.len() % 2 == 1 {
            full.push_str("### Assistant:\n");
        }

        full.trim_end().to_string()
    }

     fn call_ollama(&self, prompt: &str, model: &str, context_window: u32, api_key: Option<String>) -> Result<String> {
          let base_url = self.model_config().base_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string());
          let mut client = OllamaClient::new(&base_url, api_key);
          if let Some(timeout) = self.timeout {
              client.set_timeout(timeout);
          }
          client.set_verbose(self.verbose);

         let start = Instant::now();
         let result = client.chat(model, prompt, context_window);
         let duration = start.elapsed().as_secs_f64();

         match result {
             Ok(r) => {
                 let mut collector = PerformanceCollector::new();
                 let _ = collector.record_call(model, prompt, &r, duration);
                 Ok(r)
             }
             Err(e) => {
                 Err(anyhow!("{}", e))
             }
         }
     }

    fn save_conversation(&mut self, user_input: &str, full_prompt: &str, agent_response: &str, model: &str) {
        let mut memory = Memory::new(None);
        let _ = memory.append_to_conversation(user_input, agent_response, model);

        // Log interaction to .mullande/.logs including full prompt
        let workspace = WorkspaceManager::default();
        let logger = Logger::new(workspace);
        let _ = logger.log_interaction(model, user_input, full_prompt, agent_response);
    }
}
