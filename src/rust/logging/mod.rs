//! Logging module for mullande - stores all input/output interactions with models

use std::path::PathBuf;
use std::fs;
use anyhow::Result;
use chrono::Utc;
use crate::workspace::WorkspaceManager;

#[derive(Debug, Clone)]
pub struct Logger {
    workspace: WorkspaceManager,
    logs_dir: PathBuf,
    interactions_dir: PathBuf,
}

impl Logger {
    pub fn new(workspace: WorkspaceManager) -> Self {
        let logs_dir = workspace.mullande_dir.join(".logs");
        let interactions_dir = logs_dir.join("interactions");
        Self {
            workspace,
            logs_dir,
            interactions_dir,
        }
    }

    pub fn initialize(&self) -> Result<()> {
        if !self.logs_dir.exists() {
            fs::create_dir_all(&self.logs_dir)?;
        }
        if !self.interactions_dir.exists() {
            fs::create_dir_all(&self.interactions_dir)?;
        }

        let gitignore_path = self.workspace.mullande_dir.join(".gitignore");
        if !gitignore_path.exists() {
            let content = "# Logs\n.logs/\n*.log\n";
            fs::write(&gitignore_path, content)?;
        } else {
            let existing = fs::read_to_string(&gitignore_path).unwrap_or_default();
            if !existing.contains(".logs/") {
                let new_content = format!("{}\n# Logs\n.logs/\n*.log\n", existing);
                fs::write(&gitignore_path, new_content)?;
            }
        }

        Ok(())
    }

    fn get_daily_log_path(&self) -> PathBuf {
        let date = Utc::now().format("%Y-%m-%d");
        self.logs_dir.join(format!("{}.log", date))
    }

    fn get_interaction_stem(&self) -> String {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S_%f");
        format!("interaction_{}", timestamp)
    }

    pub fn log_interaction(&self, model: &str, user_input: &str, full_prompt: &str, jsonl_entries: &[serde_json::Value], agent_response: &str) -> Result<()> {
        self.write_interaction_log(model, user_input, full_prompt, None, None, Some(jsonl_entries), agent_response)
    }

    pub fn log_interaction_with_tools(
        &self,
        model: &str,
        user_input: &str,
        full_prompt: &str,
        tool_calls_log: &str,
        ollama_exchange_log: &str,
        jsonl_entries: &[serde_json::Value],
        agent_response: &str,
    ) -> Result<()> {
        self.write_interaction_log(
            model, user_input, full_prompt,
            Some(tool_calls_log), Some(ollama_exchange_log),
            Some(jsonl_entries),
            agent_response,
        )
    }

    fn write_interaction_log(
        &self,
        model: &str,
        user_input: &str,
        full_prompt: &str,
        tool_calls_log: Option<&str>,
        ollama_exchange_log: Option<&str>,
        jsonl_entries: Option<&[serde_json::Value]>,
        agent_response: &str,
    ) -> Result<()> {
        self.initialize()?;
        let stem = self.get_interaction_stem();
        let timestamp = Utc::now().to_rfc3339();

        let tools_section = match tool_calls_log {
            Some(log) if !log.is_empty() => format!(
                "================================================================================\n\
TOOL CALLS:\n\
{}\n",
                log
            ),
            _ => String::new(),
        };

        let exchange_section = match ollama_exchange_log {
            Some(log) if !log.is_empty() => format!(
                "================================================================================\n\
OLLAMA API EXCHANGE (full message rounds):\n\
{}\n",
                log
            ),
            _ => String::new(),
        };

        let content = if user_input == full_prompt {
            format!(
                "================================================================================\n\
Timestamp: {}\n\
Model: {}\n\
================================================================================\n\
USER INPUT / FULL PROMPT:\n\
{}\n\
{}{}================================================================================\n\
AGENT RESPONSE:\n\
{}\n\
================================================================================\n\
",
                timestamp, model, user_input.trim(),
                tools_section, exchange_section,
                agent_response.trim()
            )
        } else {
            format!(
                "================================================================================\n\
Timestamp: {}\n\
Model: {}\n\
================================================================================\n\
USER INPUT:\n\
{}\n\
================================================================================\n\
FULL PROMPT (sent to model):\n\
{}\n\
{}{}================================================================================\n\
AGENT RESPONSE:\n\
{}\n\
================================================================================\n\
",
                timestamp, model, user_input.trim(), full_prompt.trim(),
                tools_section, exchange_section,
                agent_response.trim()
            )
        };

        // Write human-readable .log
        let daily_log_path = self.get_daily_log_path();
        if daily_log_path.exists() {
            fs::write(&daily_log_path, fs::read_to_string(&daily_log_path)? + &content + "\n")?;
        } else {
            let header = format!("# Mullande Interaction Logs - {}\n\n", Utc::now().format("%Y-%m-%d"));
            fs::write(&daily_log_path, header + &content + "\n")?;
        }
        let log_path = self.interactions_dir.join(format!("{}.log", stem));
        fs::write(&log_path, &content)?;

        // Write machine-readable .jsonl (one JSON object per line)
        if let Some(entries) = jsonl_entries {
            if !entries.is_empty() {
                let jsonl_path = self.interactions_dir.join(format!("{}.jsonl", stem));
                let lines: Vec<String> = entries.iter()
                    .filter_map(|e| serde_json::to_string(e).ok())
                    .collect();
                fs::write(&jsonl_path, lines.join("\n") + "\n")?;
            }
        }

        Ok(())
    }

    pub fn log_ollama_call(&self, model: &str, full_prompt: &str, response: &str) -> Result<()> {
        self.log_interaction(model, full_prompt, full_prompt, &[], response)
    }

    pub fn log_raw(&self, level: &str, message: &str) -> Result<()> {
        self.initialize()?;

        let timestamp = Utc::now().to_rfc3339();
        let entry = format!("[{}] {}: {}\n", timestamp, level.to_uppercase(), message);

        let general_log = self.logs_dir.join("mullande.log");

        if general_log.exists() {
            fs::write(&general_log, fs::read_to_string(&general_log)? + &entry)?;
        } else {
            fs::write(&general_log, entry)?;
        }

        Ok(())
    }
}
