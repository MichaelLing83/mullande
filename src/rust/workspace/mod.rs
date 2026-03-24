//! Workspace management for mullande
//! Handles initialization and management of .mullande/.memory git repository

use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{Result, anyhow};
use std::fs;

#[derive(Debug, Clone)]
pub struct WorkspaceManager {
    pub root_dir: PathBuf,
    pub mullande_dir: PathBuf,
    pub memory_dir: PathBuf,
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new(None)
    }
}

impl WorkspaceManager {
    pub fn new(root_dir: Option<&Path>) -> Self {
        let root = root_dir.map_or_else(|| std::env::current_dir().unwrap(), |p| p.to_path_buf());
        let mullande = root.join(".mullande");
        let memory = mullande.join(".memory");
        Self {
            root_dir: root,
            mullande_dir: mullande,
            memory_dir: memory,
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        self.create_directories();
        self.init_git_repo()?;
        Ok(())
    }

    fn create_directories(&mut self) {
        if !self.mullande_dir.exists() {
            let _ = fs::create_dir_all(&self.mullande_dir);
        }
        if !self.memory_dir.exists() {
            let _ = fs::create_dir_all(&self.memory_dir);
        }
    }

    fn init_git_repo(&mut self) -> Result<()> {
        let git_dir = self.memory_dir.join(".git");
        if !git_dir.exists() {
            let output = Command::new("git")
                .arg("init")
                .current_dir(&self.memory_dir)
                .output()?;
            if !output.status.success() {
                return Err(anyhow!("Failed to init git: {}", String::from_utf8_lossy(&output.stderr)));
            }

            Command::new("git")
                .args(&["config", "user.name", "mullande"])
                .current_dir(&self.memory_dir)
                .output()
                .ok();

            Command::new("git")
                .args(&["config", "user.email", "mullande@localhost"])
                .current_dir(&self.memory_dir)
                .output()
                .ok();

            let gitignore = self.memory_dir.join(".gitignore");
            if !gitignore.exists() {
                let content = r#"# OS files
.DS_Store
Thumbs.db

# Temporary files
*.tmp
*.log
"#;
                fs::write(&gitignore, content)?;
                self.git_add(gitignore.strip_prefix(&self.memory_dir).unwrap());
                self.git_commit("Initial commit: Add .gitignore")?;
            }
        }
        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.mullande_dir.exists() && self.memory_dir.exists() && (self.memory_dir.join(".git")).exists()
    }

    pub fn get_memory_path(&self) -> &PathBuf {
        &self.memory_dir
    }

    pub fn git_add(&self, path: &Path) {
        let _ = Command::new("git")
            .arg("add")
            .arg(path.to_string_lossy().to_string())
            .current_dir(&self.memory_dir)
            .status();
    }

    pub fn git_commit(&self, message: &str) -> Result<()> {
        let output = Command::new("git")
            .args(&["commit", "-m", message])
            .current_dir(&self.memory_dir)
            .output()?;
        if !output.status.success() {
            return Err(anyhow!("Commit failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(())
    }

    pub fn git_has_changes(&self) -> bool {
        let output = match Command::new("git")
            .args(&["status", "--porcelain"])
            .current_dir(&self.memory_dir)
            .output() {
                Ok(o) => o,
                Err(_) => return false,
            };
        !String::from_utf8_lossy(&output.stdout).trim().is_empty()
    }

    pub fn git_stash(&self) -> Result<()> {
        let output = Command::new("git")
            .arg("stash")
            .current_dir(&self.memory_dir)
            .output()?;
        if !output.status.success() {
            return Err(anyhow!("Stash failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(())
    }

    pub fn git_stash_pop(&self) -> Result<()> {
        let output = Command::new("git")
            .args(&["stash", "pop"])
            .current_dir(&self.memory_dir)
            .output()?;
        if !output.status.success() {
            return Err(anyhow!("Stash pop failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        Ok(())
    }
}

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
}
