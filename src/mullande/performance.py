"""
Performance measurement and statistics for mullande
"""

import json
import platform
import time
from datetime import datetime, UTC
from pathlib import Path
from typing import Dict, List, Optional, Any

import psutil

from mullande.workspace import Memory


class PerformanceCollector:
    """Collects and stores performance data for model calls"""

    def __init__(self):
        self.memory = Memory()
        self.perf_dir = "performance"

    def _sanitize_model_name(self, model_name: str) -> str:
        """Convert model name to safe filename"""
        return model_name.replace(":", "_").replace("/", "_").replace("\\", "_")

    def get_system_info(self) -> Dict[str, Any]:
        """Gather system information"""
        # Get CPU info
        cpu_count = psutil.cpu_count(logical=True)
        cpu_physical = psutil.cpu_count(logical=False)
        cpu_freq = psutil.cpu_freq()
        cpu_freq_max = cpu_freq.max if cpu_freq else None

        # Get memory info
        mem = psutil.virtual_memory()
        mem_total_gb = round(mem.total / (1024**3), 2)

        # Get OS info
        os_name = platform.system()
        os_release = platform.release()
        os_version = platform.version()
        arch = platform.machine()
        python_version = platform.python_version()

        # Try to get ollama version
        ollama_version: Optional[str] = None
        try:
            import subprocess

            result = subprocess.run(
                ["ollama", "--version"], capture_output=True, text=True, timeout=5
            )
            if result.returncode == 0:
                parts = result.stdout.strip().split()
                # Output format: "ollama version is 0.18.2"
                if len(parts) >= 4:
                    ollama_version = parts[3]
                elif len(parts) >= 2:
                    # Fallback for other formats
                    ollama_version = parts[-1]
        except Exception:
            pass

        return {
            "captured_at": datetime.now(UTC).isoformat(),
            "os": {
                "name": os_name,
                "release": os_release,
                "version": os_version,
                "architecture": arch,
            },
            "cpu": {
                "logical_cores": cpu_count,
                "physical_cores": cpu_physical,
                "max_frequency_mhz": cpu_freq_max,
            },
            "memory": {"total_gb": mem_total_gb},
            "python_version": python_version,
            "ollama_version": ollama_version,
        }

    def ensure_initialized(self) -> None:
        """Ensure performance directory exists and system info is captured"""
        # Check if system info already exists
        sys_info_path = f"{self.perf_dir}/system_info.json"
        if not self.memory.exists(sys_info_path):
            sys_info = self.get_system_info()
            self.memory.write_one(
                sys_info_path,
                json.dumps(sys_info, indent=2, ensure_ascii=False),
                "Initialize performance tracking: capture system information",
            )

    def record_call(
        self,
        model_name: str,
        input_text: str,
        output_text: str,
        duration_seconds: float,
    ) -> None:
        """Record a single model call performance data"""
        self.ensure_initialized()

        # Estimate tokens (rough estimate: 1 token ≈ 4 characters)
        input_chars = len(input_text)
        output_chars = len(output_text)
        input_tokens_est = input_chars // 4
        output_tokens_est = output_chars // 4

        tokens_per_second = 0.0
        if duration_seconds > 0:
            tokens_per_second = output_tokens_est / duration_seconds

        record = {
            "timestamp": datetime.now(UTC).isoformat(),
            "input_length": {
                "chars": input_chars,
                "tokens_estimated": input_tokens_est,
            },
            "output_length": {
                "chars": output_chars,
                "tokens_estimated": output_tokens_est,
            },
            "duration_seconds": round(duration_seconds, 3),
            "tokens_per_second": round(tokens_per_second, 2),
        }

        # Append to model-specific jsonl file
        safe_name = self._sanitize_model_name(model_name)
        jsonl_path = f"{self.perf_dir}/{safe_name}.jsonl"

        # Read existing content if file exists
        existing_content = ""
        if self.memory.exists(jsonl_path):
            existing_content = self.memory.read(jsonl_path)

        # Append new record
        new_content = existing_content + json.dumps(record, ensure_ascii=False) + "\n"

        # Commit via memory API
        self.memory.write_one(
            jsonl_path,
            new_content,
            f"Record performance data for {model_name}: {output_tokens_est} tokens in {round(duration_seconds, 2)}s",
        )

    def get_model_stats(self, model_name: str) -> Optional[Dict[str, Any]]:
        """Get aggregated statistics for a model"""
        safe_name = self._sanitize_model_name(model_name)
        jsonl_path = f"{self.perf_dir}/{safe_name}.jsonl"

        if not self.memory.exists(jsonl_path):
            return None

        content = self.memory.read(jsonl_path)
        records: List[Dict[str, Any]] = []
        for line in content.strip().split("\n"):
            if line.strip():
                records.append(json.loads(line))

        if not records:
            return None

        # Calculate aggregates
        total_calls = len(records)
        total_duration = sum(r["duration_seconds"] for r in records)
        total_output_tokens = sum(
            r["output_length"]["tokens_estimated"] for r in records
        )
        avg_duration = total_duration / total_calls
        avg_tokens_per_second = (
            sum(r["tokens_per_second"] for r in records) / total_calls
        )
        avg_input_chars = sum(r["input_length"]["chars"] for r in records) / total_calls
        avg_output_chars = (
            sum(r["output_length"]["chars"] for r in records) / total_calls
        )

        return {
            "model_name": model_name,
            "total_calls": total_calls,
            "total_duration_seconds": round(total_duration, 2),
            "avg_duration_seconds": round(avg_duration, 2),
            "avg_tokens_per_second": round(avg_tokens_per_second, 2),
            "avg_input_chars": round(avg_input_chars, 1),
            "avg_output_chars": round(avg_output_chars, 1),
            "total_output_tokens_estimated": total_output_tokens,
        }

    def list_models_with_data(self) -> List[str]:
        """List all models that have performance data"""
        # List all jsonl files in performance directory
        # Since memory is git, we can list tracked files
        jsonl_files = []
        for f in self.memory.list_files():
            if f.startswith(self.perf_dir + "/") and f.endswith(".jsonl"):
                # Convert filename back to model name
                filename = Path(f).name
                if filename.endswith(".jsonl"):
                    model_name = filename[: -len(".jsonl")].replace("_", ":")
                    jsonl_files.append(model_name)
        return jsonl_files

    def get_system_info_cached(self) -> Optional[Dict[str, Any]]:
        """Get cached system information"""
        sys_info_path = f"{self.perf_dir}/system_info.json"
        if not self.memory.exists(sys_info_path):
            return None
        content = self.memory.read(sys_info_path)
        return json.loads(content)
