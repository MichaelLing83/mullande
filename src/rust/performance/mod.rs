//! Performance measurement and statistics for mullande

use std::fs;
use std::path::PathBuf;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use sys_info;
use crate::workspace::WorkspaceManager;

mod table;
pub use self::table::show_stats;

#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceRecord {
    pub timestamp: String,
    pub input_length: InputLength,
    pub output_length: OutputLength,
    pub duration_seconds: f64,
    pub tokens_per_second: f64,
    pub ttft_seconds: f64,
    pub thinking_time_seconds: f64,
    pub answering_time_seconds: f64,
    pub thinking_tokens: usize,
    pub answering_tokens: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InputLength {
    pub chars: usize,
    pub tokens_estimated: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutputLength {
    pub chars: usize,
    pub thinking_chars: usize,
    pub answering_chars: usize,
    pub tokens_estimated: usize,
    pub thinking_tokens: usize,
    pub answering_tokens: usize,
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
    pub avg_ttft_seconds: f64,
    pub avg_thinking_time_seconds: f64,
    pub avg_answering_time_seconds: f64,
    pub avg_thinking_tokens: f64,
    pub avg_answering_tokens: f64,
    pub thinking_tokens_per_second: f64,
    pub answering_tokens_per_second: f64,
    pub answering_tokens_per_total_time: f64,
}

pub struct PerformanceCollector {
    perf_dir: PathBuf,
}

impl Default for PerformanceCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceCollector {
    pub fn new() -> Self {
        let workspace = WorkspaceManager::default();
        Self {
            perf_dir: workspace.mullande_dir.join("performance"),
        }
    }

    fn resolve(&self, rel: &str) -> PathBuf {
        self.perf_dir.join(rel)
    }

    fn file_exists(&self, rel: &str) -> bool {
        self.resolve(rel).exists()
    }

    fn read_file(&self, rel: &str) -> Result<String> {
        let path = self.resolve(rel);
        if !path.exists() {
            return Err(anyhow!("Performance file not found: {}", rel));
        }
        Ok(fs::read_to_string(path)?)
    }

    fn write_file(&self, rel: &str, content: &str) -> Result<()> {
        let path = self.resolve(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        Ok(())
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
         let cpu_physical = None;

         let mem = sys_info::mem_info()?;
         let mem_total_gb = (mem.total as f64) / (1024.0 * 1024.0);

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
        if !self.file_exists("system_info.json") {
            let sys_info = Self::get_system_info()?;
            let json = serde_json::to_string_pretty(&sys_info)?;
            self.write_file("system_info.json", &json)?;
        }
        Ok(())
    }

    pub fn record_call(
        &mut self,
        model_name: &str,
        input_text: &str,
        output_text: &str,
        duration_seconds: f64,
        ttft_seconds: f64,
        thinking_time_seconds: f64,
        answering_time_seconds: f64,
        thinking_tokens: usize,
        answering_tokens: usize,
    ) -> Result<()> {
        self.ensure_initialized()?;

        let input_chars = input_text.len();
        let output_chars = output_text.len();
        let input_tokens_est = input_chars / 4;
        
        let thinking_chars = thinking_tokens * 4;
        let answering_chars = answering_tokens * 4;
        let total_output_tokens = thinking_tokens + answering_tokens;

        let tokens_per_second = if duration_seconds > 0.0 {
            (total_output_tokens as f64) / duration_seconds
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
                thinking_chars,
                answering_chars,
                tokens_estimated: total_output_tokens,
                thinking_tokens,
                answering_tokens,
            },
            duration_seconds,
            tokens_per_second,
            ttft_seconds,
            thinking_time_seconds,
            answering_time_seconds,
            thinking_tokens,
            answering_tokens,
        };

        let safe_name = Self::sanitize_model_name(model_name);
        let jsonl_file = format!("{}.jsonl", safe_name);

        let mut existing_content = String::new();
        if self.file_exists(&jsonl_file) {
            existing_content = self.read_file(&jsonl_file)?;
        }

        let mut new_content = existing_content;
        new_content.push_str(&serde_json::to_string(&record)?);
        new_content.push('\n');

        self.write_file(&jsonl_file, &new_content)?;

        Ok(())
    }

    pub fn get_model_stats(&self, model_name: &str) -> Result<Option<ModelStats>> {
        let safe_name = Self::sanitize_model_name(model_name);
        let jsonl_file = format!("{}.jsonl", safe_name);

        if !self.file_exists(&jsonl_file) {
            return Ok(None);
        }

        let content = self.read_file(&jsonl_file)?;
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
        
        let total_ttft: f64 = records.iter().map(|r| r.ttft_seconds).sum();
        let total_thinking_time: f64 = records.iter().map(|r| r.thinking_time_seconds).sum();
        let total_answering_time: f64 = records.iter().map(|r| r.answering_time_seconds).sum();
        let total_thinking_tokens: usize = records.iter().map(|r| r.thinking_tokens).sum();
        let total_answering_tokens: usize = records.iter().map(|r| r.answering_tokens).sum();

        let avg_thinking_tokens_per_second = if total_thinking_time > 0.0 {
            (total_thinking_tokens as f64) / total_thinking_time
        } else {
            0.0
        };
        
        let avg_answering_tokens_per_second = if total_answering_time > 0.0 {
            (total_answering_tokens as f64) / total_answering_time
        } else {
            0.0
        };

        let avg_answering_per_total_time = if total_duration > 0.0 {
            (total_answering_tokens as f64) / total_duration
        } else {
            0.0
        };

        Ok(Some(ModelStats {
            model_name: model_name.to_string(),
            total_calls,
            total_duration_seconds: (total_duration * 100.0).round() / 100.0,
            avg_duration_seconds: ((total_duration / (total_calls as f64)) * 100.0).round() / 100.0,
            avg_tokens_per_second: ((total_tokens_per_second / (total_calls as f64)) * 100.0).round() / 100.0,
            avg_input_chars: (total_input_chars as f64) / (total_calls as f64),
            avg_output_chars: (total_output_chars as f64) / (total_calls as f64),
            total_output_tokens_estimated: total_output_tokens,
            avg_ttft_seconds: ((total_ttft / (total_calls as f64)) * 100.0).round() / 100.0,
            avg_thinking_time_seconds: ((total_thinking_time / (total_calls as f64)) * 100.0).round() / 100.0,
            avg_answering_time_seconds: ((total_answering_time / (total_calls as f64)) * 100.0).round() / 100.0,
            avg_thinking_tokens: (total_thinking_tokens as f64) / (total_calls as f64),
            avg_answering_tokens: (total_answering_tokens as f64) / (total_calls as f64),
            thinking_tokens_per_second: (avg_thinking_tokens_per_second * 100.0).round() / 100.0,
            answering_tokens_per_second: (avg_answering_tokens_per_second * 100.0).round() / 100.0,
            answering_tokens_per_total_time: (avg_answering_per_total_time * 100.0).round() / 100.0,
        }))
    }

    pub fn list_models_with_data(&self) -> Result<Vec<String>> {
        if !self.perf_dir.exists() {
            return Ok(Vec::new());
        }
        let mut models = Vec::new();
        for entry in fs::read_dir(&self.perf_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".jsonl") {
                let model_name = name.strip_suffix(".jsonl").unwrap().replace("_", ":");
                models.push(model_name);
            }
        }
        Ok(models)
    }

    pub fn get_system_info_cached(&self) -> Result<Option<SystemInfo>> {
        if !self.file_exists("system_info.json") {
            return Ok(None);
        }
        let content = self.read_file("system_info.json")?;
        Ok(Some(serde_json::from_str(&content)?))
    }
}
