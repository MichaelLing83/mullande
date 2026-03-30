//! Tool registry for agentic tool calling

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

pub struct ToolRegistry {
    tools: Vec<ToolDef>,
    working_dir: PathBuf,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let working_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."));

        let tools = vec![
            ToolDef {
                name: "read_file",
                description: "Read the contents of a file. Optionally specify start_line and end_line (1-indexed) to read a specific line range.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file (relative to working directory)"
                        },
                        "start_line": {
                            "type": "integer",
                            "description": "First line to read, 1-indexed (optional)"
                        },
                        "end_line": {
                            "type": "integer",
                            "description": "Last line to read, inclusive (optional)"
                        }
                    },
                    "required": ["path"]
                }),
            },
            ToolDef {
                name: "write_file",
                description: "Create a new file or overwrite an existing file with the given content. Parent directories are created automatically.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to write (relative to working directory)"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write to the file"
                        }
                    },
                    "required": ["path", "content"]
                }),
            },
            ToolDef {
                name: "bash",
                description: "Execute a shell command and return its output (stdout + stderr combined). Use for running tests, builds, git operations, etc.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Shell command to execute"
                        }
                    },
                    "required": ["command"]
                }),
            },
            ToolDef {
                name: "glob",
                description: "Find files whose paths match a glob pattern. Returns a sorted list of matching paths relative to the working directory.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Glob pattern (e.g. '**/*.rs', 'src/**/*.ts', '*.json')"
                        },
                        "path": {
                            "type": "string",
                            "description": "Base directory to search in (default: working directory)"
                        }
                    },
                    "required": ["pattern"]
                }),
            },
            ToolDef {
                name: "grep",
                description: "Search for a regex pattern inside file contents. Returns matching lines with file names and line numbers.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Regex pattern to search for"
                        },
                        "path": {
                            "type": "string",
                            "description": "File or directory to search in (default: working directory)"
                        },
                        "glob": {
                            "type": "string",
                            "description": "Glob filter to restrict which files are searched (e.g. '*.rs')"
                        },
                        "case_insensitive": {
                            "type": "boolean",
                            "description": "Case-insensitive search (default: false)"
                        }
                    },
                    "required": ["pattern"]
                }),
            },
            ToolDef {
                name: "subagent",
                description: "Delegate a complex task to a sub-agent. The sub-agent has its own conversation history and can use tools (read_file, write_file, bash, glob, grep) to accomplish the task. Use this for multi-step tasks that require planning and multiple tool calls.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "The task to delegate to the sub-agent. Be specific about what you want accomplished."
                        },
                        "model": {
                            "type": "string",
                            "description": "Model to use for the sub-agent (optional, defaults to configured default model)"
                        }
                    },
                    "required": ["task"]
                }),
            },
        ];

        Self { tools, working_dir }
    }

    /// Returns tool definitions in the Ollama/OpenAI tool-calling JSON format.
    pub fn to_json_defs(&self) -> Vec<Value> {
        self.tools.iter().map(|t| json!({
            "type": "function",
            "function": {
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters.clone()
            }
        })).collect()
    }

    /// Execute a named tool with the given arguments JSON.
    pub fn execute(&self, name: &str, args: &Value) -> String {
        match name {
            "read_file"  => self.read_file(args),
            "write_file" => self.write_file(args),
            "bash"       => self.bash(args),
            "glob"       => self.glob(args),
            "grep"       => self.grep(args),
            "subagent"   => self.subagent(args),
            _            => format!("Error: unknown tool '{}'", name),
        }
    }

    fn subagent(&self, args: &Value) -> String {
        let task = match args["task"].as_str() {
            Some(t) => t,
            None => return "Error: missing required parameter 'task'".to_string(),
        };
        let model = args["model"].as_str();

        format!("[SUBAGENT] To execute a subagent, please use AgentSystem::run_subagent() directly.\nTask: {}\nModel: {:?}", task, model)
    }

    fn resolve(&self, path_str: &str) -> PathBuf {
        let p = Path::new(path_str);
        if p.is_absolute() { p.to_path_buf() } else { self.working_dir.join(p) }
    }

    fn read_file(&self, args: &Value) -> String {
        let path = match args["path"].as_str() {
            Some(p) => p,
            None => return "Error: missing required parameter 'path'".to_string(),
        };
        match std::fs::read_to_string(self.resolve(path)) {
            Ok(content) => {
                let start = args["start_line"].as_u64().map(|n| n as usize);
                let end   = args["end_line"].as_u64().map(|n| n as usize);
                if start.is_none() && end.is_none() {
                    format!("File: {}\n```\n{}\n```", path, content)
                } else {
                    let lines: Vec<&str> = content.lines().collect();
                    let s = start.unwrap_or(1).saturating_sub(1);
                    let e = end.unwrap_or(lines.len()).min(lines.len());
                    let numbered: Vec<String> = lines[s..e].iter().enumerate()
                        .map(|(i, l)| format!("{:4}: {}", s + i + 1, l))
                        .collect();
                    format!("File: {} (lines {}-{})\n```\n{}\n```", path, s + 1, e, numbered.join("\n"))
                }
            }
            Err(e) => format!("Error reading '{}': {}", path, e),
        }
    }

    fn write_file(&self, args: &Value) -> String {
        let path = match args["path"].as_str() {
            Some(p) => p,
            None => return "Error: missing required parameter 'path'".to_string(),
        };
        let content = match args["content"].as_str() {
            Some(c) => c,
            None => return "Error: missing required parameter 'content'".to_string(),
        };
        let full = self.resolve(path);
        if let Some(parent) = full.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return format!("Error creating directories for '{}': {}", path, e);
            }
        }
        match std::fs::write(&full, content) {
            Ok(()) => format!("Wrote {} bytes to '{}'", content.len(), path),
            Err(e) => format!("Error writing '{}': {}", path, e),
        }
    }

    fn bash(&self, args: &Value) -> String {
        let command = match args["command"].as_str() {
            Some(c) => c,
            None => return "Error: missing required parameter 'command'".to_string(),
        };
        match Command::new("bash").arg("-c").arg(command)
            .current_dir(&self.working_dir).output()
        {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let code = out.status.code().unwrap_or(-1);
                let mut result = format!("Exit code: {}", code);
                if !stdout.is_empty() {
                    result.push_str("\nSTDOUT:\n");
                    result.push_str(stdout.trim_end());
                }
                if !stderr.is_empty() {
                    result.push_str("\nSTDERR:\n");
                    result.push_str(stderr.trim_end());
                }
                result
            }
            Err(e) => format!("Error running command: {}", e),
        }
    }

    fn glob(&self, args: &Value) -> String {
        let pattern = match args["pattern"].as_str() {
            Some(p) => p,
            None => return "Error: missing required parameter 'pattern'".to_string(),
        };
        let base = args["path"].as_str()
            .map(|p| self.resolve(p))
            .unwrap_or_else(|| self.working_dir.clone());
        let full_pattern = base.join(pattern).to_string_lossy().to_string();

        match glob::glob(&full_pattern) {
            Ok(entries) => {
                let mut paths: Vec<String> = entries.filter_map(|r| r.ok())
                    .map(|p| p.strip_prefix(&self.working_dir)
                        .map(|r| r.to_string_lossy().to_string())
                        .unwrap_or_else(|_| p.to_string_lossy().to_string()))
                    .collect();
                paths.sort();
                if paths.is_empty() {
                    format!("No files match '{}'", pattern)
                } else {
                    format!("Found {} file(s) matching '{}':\n{}", paths.len(), pattern, paths.join("\n"))
                }
            }
            Err(e) => format!("Invalid glob pattern '{}': {}", pattern, e),
        }
    }

    fn grep(&self, args: &Value) -> String {
        let pattern = match args["pattern"].as_str() {
            Some(p) => p,
            None => return "Error: missing required parameter 'pattern'".to_string(),
        };
        let search_path = args["path"].as_str()
            .map(|p| self.resolve(p))
            .unwrap_or_else(|| self.working_dir.clone());
        let case_insensitive = args["case_insensitive"].as_bool().unwrap_or(false);

        // Try ripgrep first, fall back to system grep
        let output = {
            let mut cmd = Command::new("rg");
            cmd.arg("--line-number").arg("--no-heading").arg("--color=never");
            if case_insensitive { cmd.arg("--ignore-case"); }
            if let Some(g) = args["glob"].as_str() { cmd.arg("--glob").arg(g); }
            cmd.arg(pattern).arg(&search_path).current_dir(&self.working_dir);
            cmd.output()
        }.or_else(|_| {
            let mut cmd = Command::new("grep");
            cmd.arg("-rn").arg("--color=never");
            if case_insensitive { cmd.arg("-i"); }
            if let Some(g) = args["glob"].as_str() { cmd.arg("--include").arg(g); }
            cmd.arg(pattern).arg(&search_path).current_dir(&self.working_dir);
            cmd.output()
        });

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.is_empty() {
                    format!("No matches for '{}'", pattern)
                } else {
                    let base = format!("{}/", self.working_dir.to_string_lossy());
                    let cleaned = stdout.replace(&base, "");
                    format!("Matches for '{}':\n{}", pattern, cleaned.trim_end())
                }
            }
            Err(e) => format!("Error: {}", e),
        }
    }
}
