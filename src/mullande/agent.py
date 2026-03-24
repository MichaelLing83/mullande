"""
Core Agent system implementation for mullande
"""

import time
from typing import Optional, List, Dict, Any
from pydantic import BaseModel
import ollama
from rich.console import Console
from rich.table import Table

from mullande import __version__
from mullande.config import get_config, Config, ModelConfig
from mullande.performance import PerformanceCollector


class AgentResponse(BaseModel):
    """Response from the Agent system"""

    content: str
    metadata: Dict[str, Any] = {}


class AgentSystem:
    """Main Agent system for large model interactions"""

    def __init__(self, model: Optional[str] = None):
        """Initialize the Agent system with optional model specification"""
        self.config = get_config()
        self.requested_model = model
        self.model_config = self.config.get_model_config(model)
        self.conversation_history: List[str] = []

    @property
    def effective_model_id(self) -> str:
        """Get effective model ID"""
        if self.requested_model is None:
            # Using default model
            if self.model_config.model_id:
                return self.model_config.model_id
            return "unknown"
        # If model is requested explicitly and model_config has explicit model_id, use that
        # Otherwise (model not configured) use the requested name itself
        if (
            self.config.data.models
            and self.requested_model in self.config.data.models
            and self.config.data.models[self.requested_model].model_id
        ):
            return self.config.data.models[self.requested_model].model_id
        return self.requested_model

    def get_api_key(self) -> Optional[str]:
        """Get API key from environment as configured"""
        return self.config.get_api_key(self.requested_model)

    def get_context_window(self) -> int:
        """Get effective context window size"""
        return self.config.get_context_window(self.requested_model)

    def process(self, input_text: str) -> str:
        """Process input text through the Agent system and return response"""
        self.conversation_history.append(input_text)

        # Get model configuration
        provider = self.model_config.provider
        model_id = self.effective_model_id
        context_window = self.get_context_window()
        api_key = self.get_api_key()

        if provider == "ollama":
            return self._call_ollama(input_text, model_id, context_window)
        elif provider in ["volcengine", "copilot"]:
            # Will implement these providers later
            response = f"Provider {provider} not implemented yet.\n"
            response += f"Configuration:\n"
            response += f"- Provider: {provider}\n"
            response += f"- Model: {model_id}\n"
            response += f"- Context window: {context_window}\n"
            if api_key:
                response += "- API key loaded from environment: ✓\n"
            return response
        else:
            return f"Unknown provider: {provider}"

    def _call_ollama(self, prompt: str, model: str, context_window: int) -> str:
        """Call ollama using official Python API"""
        options = {"num_ctx": context_window} if context_window > 0 else {}
        base_url = self.model_config.base_url

        client_kwargs = {}
        if base_url:
            client_kwargs["host"] = base_url

        api_key = self.get_api_key()
        if api_key:
            client_kwargs["headers"] = {"Authorization": f"Bearer {api_key}"}

        start_time = time.time()
        try:
            if client_kwargs:
                # Create custom client if we have custom options
                client = ollama.Client(**client_kwargs)
                response = client.chat(
                    model=model,
                    messages=[{"role": "user", "content": prompt}],
                    options=options,
                )
            else:
                # Use default client
                response = ollama.chat(
                    model=model,
                    messages=[{"role": "user", "content": prompt}],
                    options=options,
                )

            result = response["message"]["content"]
            duration = time.time() - start_time

            # Record performance data
            collector = PerformanceCollector()
            collector.record_call(model, prompt, result, duration)

            return result
        except Exception as e:
            duration = time.time() - start_time
            return f"Error connecting to ollama: {e}\nPlease ensure ollama is running and the model '{model}' is pulled.\nHint: Run 'ollama pull {model}' to download the model first."

    def start_chat(self) -> None:
        """Start an interactive chat session"""
        # Placeholder for interactive chat implementation
        import readline
        from rich.console import Console
        from rich.table import Table
        from mullande import __version__

        console = Console()
        try:
            while True:
                prompt = input("You > ")
                prompt = prompt.strip()

                # Handle special commands starting with /
                if prompt.startswith("/"):
                    self._handle_special_command(prompt, console)
                    continue

                response = self.process(prompt)
                console.print(f"Agent > {response}")
        except KeyboardInterrupt:
            print("\nExiting chat...")

    def _handle_special_command(self, cmd: str, console) -> None:
        """Handle special chat commands starting with /"""
        parts = cmd.split(maxsplit=1)
        command = parts[0].lower()

        if command == "/models":
            self._cmd_list_models(console)
        elif command == "/model":
            if len(parts) < 2:
                console.print("Agent > Usage: [bold]/model <model_name>[/bold]")
                console.print(
                    f"Agent > Current model: [bold cyan]{self.effective_model_id}[/bold cyan]"
                )
            else:
                self._cmd_switch_model(parts[1], console)
        elif command == "/stats":
            from mullande.performance import PerformanceCollector

            collector = PerformanceCollector()

            # Show system information
            sys_info = collector.get_system_info_cached()
            if sys_info:
                console.print("\n[bold blue]System Information[/bold blue]")
                os_info = f"{sys_info['os']['name']} {sys_info['os']['release']} ({sys_info['os']['architecture']})"
                cpu_info = f"{sys_info['cpu']['physical_cores']} physical / {sys_info['cpu']['logical_cores']} logical cores"
                mem_info = f"{sys_info['memory']['total_gb']} GB total"
                ollama_ver = sys_info.get("ollama_version", "Unknown")

                console.print(f"  OS: {os_info}")
                console.print(f"  CPU: {cpu_info}")
                console.print(f"  Memory: {mem_info}")
                console.print(f"  Ollama version: {ollama_ver}")
                console.print()

            # Get all models with performance data
            models = collector.list_models_with_data()
            if not models:
                console.print(
                    "Agent > [yellow]No performance data collected yet.[/yellow]"
                )
                return

            # Create table
            table = Table(title="Performance Statistics by Model")
            table.add_column("Model", style="cyan")
            table.add_column("Calls", justify="right", style="green")
            table.add_column("Avg Duration", justify="right", style="white")
            table.add_column("Tokens/sec", justify="right", style="magenta")
            table.add_column("Avg Input Chars", justify="right", style="blue")
            table.add_column("Avg Output Chars", justify="right", style="blue")

            total_calls_all = 0
            for model_name in sorted(models):
                stats = collector.get_model_stats(model_name)
                if stats:
                    table.add_row(
                        model_name,
                        str(stats["total_calls"]),
                        f"{stats['avg_duration_seconds']}s",
                        f"{stats['avg_tokens_per_second']}",
                        f"{stats['avg_input_chars']}",
                        f"{stats['avg_output_chars']}",
                    )
                    total_calls_all += stats["total_calls"]

            console.print(table)
            console.print(
                f"\n[bold]Total recorded calls across all models: {total_calls_all}[/bold]"
            )
        elif command == "/version":
            console.print(f"Agent > mullande version: [bold]{__version__}[/bold]")
        elif command == "/config":
            from mullande.config import get_config

            config = get_config()
            console.print("Agent > Current configuration:")
            console.print(str(config))
        else:
            console.print(f"Agent > [red]Unknown command: {command}[/red]")
            console.print(
                "Agent > Available commands: [bold]/models, /model <name>, /stats, /version, /config[/bold]"
            )

    def _cmd_list_models(self, console) -> None:
        """List all configured models"""
        default_model = self.effective_model_id
        models_list = []

        # Add default model
        models_list.append(
            (
                self.config.data.model.model_id,
                self.config.data.model.provider,
                "*default*",
            )
        )

        # Add additional models
        if self.config.data.models:
            for name, model_config in self.config.data.models.items():
                models_list.append((name, model_config.provider, ""))

        table = Table(title="Configured Models")
        table.add_column("Model", style="cyan")
        table.add_column("Provider", style="green")
        table.add_column("Default", style="yellow")

        for name, provider, is_default in sorted(models_list):
            table.add_row(name, provider, is_default)

        console.print(table)
        print(f"\nCurrent active model: [bold cyan]{default_model}[/bold cyan]")

    def _cmd_switch_model(self, model_name: str, console) -> None:
        """Switch to a different model"""
        # Check if model exists in configuration
        if self.config.data.models and model_name in self.config.data.models:
            self.requested_model = model_name
            self.model_config = self.config.get_model_config(model_name)
            console.print(f"✅ Switched to model: [bold cyan]{model_name}[/bold cyan]")
        elif model_name == self.config.data.model.model_id:
            # Already the default
            self.requested_model = None
            self.model_config = self.config.get_model_config(None)
            console.print(
                f"✅ Switched to default model: [bold cyan]{model_name}[/bold cyan]"
            )
        else:
            # Use it even if not explicitly configured
            self.requested_model = model_name
            self.model_config = self.config.get_model_config(model_name)
            console.print(
                f"✅ Switched to model: [bold cyan]{model_name}[/bold cyan] (not in configuration, using default provider settings)"
            )
