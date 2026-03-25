//! Configuration management for mullande

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default)]
pub struct ModelParams {
    pub temperature: Option<f32>,
    pub top_k: Option<u32>,
    pub top_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub thinking: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_enabled: Option<bool>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            provider: "ollama".to_string(),
            model_id: Some("llama3".to_string()),
            base_url: Some("http://localhost:11434".to_string()),
            context_window: None,
            api_key_env: None,
            temperature: None,
            top_k: None,
            top_p: None,
            presence_penalty: None,
            thinking: None,
            tools_enabled: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    pub model: ModelConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<std::collections::HashMap<String, ModelConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_context_window: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub data: ConfigSchema,
    pub config_path: PathBuf,
}

impl Config {
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&self.data)?)
    }

    pub fn save(&self, path: Option<&Path>) -> Result<()> {
        let save_path = match path {
            Some(p) => p.to_path_buf(),
            None => self.config_path.clone(),
        };
        if let Some(parent) = save_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = self.to_json()?;
        fs::write(save_path, json)?;
        Ok(())
    }

    pub fn get_model_config(&self, model_id: Option<&str>) -> ModelConfig {
        match model_id {
            None => self.data.model.clone(),
            Some(model) => {
                if let Some(models) = &self.data.models {
                    if let Some(config) = models.get(model) {
                        let mut merged = self.data.model.clone();
                        if let Some(provider) = Some(&config.provider) {
                            merged.provider = provider.clone();
                        }
                        if config.model_id.is_some() {
                            merged.model_id = config.model_id.clone();
                        }
                        if config.base_url.is_some() {
                            merged.base_url = config.base_url.clone();
                        }
                        if config.context_window.is_some() {
                            merged.context_window = config.context_window;
                        }
                        if config.api_key_env.is_some() {
                            merged.api_key_env = config.api_key_env.clone();
                        }
                        if config.temperature.is_some() {
                            merged.temperature = config.temperature;
                        }
                        if config.top_k.is_some() {
                            merged.top_k = config.top_k;
                        }
                        if config.top_p.is_some() {
                            merged.top_p = config.top_p;
                        }
                        if config.presence_penalty.is_some() {
                            merged.presence_penalty = config.presence_penalty;
                        }
                        if config.thinking.is_some() {
                            merged.thinking = config.thinking;
                        }
                        merged
                    } else {
                        self.data.model.clone()
                    }
                } else {
                    self.data.model.clone()
                }
            }
        }
    }

    pub fn get_model_params(&self, model_id: Option<&str>) -> ModelParams {
        let cfg = self.get_model_config(model_id);
        ModelParams {
            temperature: cfg.temperature,
            top_k: cfg.top_k,
            top_p: cfg.top_p,
            presence_penalty: cfg.presence_penalty,
            thinking: cfg.thinking,
        }
    }

    pub fn get_context_window(&self, model_id: Option<&str>) -> u32 {
        let model_config = self.get_model_config(model_id);
        if let Some(cw) = model_config.context_window {
            return cw;
        }
        if let Some(gcw) = self.data.global_context_window {
            return gcw;
        }
        4096
    }

    pub fn get_api_key(&self, model_id: Option<&str>) -> Option<String> {
        let model_config = self.get_model_config(model_id);
        if let Some(env_name) = &model_config.api_key_env {
            return env::var(env_name).ok();
        }

        let provider = &model_config.provider;
        let default_env = match provider.as_str() {
            "volcengine" => Some("VOLCENGINE_API_KEY"),
            "copilot" => Some("GITHUB_TOKEN"),
            "ollama" => None,
            _ => None,
        };

        match default_env {
            Some(env_name) => env::var(env_name).ok(),
            None => None,
        }
    }
}

pub fn get_config(mullande_dir: &Path) -> Result<Config> {
    let config_path = mullande_dir.join("config.json");

    if !config_path.exists() {
        let default_config = ConfigSchema {
            model: ModelConfig::default(),
            models: None,
            global_context_window: Some(4096),
        };
        let config = Config {
            data: default_config,
            config_path: config_path.clone(),
        };
        config.save(None)?;
        return Ok(config);
    }

    let content = fs::read_to_string(&config_path)?;
    let data: ConfigSchema = serde_json::from_str(&content)
        .map_err(|e| anyhow!("Configuration validation failed: {}", e))?;

    Ok(Config {
        data,
        config_path,
    })
}
