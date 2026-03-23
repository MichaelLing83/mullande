"""
Configuration management for mullande
"""
from typing import Optional, Dict, Any
from dataclasses import dataclass, asdict
import json
from pathlib import Path


@dataclass
class Config:
    """Configuration for mullande Agent system"""
    default_model: str = "gpt-4"
    api_key: Optional[str] = None
    temperature: float = 0.7
    max_tokens: int = 4096
    system_prompt: str = "You are a helpful assistant."
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert configuration to dictionary"""
        return asdict(self)
    
    def save(self, path: str) -> None:
        """Save configuration to JSON file"""
        with open(path, 'w') as f:
            json.dump(self.to_dict(), f, indent=2)
    
    @classmethod
    def load(cls, path: str) -> 'Config':
        """Load configuration from JSON file"""
        with open(path, 'r') as f:
            data = json.load(f)
        return cls(**data)


def get_config() -> Config:
    """Get current configuration"""
    # In a real implementation, this would load from environment or file
    return Config()
