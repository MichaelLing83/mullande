//! Memory module for mullande - handles .mullande/.memory git repository operations
//! and conversation history management in CONVERSATIONS.md

use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Result, anyhow};
use chrono::Utc;
use crate::workspace::WorkspaceManager;

#[derive(Debug, Clone)]
pub struct Memory {
    workspace: WorkspaceManager,
    memory_dir: PathBuf,
}

impl Default for Memory {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Memory {
    pub fn new(workspace: Option<WorkspaceManager>) -> Self {
        let ws = workspace.unwrap_or_default();
        let mem = ws.get_memory_path().clone();
        Self {
            workspace: ws,
            memory_dir: mem,
        }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        self.memory_dir.join(path)
    }

    pub fn read(&self, path: &str) -> Result<String> {
        let full_path = self.resolve_path(path);
        if !full_path.exists() {
            return Err(anyhow!("File not found in memory: {}", path));
        }
        Ok(fs::read_to_string(full_path)?)
    }

    pub fn read_bytes(&self, path: &str) -> Result<Vec<u8>> {
        let full_path = self.resolve_path(path);
        if !full_path.exists() {
            return Err(anyhow!("File not found in memory: {}", path));
        }
        Ok(fs::read(full_path)?)
    }

    pub fn exists(&self, path: &str) -> bool {
        let full_path = self.resolve_path(path);
        full_path.exists()
    }

    fn get_current_commit(&self) -> Result<String> {
        use std::process::Command;
        let output = Command::new("git")
            .args(&["rev-parse", "HEAD"])
            .current_dir(&self.memory_dir)
            .output()?;
        if !output.status.success() {
            return Err(anyhow!("Failed to get HEAD: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn write_atomic(&mut self, files: Vec<(&str, &str)>, commit_message: &str) -> bool {
        let original_head = match self.get_current_commit() {
            Ok(h) => h,
            Err(_) => String::new(),
        };
        let has_changes = self.workspace.git_has_changes();
        if has_changes {
            if self.workspace.git_stash().is_err() {
                return false;
            }
        }

        let result = (|| -> Result<()> {
            for (path, content) in files {
                let full_path = self.resolve_path(path);
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent).ok();
                }
                fs::write(&full_path, content)?;
                let rel_path = path;
                self.workspace.git_add(Path::new(rel_path));
            }
            self.workspace.git_commit(commit_message)?;
            Ok(())
        })();

        if result.is_err() {
            use std::process::Command;
            let _ = Command::new("git")
                .args(&["reset", "--hard", &original_head])
                .current_dir(&self.memory_dir)
                .output();
            if has_changes {
                let _ = self.workspace.git_stash_pop();
            }
            false
        } else {
            true
        }
    }

    pub fn write_one(&mut self, path: &str, content: &str, commit_message: &str) -> bool {
        self.write_atomic(vec![(path, content)], commit_message)
    }

    pub fn list_files(&self) -> Result<Vec<String>> {
        use std::process::Command;
        let output = Command::new("git")
            .arg("ls-files")
            .current_dir(&self.memory_dir)
            .output()?;
        if !output.status.success() {
            return Err(anyhow!("Failed to list files: {}", String::from_utf8_lossy(&output.stderr)));
        }
        let files = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(files)
    }

    pub fn append_to_conversation(&mut self, user_input: &str, agent_response: &str, model: &str) -> bool {
        const CONVERSATION_PATH: &str = "CONVERSATIONS.md";

        let timestamp = Utc::now().to_rfc3339();
        let entry = format!("\n\n---\n\n**[{}]** Model: `{}`\n\n**User:** {}\n\n**Agent:** {}\n",
            timestamp, model, user_input, agent_response);

        let existing_content = if self.exists(CONVERSATION_PATH) {
            self.read(CONVERSATION_PATH).unwrap_or_default()
        } else {
            "# Mullande Conversation Log\n\nThis file stores all conversations from mullande run and mullande chat.\n".to_string()
        };

        let new_content = existing_content + &entry;
        self.write_one(CONVERSATION_PATH, &new_content,
            &format!("Add conversation turn using model {}: {} chars input", model, user_input.len()))
    }

    /// Returns the next sequential number for a tool_calls/NNN.md file.
    fn next_tool_call_number(&self) -> usize {
        let dir = self.memory_dir.join("tool_calls");
        if !dir.exists() {
            return 1;
        }
        let count = fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
                    .count()
            })
            .unwrap_or(0);
        count + 1
    }

