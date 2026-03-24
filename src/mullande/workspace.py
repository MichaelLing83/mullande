"""
Workspace management for mullande
Handles initialization and management of .mullande/.memory git repository
"""

import os
import subprocess
from pathlib import Path
from typing import Optional, Dict, List, Union


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
                ["git", "init"],
                cwd=str(self.memory_dir),
                capture_output=True,
                check=True,
            )
            # Set default user config for git if not set
            subprocess.run(
                ["git", "config", "user.name", "mullande"],
                cwd=str(self.memory_dir),
                capture_output=True,
                check=False,
            )
            subprocess.run(
                ["git", "config", "user.email", "mullande@localhost"],
                cwd=str(self.memory_dir),
                capture_output=True,
                check=False,
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
            ["git", "add", path],
            cwd=str(self.memory_dir),
            capture_output=True,
            check=True,
        )

    def git_commit(self, message: str) -> None:
        """Commit changes in the memory repo"""
        subprocess.run(
            ["git", "commit", "-m", message],
            cwd=str(self.memory_dir),
            capture_output=True,
            check=True,
        )

    def git_log(self) -> str:
        """Get git log from memory repo"""
        result = subprocess.run(
            ["git", "log", "--oneline"],
            cwd=str(self.memory_dir),
            capture_output=True,
            text=True,
            check=True,
        )
        return result.stdout

    def git_stash(self) -> None:
        """Stash current changes"""
        subprocess.run(
            ["git", "stash"],
            cwd=str(self.memory_dir),
            capture_output=True,
            check=True,
        )

    def git_stash_pop(self) -> None:
        """Pop stashed changes"""
        subprocess.run(
            ["git", "stash", "pop"],
            cwd=str(self.memory_dir),
            capture_output=True,
            check=True,
        )

    def git_checkout_head(self) -> None:
        """Checkout HEAD to discard changes"""
        subprocess.run(
            ["git", "checkout", "--", "."],
            cwd=str(self.memory_dir),
            capture_output=True,
            check=True,
        )

    def git_has_changes(self) -> bool:
        """Check if there are uncommitted changes"""
        result = subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=str(self.memory_dir),
            capture_output=True,
            text=True,
            check=True,
        )
        return len(result.stdout.strip()) > 0


class Memory:
    """
    Memory API for atomic read/write operations on .mullande/.memory

    All writes are atomic: either all files are written and committed,
    or if any step fails, all changes are rolled back via git.
    """

    def __init__(self, workspace: Optional[WorkspaceManager] = None):
        """Initialize Memory with workspace manager"""
        self.workspace = workspace or WorkspaceManager()
        self.memory_dir = self.workspace.get_memory_path()

    def _resolve_path(self, path: Union[str, Path]) -> Path:
        """Resolve a relative path to absolute path in memory directory"""
        return self.memory_dir / path

    def read(self, path: Union[str, Path]) -> str:
        """Read a file from memory as text"""
        full_path = self._resolve_path(path)
        if not full_path.exists():
            raise FileNotFoundError(f"File not found in memory: {path}")
        return full_path.read_text()

    def read_bytes(self, path: Union[str, Path]) -> bytes:
        """Read a file from memory as bytes"""
        full_path = self._resolve_path(path)
        if not full_path.exists():
            raise FileNotFoundError(f"File not found in memory: {path}")
        return full_path.read_bytes()

    def exists(self, path: Union[str, Path]) -> bool:
        """Check if a file exists in memory"""
        full_path = self._resolve_path(path)
        return full_path.exists()

    def _get_current_commit(self) -> str:
        """Get current HEAD commit hash"""
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=str(self.memory_dir),
            capture_output=True,
            text=True,
            check=True,
        )
        return result.stdout.strip()

    def write_atomic(
        self, files: Dict[Union[str, Path], Union[str, bytes]], commit_message: str
    ) -> bool:
        """
        Atomically write multiple files.

        Args:
            files: Dictionary mapping file paths to content (str or bytes)
            commit_message: Commit message for git

        Returns:
            True if successful, False if failed
            All changes are rolled back on failure.
        """
        original_head = self._get_current_commit()
        has_changes = self.workspace.git_has_changes()
        if has_changes:
            # Stash existing uncommitted changes
            self.workspace.git_stash()

        try:
            # Write all files
            for path, content in files.items():
                full_path = self._resolve_path(path)
                full_path.parent.mkdir(parents=True, exist_ok=True)
                if isinstance(content, str):
                    full_path.write_text(content)
                else:
                    full_path.write_bytes(content)
                self.workspace.git_add(str(path))

            # Commit all changes
            self.workspace.git_commit(commit_message)
            return True
        except Exception:
            # Rollback all changes to original commit
            subprocess.run(
                ["git", "reset", "--hard", original_head],
                cwd=str(self.memory_dir),
                capture_output=True,
                check=True,
            )
            if has_changes:
                self.workspace.git_stash_pop()
            return False

    def write_one(
        self, path: Union[str, Path], content: Union[str, bytes], commit_message: str
    ) -> bool:
        """Atomically write a single file"""
        return self.write_atomic({path: content}, commit_message)

    def list_files(self) -> List[str]:
        """List all tracked files in memory"""
        result = subprocess.run(
            ["git", "ls-files"],
            cwd=str(self.memory_dir),
            capture_output=True,
            text=True,
            check=True,
        )
        return [line for line in result.stdout.splitlines() if line]

    def get_history(self) -> str:
        """Get commit history"""
        return self.workspace.git_log()
