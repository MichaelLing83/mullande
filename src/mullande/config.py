"""
Configuration management for mullande
"""

import os
import json
import jsonschema
from typing import Optional, Dict, Any, List, Union
from dataclasses import dataclass, asdict
from pathlib import Path

from pydantic import BaseModel, Field, ValidationError
from mullande.workspace import WorkspaceManager


class ModelConfig(BaseModel):
    """Configuration for a single model"""

    provider: str = Field(description="Model provider: ollama, volcengine, copilot")
    model_id: Optional[str] = Field(None, description="Model identifier")
    base_url: Optional[str] = Field(None, description="Base URL for API endpoint")
    context_window: Optional[int] = Field(
        None, ge=1, description="Maximum context window size"
    )
    api_key_env: Optional[str] = Field(
        None, description="Environment variable containing API key"
    )


class ConfigSchema(BaseModel):
    """Root configuration schema"""

    model: ModelConfig = Field(description="Default model configuration")
    models: Optional[Dict[str, ModelConfig]] = Field(
        None, description="Map of model configurations"
    )
    global_context_window: Optional[int] = Field(
        None, ge=1, description="Global default context window size"
    )


@dataclass
class Config:
    """Configuration container with validation and loading"""

    data: ConfigSchema
    config_path: Path

    def __init__(self, data: ConfigSchema, config_path: Path):
        self.data = data
        self.config_path = config_path

    def to_dict(self) -> Dict[str, Any]:
        """Convert configuration to dictionary"""
        return self.data.model_dump()

    def save(self, path: Optional[Union[str, Path]] = None) -> None:
        """Save configuration to JSON file"""
        save_path = Path(path) if path else self.config_path
        save_path.parent.mkdir(parents=True, exist_ok=True)
        with open(save_path, "w") as f:
            json.dump(self.to_dict(), f, indent=2, ensure_ascii=False)

    def get_model_config(self, model_id: Optional[str] = None) -> ModelConfig:
        """Get configuration for specified model or default"""
        if model_id is None:
            return self.data.model

        if self.data.models and model_id in self.data.models:
            # Merge with default config
            default = self.data.model.model_dump()
            model_overrides = self.data.models[model_id].model_dump(exclude_unset=True)
            merged = {**default, **model_overrides}
            return ModelConfig(**merged)

        return self.data.model

    def get_context_window(self, model_id: Optional[str] = None) -> int:
        """Get effective context window for model"""
        model_config = self.get_model_config(model_id)
        if model_config.context_window:
            return model_config.context_window

        if self.data.global_context_window:
            return self.data.global_context_window

        return 4096

    def get_api_key(self, model_id: Optional[str] = None) -> Optional[str]:
        """Get API key from environment variable"""
        model_config = self.get_model_config(model_id)
        if model_config.api_key_env:
            return os.environ.get(model_config.api_key_env)

        # Provider default environment variables
        provider = model_config.provider
        provider_env_map = {
            "volcengine": "VOLCENGINE_API_KEY",
            "copilot": "GITHUB_TOKEN",
            "ollama": None,
        }
        default_env = provider_env_map.get(provider)
        if default_env:
            return os.environ.get(default_env)

        return None

    def __str__(self) -> str:
        """Pretty print configuration"""
        return json.dumps(self.to_dict(), indent=2, ensure_ascii=False)


def get_schema_path() -> Path:
    """Get path to the configuration JSON schema"""
    # Find schema relative to package root
    import mullande

    package_root = Path(mullande.__file__).parent.parent.parent
    return package_root / "config" / "config.schema.json"


def validate_config(config_data: Dict[str, Any]) -> List[str]:
    """Validate configuration against schema, return list of errors"""
    schema_path = get_schema_path()
    if not schema_path.exists():
        return ["Configuration schema not found"]

    with open(schema_path, "r") as f:
        schema = json.load(f)

    try:
        jsonschema.validate(instance=config_data, schema=schema)
        return []
    except jsonschema.ValidationError as e:
        return [str(e)]