    /// Save each tool call to tool_calls/NNN.md and add hyperlinks to CONVERSATIONS.md.
    /// `tool_calls` is a slice of `(tool_name, args_json, result)`.
    pub fn append_to_conversation_with_tools(
        &mut self,
        user_input: &str,
        agent_response: &str,
        model: &str,
        tool_calls: &[(String, String, String)],
    ) -> bool {
        const CONVERSATION_PATH: &str = "CONVERSATIONS.md";
        let timestamp = Utc::now().to_rfc3339();

        // Build tool call md files and collect link entries
        let mut file_contents: Vec<(String, String)> = Vec::new();
        let mut tool_links: Vec<String> = Vec::new();
        let mut next_num = self.next_tool_call_number();

        for (tool_name, args_json, result) in tool_calls {
            let rel_path = format!("tool_calls/{:03}.md", next_num);

            let md_content = format!(
                "# Tool Call {:03}\n\n**Timestamp:** {}\n**Model:** `{}`\n**Tool:** `{}`\n\n## Request\n\n```json\n{}\n```\n\n## Response\n\n```\n{}\n```\n",
                next_num, timestamp, model, tool_name, args_json, result
            );

            // Link display: "tool_name(args_json)" truncated to 80 chars
            let link_text_full = format!("{}({})", tool_name, args_json);
            let display: String = link_text_full.chars().take(80).collect();
            let display = if link_text_full.chars().count() > 80 {
                format!("{}...", display)
            } else {
                display
            };

            // Relative from CONVERSATIONS.md (at memory root) to tool_calls/NNN.md
            tool_links.push(format!("[{}]({})", display, rel_path));
            file_contents.push((rel_path, md_content));
            next_num += 1;
        }

        // Build CONVERSATIONS.md entry
        let tool_section = if !tool_links.is_empty() {
            let links = tool_links.iter().map(|l| format!("- {}", l)).collect::<Vec<_>>().join("\n");
            format!("\n\n**Tool Calls:**\n{}", links)
        } else {
            String::new()
        };

        let entry = format!(
            "\n\n---\n\n**[{}]** Model: `{}`\n\n**User:** {}{}\n\n**Agent:** {}\n",
            timestamp, model, user_input, tool_section, agent_response
        );

        let existing_content = if self.exists(CONVERSATION_PATH) {
            self.read(CONVERSATION_PATH).unwrap_or_default()
        } else {
            "# Mullande Conversation Log\n\nThis file stores all conversations from mullande run and mullande chat.\n".to_string()
        };

        file_contents.push((CONVERSATION_PATH.to_string(), existing_content + &entry));

        let files: Vec<(&str, &str)> = file_contents
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        self.write_atomic(
            files,
            &format!("Add tool-call conversation: {} tool invocations, model {}", tool_calls.len(), model),
        )
    }

    pub fn load_conversation_history(&self) -> Result<Vec<String>> {
        const CONVERSATION_PATH: &str = "CONVERSATIONS.md";

        if !self.exists(CONVERSATION_PATH) {
            return Ok(Vec::new());
        }

        let content = self.read(CONVERSATION_PATH)?;
        let mut history = Vec::new();

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum ParseState {
            WaitingUser,
            InUser,
            WaitingAgent,
            InAgent,
        }

        let mut state = ParseState::WaitingUser;
        let mut current_user = String::new();
        let mut current_agent = String::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("---") || trimmed.starts_with("**[") || trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with("**User:**") {
                if state == ParseState::InAgent && !current_user.is_empty() && !current_agent.is_empty() {
                    // We have a complete turn
                    history.push(current_user.trim().to_string());
                    history.push(current_agent.trim().to_string());
                    current_user.clear();
                    current_agent.clear();
                }
                state = ParseState::InUser;
                let user_content = trimmed.strip_prefix("**User:**").unwrap_or(trimmed);
                current_user.push_str(user_content);
                current_user.push('\n');
            } else if trimmed.starts_with("**Agent:**") {
                state = ParseState::InAgent;
                let agent_content = trimmed.strip_prefix("**Agent:**").unwrap_or(trimmed);
                current_agent.push_str(agent_content);
                current_agent.push('\n');
            } else if state == ParseState::InUser {
                current_user.push_str(line);
                current_user.push('\n');
            } else if state == ParseState::InAgent {
                current_agent.push_str(line);
                current_agent.push('\n');
            }
        }

        // Handle last incomplete turn if needed
        if state == ParseState::InUser && !current_user.is_empty() {
            history.push(current_user.trim().to_string());
        } else if !current_user.is_empty() && !current_agent.is_empty() {
            history.push(current_user.trim().to_string());
            history.push(current_agent.trim().to_string());
        }

        Ok(history)
    }
}
