//! Ollama API client implementation

use anyhow::{Result, anyhow};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::io::{self, Write, BufRead};
use crate::config::ModelParams;

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub options: Option<ChatOptions>,
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub think: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub function: ToolCallFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatOptions {
    #[serde(rename = "num_ctx")]
    pub num_ctx: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
}

/// Token and timing metrics returned by Ollama for a non-streaming request.
#[derive(Debug, Clone, Default)]
pub struct OllamaCallMetrics {
    /// Number of tokens generated in the response.
    pub eval_count: usize,
    /// Number of input tokens processed.
    pub prompt_eval_count: usize,
    /// Time Ollama spent generating tokens (nanoseconds).
    pub eval_duration_ns: u64,
    /// Total time for the request including model load (nanoseconds).
    pub total_duration_ns: u64,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub message: Option<ChatMessage>,
    pub delta: Option<ChatMessage>,
    pub done: bool,
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
    timeout: Option<std::time::Duration>,
    verbose: bool,
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

    fn make_client(&self) -> Result<Client> {
        let default_timeout = std::time::Duration::from_secs(600); // 10 minutes default
        let timeout = self.timeout.unwrap_or(default_timeout);
        let mut builder = Client::builder()
            .timeout(timeout);
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

    fn build_options(context_window: u32, params: &ModelParams) -> Option<ChatOptions> {
        let has_ctx = context_window > 0;
        let has_params = params.temperature.is_some()
            || params.top_k.is_some()
            || params.top_p.is_some()
            || params.presence_penalty.is_some();
        if !has_ctx && !has_params {
            return None;
        }
        Some(ChatOptions {
            num_ctx: context_window,
            temperature: params.temperature,
            top_k: params.top_k,
            top_p: params.top_p,
            presence_penalty: params.presence_penalty,
        })
    }

    pub fn chat(&self, model: &str, prompt: &str, context_window: u32, params: &ModelParams) -> Result<String> {
        let client = self.make_client()?;
        let options = Self::build_options(context_window, params);

        let req = ChatRequest {
            model: model.to_string(),
             messages: vec![ChatMessage {
                 role: "user".to_string(),
                 content: prompt.to_string(),
                 thinking: None,
                 tool_calls: None,
             }],
            options,
            stream: Some(true),
            think: params.thinking,
            tools: None,
        };

        let url = format!("{}api/chat", self.base_url);
        let response = client.post(&url)
            .json(&req)
            .send()?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Ollama API error: {} - {}", status, text));
        }

        let mut full_content = String::new();
        let mut current_thinking = String::new();
        let mut thinking_started = false;

        let reader = io::BufReader::new(response);
        for line in reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(e) => {
                    return Err(e.into());
                }
            };
            let json: Result<ChatResponse, _> = serde_json::from_str(&line);
            match json {
                Ok(msg) => {
                    if self.verbose {
                        println!("[debug] Received chunk: {:?}", msg);
                    }
                     if let Some(delta) = &msg.delta {
                          // Always add content to full response if it exists
                          if !delta.content.is_empty() {
                              full_content.push_str(&delta.content);
                          }

                            // Handle separate thinking field (new Ollama format like qwen3.5)
                            if let Some(thinking) = &delta.thinking {
                                if !thinking.is_empty() {
                                    current_thinking.push_str(thinking);
                                    if !thinking_started {
                                        print!("\x1b[90m[thinking] ");
                                        thinking_started = true;
                                    }
                                    print!("{}", thinking);
                                    io::stdout().flush()?;
                                }
                            }
                            // Legacy: thinking inside content with <think> tags
                            else if delta.content.contains("<think>") || !current_thinking.is_empty() {
                                current_thinking.push_str(&delta.content);
                                if !thinking_started {
                                    print!("\x1b[90m[thinking] ");
                                    thinking_started = true;
                                }
                                print!("{}", delta.content);
                                io::stdout().flush()?;
                            }
                      }
                       // Some responses put thinking in message instead of delta
                         if let Some(message) = &msg.message {
                             // Handle thinking if present
                             if let Some(thinking) = &message.thinking {
                                 if !thinking.is_empty() {
                                     current_thinking.push_str(thinking);
                                     if !thinking_started {
                                         print!("\x1b[90m[thinking] ");
                                         thinking_started = true;
                                     }
                                     print!("{}", thinking);
                                     io::stdout().flush()?;
                                 }
                             }
                            // Always add content to full response if it exists
                            if !message.content.is_empty() {
                                full_content.push_str(&message.content);
                            }
                        }
                    if msg.done {
                        break;
                    }
                }
                Err(_) => {
                    // Bad JSON line probably due to chunking - just continue
                    if self.verbose {
                        println!("[debug] Failed to parse JSON line: {}", line);
                    }
                    continue;
                }
            }
        }

        // Reset color and print final newline after thinking block
        if !current_thinking.is_empty() {
            print!("\x1b[0m");
            println!();
        }

        Ok(full_content)
    }

    pub fn chat_with_timing(&self, model: &str, prompt: &str, context_window: u32, params: &ModelParams) -> Result<(String, f64, f64, usize, usize)> {
        let client = self.make_client()?;
        let options = Self::build_options(context_window, params);

        let req = ChatRequest {
            model: model.to_string(),
             messages: vec![ChatMessage {
                 role: "user".to_string(),
                 content: prompt.to_string(),
                 thinking: None,
                 tool_calls: None,
             }],
            options,
            stream: Some(true),
            think: params.thinking,
            tools: None,
        };

        let url = format!("{}api/chat", self.base_url);
        let request_time = std::time::Instant::now();
        let response = client.post(&url)
            .json(&req)
            .send()?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Ollama API error: {} - {}", status, text));
        }

        let mut full_content = String::new();
        let mut current_thinking = String::new();
        let mut thinking_started = false;
        
        let mut ttft_seconds: f64 = 0.0;
        let mut thinking_start_time: Option<std::time::Instant> = None;
        let mut thinking_end_time: Option<std::time::Instant> = None;
        let mut in_thinking = false;

        let reader = io::BufReader::new(response);
        for line in reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(e) => {
                    return Err(e.into());
                }
            };
            let json: Result<ChatResponse, _> = serde_json::from_str(&line);
            match json {
                Ok(msg) => {
                    if self.verbose {
                        println!("[debug] Received chunk: {:?}", msg);
                    }
                    
                    if ttft_seconds == 0.0 {
                        ttft_seconds = request_time.elapsed().as_secs_f64();
                    }
                    
                     if let Some(delta) = &msg.delta {
                           // Always add content to full response if it exists
                           if !delta.content.is_empty() {
                               full_content.push_str(&delta.content);
                           }

                            // Handle separate thinking field (new Ollama format like qwen3.5)
                            if let Some(thinking) = &delta.thinking {
                                if !thinking.is_empty() {
                                    if !in_thinking {
                                        in_thinking = true;
                                        thinking_start_time = Some(std::time::Instant::now());
                                    }
                                    current_thinking.push_str(thinking);
                                    if !thinking_started {
                                        print!("\x1b[90m[thinking] ");
                                        thinking_started = true;
                                    }
                                    print!("{}", thinking);
                                    io::stdout().flush()?;
                                }
                            }
                            // Legacy: thinking inside content with <think> tags
                            else if delta.content.contains("<think>") || !current_thinking.is_empty() {
                                if !in_thinking {
                                    in_thinking = true;
                                    thinking_start_time = Some(std::time::Instant::now());
                                }
                                current_thinking.push_str(&delta.content);
                                if !thinking_started {
                                    print!("\x1b[90m[thinking] ");
                                    thinking_started = true;
                                }
                                print!("{}", delta.content);
                                io::stdout().flush()?;
                            } else if in_thinking && !delta.content.is_empty() {
                                // Transition from thinking to answering
                                in_thinking = false;
                                thinking_end_time = Some(std::time::Instant::now());
                            }
                      }
                       // Some responses put thinking in message instead of delta
                         if let Some(message) = &msg.message {
                             // Handle thinking if present
                             if let Some(thinking) = &message.thinking {
                                 if !thinking.is_empty() {
                                     if !in_thinking {
                                         in_thinking = true;
                                         thinking_start_time = Some(std::time::Instant::now());
                                     }
                                     current_thinking.push_str(thinking);
                                     if !thinking_started {
                                         print!("\x1b[90m[thinking] ");
                                         thinking_started = true;
                                     }
                                     print!("{}", thinking);
                                     io::stdout().flush()?;
                                 }
                             }
                            // Always add content to full response if it exists
                            if !message.content.is_empty() {
                                full_content.push_str(&message.content);
                            }
                        }
                    if msg.done {
                        if in_thinking && thinking_start_time.is_some() {
                            thinking_end_time = Some(std::time::Instant::now());
                        }
                        break;
                    }
                }
                Err(_) => {
                    // Bad JSON line probably due to chunking - just continue
                    if self.verbose {
                        println!("[debug] Failed to parse JSON line: {}", line);
                    }
                    continue;
                }
            }
        }

        // Reset color and print final newline after thinking block
        if !current_thinking.is_empty() {
            print!("\x1b[0m");
            println!();
        }

        // Calculate thinking time
        let thinking_time_seconds = if let (Some(start), Some(end)) = (thinking_start_time, thinking_end_time) {
            end.duration_since(start).as_secs_f64()
        } else if in_thinking && thinking_start_time.is_some() {
            // If thinking never ended, use total elapsed time minus TTFT
            request_time.elapsed().as_secs_f64() - ttft_seconds
        } else {
            0.0
        };

        let thinking_tokens = current_thinking.len() / 4;
        let answering_tokens = full_content.len() / 4;

        Ok((full_content, ttft_seconds, thinking_time_seconds, thinking_tokens, answering_tokens))
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
        let models: Vec<String> = resp.models
            .into_iter()
            .map(|m| {
                let name = m.name;
                name.strip_suffix(":latest").unwrap_or(&name).to_string()
            })
            .collect();
        Ok(models)
    }

    /// Send a list of messages (non-streaming) with optional tool definitions.
    /// Returns the assistant's response message and Ollama call metrics.
    pub fn send_messages(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        context_window: u32,
        params: &ModelParams,
        tools: Vec<serde_json::Value>,
    ) -> Result<(ChatMessage, OllamaCallMetrics)> {
        let client = self.make_client()?;
        let options = Self::build_options(context_window, params);
        let tools_opt = if tools.is_empty() { None } else { Some(tools) };

        let req = ChatRequest {
            model: model.to_string(),
            messages,
            options,
            stream: Some(false),
            think: params.thinking,
            tools: tools_opt,
        };

        let url = format!("{}api/chat", self.base_url);
        let response = client.post(&url).json(&req).send()?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Ollama API error: {} - {}", status, text));
        }

        let body: serde_json::Value = response.json()?;
        let metrics = OllamaCallMetrics {
            eval_count: body.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            prompt_eval_count: body.get("prompt_eval_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            eval_duration_ns: body.get("eval_duration").and_then(|v| v.as_u64()).unwrap_or(0),
            total_duration_ns: body.get("total_duration").and_then(|v| v.as_u64()).unwrap_or(0),
        };
        if let Some(msg) = body.get("message") {
            let message: ChatMessage = serde_json::from_value(msg.clone())
                .map_err(|e| anyhow!("Failed to parse message: {}", e))?;
            return Ok((message, metrics));
        }
        Err(anyhow!("No message field in response: {}", body))
    }
}