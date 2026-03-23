"""
Workspace management for mullande
Handles initialization and management of .mullande/.memory git repository
"""

import os
import subprocess
from pathlib import Path
from typing import Optional


class WorkspaceManager:
    """Manages the mullande workspace and .memory git repository"""

    def __init__(self, root_dir: Optional[Path] = None):
        """Initialize workspace manager with optional root directory"""
        self.root_dir = root_dir or Path.cwd()
        self.mullande_dir = self.root_dir / ".mullande"
        self.memory_dir = self.mullande_dir / ".memory"

    def initialize(self) -> None:
        """Initialize workspace: create directories and initialize git repo if needed"""
        self._create_directories()
        self._init_git_repo()

    def _create_directories(self) -> None:
        """Create the .mullande/.memory directory structure"""
        if not self.mullande_dir.exists():
            self.mullande_dir.mkdir(parents=True, exist_ok=True)

        if not self.memory_dir.exists():
            self.memory_dir.mkdir(parents=True, exist_ok=True)

    def _init_git_repo(self) -> None:
        """Initialize git repository in .memory if it's not already one"""
        if not (self.memory_dir / ".git").exists():
            # Need to initialize git repo
            subprocess.run(
                ["git", "init"], cwd=self.memory_dir, capture_output=True, check=True
            )
            # Create .gitignore to exclude unnecessary files
            gitignore = self.memory_dir / ".gitignore"
            if not gitignore.exists():
                gitignore.write_text("""# OS files
.DS_Store
Thumbs.db

# Temporary files
*.tmp
*.log
""")
                self.git_add(str(gitignore))
                self.git_commit("Initial commit: Add .gitignore")

    def is_initialized(self) -> bool:
        """Check if workspace is already initialized"""
        return (
            self.mullande_dir.exists()
            and self.memory_dir.exists()
            and (self.memory_dir / ".git").exists()
        )

    def get_memory_path(self) -> Path:
        """Get the path to the memory directory"""
        return self.memory_dir

    def git_add(self, path: str) -> None:
        """Add a file to git in the memory repo"""
        subprocess.run(
            ["git", "add", path], cwd=self.memory_dir, capture_output=True, check=True
        )

    def git_commit(self, message: str) -> None:
        """Commit changes in the memory repo"""
        subprocess.run(
            ["git", "commit", "-m", message],
            cwd=self.memory_dir,
            capture_output=True,
            check=True,
        )

    def git_log(self) -> str:
        """Get git log from memory repo"""
        result = subprocess.run(
            ["git", "log", "--oneline"],
            cwd=self.memory_dir,
            capture_output=True,
            text=True,
            check=True,
        )
        return result.stdout
