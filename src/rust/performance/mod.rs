//! Performance measurement and statistics for mullande

use std::fs;
use std::path::Path;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use sys_info;
use crate::memory::Memory;

mod table;
pub use self::table::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceRecord {
    pub timestamp: String,
    pub input_length: InputLength,
    pub output_length: OutputLength,
    pub duration_seconds: f64,
    pub tokens_per_second: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InputLength {
    pub chars: usize,
    pub tokens_estimated: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutputLength {
    pub chars: usize,
    pub tokens_estimated: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemInfo {
    pub captured_at: String,
    pub os: OSInfo,
    pub cpu: CPUInfo,
    pub memory: MemoryInfo,
    pub ollama_version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OSInfo {
    pub name: String,
    pub release: String,
    pub version: String,
    pub architecture: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CPUInfo {
    pub logical_cores: Option<usize>,
    pub physical_cores: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_gb: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelStats {
    pub model_name: String,
    pub total_calls: usize,
    pub total_duration_seconds: f64,
    pub avg_duration_seconds: f64,
    pub avg_tokens_per_second: f64,
    pub avg_input_chars: f64,
    pub avg_output_chars: f64,
    pub total_output_tokens_estimated: usize,
}

pub struct PerformanceCollector {
    memory: Memory,
    perf_dir: &'static str,
}

impl Default for PerformanceCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceCollector {
    pub fn new() -> Self {
        Self {
            memory: Memory::new(None),
            perf_dir: "performance",
        }
    }

    fn sanitize_model_name(model_name: &str) -> String {
        model_name.replace(":", "_").replace("/", "_").replace("\\", "_")
    }

    pub fn get_system_info() -> Result<SystemInfo> {
        let os_name = std::env::consts::OS.to_string();
        let os_release = sys_info::os_release().unwrap_or_default();
        let os_version = "".to_string();
        let arch = std::env::consts::ARCH.to_string();

        let cpu_logical = sys_info::cpu_num().ok().map(|x| x as usize);
        let cpu_physical = sys_info::cpu_num().ok().map(|x| x as usize);

        let mem = sys_info::mem_info()?;
        let mem_total_gb = (mem.total as f64) / (1024.0 * 1024.0) / 1000.0;

        let ollama_version = PerformanceCollector::get_ollama_version();

        Ok(SystemInfo {
            captured_at: Utc::now().to_rfc3339(),
            os: OSInfo {
                name: os_name,
                release: os_release,
                version: os_version,
                architecture: arch,
            },
            cpu: CPUInfo {
                logical_cores: cpu_logical,
                physical_cores: cpu_physical,
            },
            memory: MemoryInfo { total_gb: mem_total_gb },
            ollama_version,
        })
    }

    fn get_ollama_version() -> Option<String> {
        let output = std::process::Command::new("ollama")
            .arg("--version")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = stdout.trim().split_whitespace().collect();
        if parts.len() >= 4 {
            Some(parts[3].to_string())
        } else if let Some(last) = parts.last() {
            Some(last.to_string())
        } else {
            None
        }
    }

    pub fn ensure_initialized(&mut self) -> Result<()> {
        let sys_info_path = format!("{}/system_info.json", self.perf_dir);
        if !self.memory.exists(&sys_info_path) {
            let sys_info = Self::get_system_info()?;
            let json = serde_json::to_string_pretty(&sys_info)?;
            self.memory.write_one(&sys_info_path, &json,
                "Initialize performance tracking: capture system information");
        }
        Ok(())
    }

    pub fn record_call(
        &mut self,
        model_name: &str,
        input_text: &str,
        output_text: &str,
        duration_seconds: f64,
    ) -> Result<()> {
        self.ensure_initialized()?;

        let input_chars = input_text.len();
        let output_chars = output_text.len();
        let input_tokens_est = input_chars / 4;
        let output_tokens_est = output_chars / 4;

        let tokens_per_second = if duration_seconds > 0.0 {
            (output_tokens_est as f64) / duration_seconds
        } else {
            0.0
        };

        let record = PerformanceRecord {
            timestamp: Utc::now().to_rfc3339(),
            input_length: InputLength {
                chars: input_chars,
                tokens_estimated: input_tokens_est,
            },
            output_length: OutputLength {
                chars: output_chars,
                tokens_estimated: output_tokens_est,
            },
            duration_seconds,
            tokens_per_second,
        };

        let safe_name = Self::sanitize_model_name(model_name);
        let jsonl_path = format!("{}/{}.jsonl", self.perf_dir, safe_name);

        let mut existing_content = String::new();
        if self.memory.exists(&jsonl_path) {
            existing_content = self.memory.read(&jsonl_path)?;
        }

        let mut new_content = existing_content;
        new_content.push_str(&serde_json::to_string(&record)?);
        new_content.push('\n');

        self.memory.write_one(&jsonl_path, &new_content,
            &format!("Record performance data for {}: {} tokens in {}s",
                model_name, output_tokens_est, duration_seconds.round()));

        Ok(())
    }

    pub fn get_model_stats(&self, model_name: &str) -> Result<Option<ModelStats>> {
        let safe_name = Self::sanitize_model_name(model_name);
        let jsonl_path = format!("{}/{}.jsonl", self.perf_dir, safe_name);

        if !self.memory.exists(&jsonl_path) {
            return Ok(None);
        }

        let content = self.memory.read(&jsonl_path)?;
        let mut records: Vec<PerformanceRecord> = Vec::new();

         for line in content.lines() {
             let line: &str = line.trim();
             if !line.is_empty() {
                 let record: PerformanceRecord = serde_json::from_str(line)?;
                 records.push(record);
             }
         }

        if records.is_empty() {
            return Ok(None);
        }

        let total_calls = records.len();
        let total_duration: f64 = records.iter().map(|r| r.duration_seconds).sum();
        let total_output_tokens: usize = records.iter().map(|r| r.output_length.tokens_estimated).sum();
        let total_input_chars: usize = records.iter().map(|r| r.input_length.chars).sum();
        let total_output_chars: usize = records.iter().map(|r| r.output_length.chars).sum();
        let total_tokens_per_second: f64 = records.iter().map(|r| r.tokens_per_second).sum();

        Ok(Some(ModelStats {
            model_name: model_name.to_string(),
            total_calls,
            total_duration_seconds: (total_duration * 100.0).round() / 100.0,
            avg_duration_seconds: ((total_duration / (total_calls as f64)) * 100.0).round() / 100.0,
            avg_tokens_per_second: ((total_tokens_per_second / (total_calls as f64)) * 100.0).round() / 100.0,
            avg_input_chars: (total_input_chars as f64) / (total_calls as f64),
            avg_output_chars: (total_output_chars as f64) / (total_calls as f64),
            total_output_tokens_estimated: total_output_tokens,
        }))
    }

    pub fn list_models_with_data(&self) -> Result<Vec<String>> {
        let files = self.memory.list_files()?;
        let mut models = Vec::new();
        let prefix = format!("{}/", self.perf_dir);
         for file in files {
             let file: &str = &file;
             if file.starts_with(&prefix) && file.ends_with(".jsonl") {
                let model_name = file.strip_prefix(&prefix)
                    .unwrap()
                    .strip_suffix(".jsonl")
                    .unwrap()
                    .replace("_", ":");
                models.push(model_name);
            }
        }
        Ok(models)
    }

    pub fn get_system_info_cached(&self) -> Result<Option<SystemInfo>> {
        let sys_info_path = format!("{}/system_info.json", self.perf_dir);
        if !self.memory.exists(&sys_info_path) {
            return Ok(None);
        }
        let content = self.memory.read(&sys_info_path)?;
        Ok(Some(serde_json::from_str(&content)?))
    }
}