def get_config() -> Config:
    """Get current configuration from .mullande/config.json"""
    workspace = WorkspaceManager()
    config_path = workspace.mullande_dir / "config.json"

    if not config_path.exists():
        # Create default configuration
        default_config = ConfigSchema(
            model=ModelConfig(
                provider="ollama", model_id="llama3", base_url="http://localhost:11434"
            ),
            global_context_window=4096,
        )
        config = Config(default_config, config_path)
        config.save()
        return config

    with open(config_path, "r") as f:
        try:
            data = json.load(f)
        except json.JSONDecodeError as e:
            raise ValueError(f"Invalid JSON in {config_path}: {e}")

    try:
        config_data = ConfigSchema(**data)
        return Config(config_data, config_path)
    except ValidationError as e:
        raise ValueError(f"Configuration validation failed: {e}")


def create_config_interactive() -> Config:
    """Create configuration interactively"""
    import click

    click.echo("=== Interactive Configuration Creation ===")
    click.echo(
        "Note: Authentication information (API keys) should be stored in environment variables,"
    )
    click.echo(
        "not in the configuration file. We'll just ask for the environment variable name.\n"
    )

    providers = ["ollama", "volcengine", "copilot"]
    click.echo("Available providers: " + ", ".join(providers))
    while True:
        provider = click.prompt("Default model provider", default="ollama")
        if provider in providers:
            break
        click.echo(f"Invalid provider. Please choose from: {', '.join(providers)}")

    model_id = click.prompt(
        "Default model ID", default="llama3" if provider == "ollama" else ""
    )

    base_url = None
    if provider == "ollama":
        base_url = click.prompt("Ollama base URL", default="http://localhost:11434")

    api_key_env = None
    if provider in ["volcengine", "copilot"]:
        default_env = (
            "VOLCENGINE_API_KEY" if provider == "volcengine" else "GITHUB_TOKEN"
        )
        api_key_env = click.prompt(
            "Environment variable containing API key", default=default_env
        )

    context_window: Optional[int] = None
    if click.confirm(
        "Configure custom context window for default model?", default=False
    ):
        context_window = click.prompt("Context window size", type=int, default=4096)

    global_context_window: Optional[int] = None
    if click.confirm("Configure global default context window?", default=False):
        global_context_window = click.prompt(
            "Global context window size", type=int, default=4096
        )

    default_model = ModelConfig(
        provider=provider,
        model_id=model_id,
        base_url=base_url,
        context_window=context_window,
        api_key_env=api_key_env,
    )

    models: Optional[Dict[str, ModelConfig]] = None
    if click.confirm("Add additional model configurations?", default=False):
        models = {}
        while True:
            model_name = click.prompt("Model ID (enter to stop adding)", default="")
            if not model_name:
                break
            click.echo(f"Configuring {model_name}:")
            while True:
                p = click.prompt(f"Provider for {model_name}", type=str)
                if p in providers:
                    break
                click.echo(
                    f"Invalid provider. Please choose from: {', '.join(providers)}"
                )

            mid: Optional[str] = (
                click.prompt(f"Model ID for {model_name}", default="") or None
            )
            bu: Optional[str] = None
            if p == "ollama":
                bu = click.prompt(
                    f"Base URL for {model_name}", default="http://localhost:11434"
                )

            ake: Optional[str] = None
            if p in ["volcengine", "copilot"]:
                de = "VOLCENGINE_API_KEY" if p == "volcengine" else "GITHUB_TOKEN"
                ake = click.prompt(f"Environment variable with API key", default=de)

            cw: Optional[int] = None
            if click.confirm(f"Custom context window for {model_name}?", default=False):
                cw = click.prompt("Context window size", type=int, default=4096)

            models[model_name] = ModelConfig(
                provider=p,
                model_id=mid,
                base_url=bu,
                context_window=cw,
                api_key_env=ake,
            )

            if not click.confirm("Add another model?", default=False):
                break

    config_data = ConfigSchema(
        model=default_model,
        models=models if models else None,
        global_context_window=global_context_window,
    )

    workspace = WorkspaceManager()
    config_path = workspace.mullande_dir / "config.json"
    config = Config(config_data, config_path)
    config.save()

    return config
