//! Core Agent system implementation for mullande

use std::time::Instant;
use anyhow::{Result, anyhow};
use crate::config::{Config, ModelConfig, ModelParams};
use crate::performance::PerformanceCollector;
use crate::workspace::WorkspaceManager;
use crate::memory::Memory;
use crate::logging::Logger;
use crate::agent::ollama::{OllamaClient, ChatMessage};
use crate::tools::ToolRegistry;

pub mod ollama;



#[derive(Debug)]
pub struct ProcessResult {
    pub content: String,
    pub model: String,
    pub input_tokens: usize,
    pub duration_seconds: f64,
}

#[derive(Debug)]
struct ToolCallRecord {
    iteration: usize,
    name: String,
    args: String,
    result: String,
}

pub struct AgentSystem {
    pub config: Config,
    pub requested_model: Option<String>,
    conversation_history: Vec<String>,
    timeout: Option<std::time::Duration>,
    verbose: bool,
    params_override: ModelParams,
    tools_enabled: bool,
    last_tool_calls: Vec<ToolCallRecord>,
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
            params_override: ModelParams::default(),
            tools_enabled: false,
            last_tool_calls: Vec::new(),
        }
    }

    pub fn set_timeout(&mut self, timeout: std::time::Duration) {
        self.timeout = Some(timeout);
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    pub fn set_model_params(&mut self, params: ModelParams) {
        self.params_override = params;
    }

    pub fn set_tools_enabled(&mut self, enabled: bool) {
        self.tools_enabled = enabled;
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

        // Merge config params with CLI overrides (CLI has highest priority)
        let config_params = self.config.get_model_params(self.requested_model.as_deref());
        let params = ModelParams {
            temperature: self.params_override.temperature.or(config_params.temperature),
            top_k: self.params_override.top_k.or(config_params.top_k),
            top_p: self.params_override.top_p.or(config_params.top_p),
            presence_penalty: self.params_override.presence_penalty.or(config_params.presence_penalty),
            thinking: self.params_override.thinking.or(config_params.thinking),
        };

         // Build full prompt with conversation history
        let full_prompt = self.build_full_prompt(input_text);

        let start = std::time::Instant::now();
        let result = match provider.as_str() {
            "ollama" => {
                if self.tools_enabled {
                    self.call_ollama_with_tools(&full_prompt, &model_id, context_window, api_key, params)
                } else {
                    self.call_ollama(&full_prompt, &model_id, context_window, api_key, params)
                }
            }
            "volcengine" | "copilot" => {
                Ok((format!("Provider {} not implemented yet.\nConfiguration:\n- Provider: {}\n- Model: {}\n- Context window: {}",
                    provider, provider, model_id, context_window), 0.0, 0.0, 0.0, 0, 0))
            }
            _ => Ok((format!("Unknown provider: {}", provider), 0.0, 0.0, 0.0, 0, 0)),
        };
        let duration = start.elapsed().as_secs_f64();

        match result {
            Ok((result, _ttft, _thinking_time, _answering_time, _thinking_tokens, _answering_tokens)) => {
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

    fn build_full_prompt(&self, new_input: &str) -> String {
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

        // Add the new input
        full.push_str("### User:\n");
        full.push_str(new_input);
        full.push_str("\n\n");

        full.trim_end().to_string()
    }

      fn call_ollama(&self, prompt: &str, model: &str, context_window: u32, api_key: Option<String>, params: ModelParams) -> Result<(String, f64, f64, f64, usize, usize)> {
           let base_url = self.model_config().base_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string());
           let mut client = OllamaClient::new(&base_url, api_key);
           if let Some(timeout) = self.timeout {
               client.set_timeout(timeout);
           }
           client.set_verbose(self.verbose);

          let start = Instant::now();
          let result = client.chat_with_timing(model, prompt, context_window, &params);
          let duration = start.elapsed().as_secs_f64();

          match result {
              Ok((r, ttft, thinking_time, thinking_tokens, answering_tokens)) => {
                  let answering_time = duration - ttft - thinking_time;
                  let mut collector = PerformanceCollector::new();
                  let _ = collector.record_call(model, prompt, &r, duration, ttft, thinking_time, answering_time, thinking_tokens, answering_tokens);
                  Ok((r, duration, ttft, thinking_time, thinking_tokens, answering_tokens))
              }
              Err(e) => {
                  Err(anyhow!("{}", e))
              }
          }
      }

    fn save_conversation(&mut self, user_input: &str, full_prompt: &str, agent_response: &str, model: &str) {
        let mut memory = Memory::new(None);
        let _ = memory.append_to_conversation(user_input, agent_response, model);

        let workspace = WorkspaceManager::default();
        let logger = Logger::new(workspace);

        if self.last_tool_calls.is_empty() {
            let _ = logger.log_interaction(model, user_input, full_prompt, agent_response);
        } else {
            let mut tool_log = String::new();
            for record in &self.last_tool_calls {
                tool_log.push_str(&format!(
                    "[{}] {}({})\n    Result:\n{}\n",
                    record.iteration,
                    record.name,
                    record.args,
                    record.result.lines()
                        .map(|l| format!("    {}", l))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ));
            }
            let _ = logger.log_interaction_with_tools(model, user_input, full_prompt, &tool_log, agent_response);
        }
    }

    fn call_ollama_with_tools(
        &mut self,
        prompt: &str,
        model: &str,
        context_window: u32,
        api_key: Option<String>,
        params: ModelParams,
    ) -> Result<(String, f64, f64, f64, usize, usize)> {
        let base_url = self.model_config().base_url.clone()
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let mut client = OllamaClient::new(&base_url, api_key);
        if let Some(timeout) = self.timeout { client.set_timeout(timeout); }
        client.set_verbose(self.verbose);

        let registry = ToolRegistry::new();
        let tool_defs = registry.to_json_defs();

        let mut messages: Vec<ChatMessage> = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
            thinking: None,
            tool_calls: None,
        }];

        let start = Instant::now();
        let max_iterations = 15;
        let mut final_answer = String::new();
        self.last_tool_calls.clear();

        for iteration in 0..max_iterations {
            let response = client.send_messages(model, messages.clone(), context_window, &params, tool_defs.clone())?;

            if let Some(calls) = response.tool_calls.as_ref().filter(|c| !c.is_empty()) {
                messages.push(response.clone());

                for tc in calls {
                    let args_display = serde_json::to_string(&tc.function.arguments)
                        .unwrap_or_else(|_| "{}".to_string());
                    println!("\x1b[36m[tool:{}] {}({})\x1b[0m",
                        iteration + 1, tc.function.name, args_display);

                    let result = registry.execute(&tc.function.name, &tc.function.arguments);

                    // Show a brief preview in gray
                    let preview: String = result.chars().take(200).collect();
                    let suffix = if result.len() > 200 { "…" } else { "" };
                    println!("\x1b[90m{}{}\x1b[0m", preview, suffix);

                    self.last_tool_calls.push(ToolCallRecord {
                        iteration: iteration + 1,
                        name: tc.function.name.clone(),
                        args: args_display,
                        result: result.clone(),
                    });

                    messages.push(ChatMessage {
                        role: "tool".to_string(),
                        content: result,
                        thinking: None,
                        tool_calls: None,
                    });
                }
            } else {
                final_answer = response.content.clone();
                break;
            }
        }

        let duration = start.elapsed().as_secs_f64();
        let answer_tokens = final_answer.len() / 4;
        Ok((final_answer, duration, 0.0, 0.0, 0, answer_tokens))
    }
}
