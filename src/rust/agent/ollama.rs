//! Ollama API client implementation

use anyhow::{Result, anyhow};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::io::{self, Write, BufRead};

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
    pub thinking: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatOptions {
    #[serde(rename = "num_ctx")]
    pub num_ctx: u32,
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

    pub fn chat(&self, model: &str, prompt: &str, context_window: u32) -> Result<String> {
        let client = self.make_client()?;
        let options = if context_window > 0 {
            Some(ChatOptions { num_ctx: context_window })
        } else {
            None
        };

        let req = ChatRequest {
            model: model.to_string(),
             messages: vec![ChatMessage {
                 role: "user".to_string(),
                 content: prompt.to_string(),
                 thinking: None,
             }],
            options,
            stream: Some(true),
        };

        let url = format!("{}api/chat", self.base_url);
        let mut response = client.post(&url)
            .json(&req)
            .send()?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Ollama API error: {} - {}", status, text));
        }

        let mut full_content = String::new();
        let mut current_thinking = String::new();

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
                                  // Print with [thinking] prefix on every line
                                  print!("\r\x1b[K");
                                  for line in current_thinking.lines() {
                                      println!("[thinking] {}", line);
                                  }
                                  io::stdout().flush()?;
                              }
                          }
                          // Legacy: thinking inside content with <think> tags
                          else if delta.content.contains("<think>") || !current_thinking.is_empty() {
                              current_thinking.push_str(&delta.content);
                              // Print with [thinking] prefix on every line
                              print!("\r\x1b[K");
                              for line in current_thinking.lines() {
                                  println!("[thinking] {}", line);
                              }
                              io::stdout().flush()?;
                          }
                      }
                       // Some responses put thinking in message instead of delta
                       if let Some(message) = &msg.message {
                           // Handle thinking if present
                           if let Some(thinking) = &message.thinking {
                               if !thinking.is_empty() {
                                   current_thinking.push_str(thinking);
                                   // Print with [thinking] prefix on every line
                                   print!("\r\x1b[K");
                                   for line in current_thinking.lines() {
                                       println!("[thinking] {}", line);
                                   }
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

        // Final newline after done thinking to clear the thinking line
        if !current_thinking.is_empty() {
            println!();
        }

        Ok(full_content)
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
}