"""
mullande - Large Model Agent System command line interface
"""

import click
import sys
from pathlib import Path
from typing import Optional

from mullande import __version__
from mullande.agent import AgentSystem
from mullande.workspace import WorkspaceManager


@click.group(
    invoke_without_command=True,
    context_settings={"help_option_names": ["-h", "--help"]},
)
@click.version_option(version=__version__)
@click.option(
    "--config", "-c", type=click.Path(exists=True), help="Path to configuration file"
)
@click.pass_context
def main(ctx: click.Context, config: Optional[str]) -> None:
    """
    mullande - A powerful large model Agent system

    This is the main command line interface for interacting with
    the mullande large model Agent system.
    """
    # Initialize workspace - create .mullande/.memory and init git repo
    workspace = WorkspaceManager()
    if not workspace.is_initialized():
        click.echo("Initializing mullande workspace...")
        workspace.initialize()
        click.echo(f"Workspace initialized at {workspace.get_memory_path()}")

    if ctx.invoked_subcommand is None:
        # Default behavior when no subcommand is provided
        click.echo(f"mullande v{__version__} - Large Model Agent System")
        click.echo()
        click.echo("Use 'mullande --help' to see available commands")
        sys.exit(0)


@main.command()
@click.option("--model", "-m", help="Specify the LLM model to use")
@click.option("--prompt", "-p", help="Prompt text to process")
@click.argument("input", required=False)
def run(model: Optional[str], prompt: Optional[str], input: Optional[str]) -> None:
    """Run the Agent system with the given input"""
    agent = AgentSystem(model=model)
    if input:
        # Check if input is a file that exists
        input_path = Path(input)
        if input_path.exists() and input_path.is_file():
            with open(input_path, "r") as f:
                content = f.read()
        else:
            # Treat as direct string input
            content = input
        result = agent.process(content)
        click.echo(result)
    elif prompt:
        result = agent.process(prompt)
        click.echo(result)
    else:
        # Read from stdin
        import sys

        content = sys.stdin.read()
        if content:
            result = agent.process(content)
            click.echo(result)
        else:
            click.echo(
                "Please provide input via argument (file or text), --prompt, or stdin"
            )


@main.command()
def chat() -> None:
    """Start interactive chat session with Agent"""
    click.echo("Starting interactive chat session (Ctrl+C to exit)")
    agent = AgentSystem()
    agent.start_chat()


@main.command()
@click.option("--output", "-o", type=click.Path(), help="Export configuration to file")
@click.option(
    "--check", is_flag=True, help="Validate configuration file against schema"
)
@click.option("--edit", is_flag=True, help="Interactively edit configuration")
@click.option(
    "--import",
    "-i",
    "import_source",
    type=click.Choice(["ollama"]),
    help="Import models from source (currently only ollama)",
)
@click.pass_context
def config(
    ctx: click.Context,
    output: Optional[str],
    check: bool,
    edit: bool,
    import_source: Optional[str],
) -> None:
    """Show, validate, or interactively edit configuration"""
    from mullande.config import get_config, validate_config, create_config_interactive
    import ollama

    if import_source == "ollama":
        config = get_config()
        click.echo("Importing models from local ollama...")

        # Get default provider settings from current config
        default_provider = config.data.model.provider
        default_base_url = config.data.model.base_url
        default_api_key_env = config.data.model.api_key_env

        # List local models
        try:
            models_response = ollama.list()
            # Newer versions of ollama return pydantic model
            if hasattr(models_response, "models"):
                models = models_response.models
            else:
                models = models_response["models"]
        except Exception as e:
            click.echo(f"❌ Failed to list models from ollama: {e}")
            ctx.exit(code=1)

        model_names = []
        for model in models:
            if hasattr(model, "model"):
                # Newer versions use 'model' attribute that includes tag (qwen3.5:latest)
                full_name = getattr(model, "model")
                # Strip the :latest tag if present for cleaner matching
                if full_name.endswith(":latest"):
                    model_names.append(full_name.rsplit(":", 1)[0])
                else:
                    model_names.append(full_name)
            elif hasattr(model, "name"):
                model_names.append(getattr(model, "name"))
            elif "model" in model:
                full_name = model["model"]
                if full_name.endswith(":latest"):
                    model_names.append(full_name.rsplit(":", 1)[0])
                else:
                    model_names.append(full_name)
            elif "name" in model:
                model_names.append(model["name"])
        if not model_names:
            click.echo("⚠️ No models found in local ollama")
            ctx.exit(0)

        click.echo(f"Found {len(model_names)} models locally: {', '.join(model_names)}")

        # Initialize models dict if not exists
        if config.data.models is None:
            config.data.models = {}

        # Count imported vs skipped
        imported = 0
        skipped = 0

        # Add each model to config if not already present
        for model_name in model_names:
            if model_name in config.data.models:
                skipped += 1
                continue

            # Create model configuration with same provider as default
            from mullande.config import ModelConfig

            model_config = ModelConfig(
                provider=default_provider,
                model_id=model_name,
                base_url=default_base_url,
                api_key_env=default_api_key_env,
            )
            config.data.models[model_name] = model_config
            imported += 1

        # Save updated config
        config.save()
        click.echo(f"\n✅ Import complete:")
        click.echo(f"  - Imported: {imported} new models")
        click.echo(f"  - Skipped: {skipped} already existing")
        click.echo(f"  - Total models in config: {len(config.data.models)}")
        return

    if check:
        config = get_config()
        errors = validate_config(config.to_dict())
        if errors:
            click.echo("Configuration validation failed:")
            for error in errors:
                click.echo(f"  - {error}")
            ctx.exit(code=1)
        else:
            click.echo("✅ Configuration is valid")
        return

    if edit or not get_config().config_path.exists():
        if not get_config().config_path.exists():
            click.echo(
                "Configuration file does not exist, starting interactive creation..."
            )
        else:
            click.echo("Starting interactive configuration editing...")

        config = create_config_interactive()
        click.echo(f"\n✅ Configuration saved to {config.config_path}")
        click.echo("\nConfiguration:")
        click.echo(str(config))
        click.echo(
            "\nRemember: API keys should be set in your environment variables, not in this file!"
        )
        return

    config = get_config()
    if output:
        config.save(output)
        click.echo(f"Configuration saved to {output}")
    else:
        click.echo("Current configuration:")
        click.echo(str(config))


@main.command()
def stats() -> None:
    """Show performance statistics for all models"""
    from rich.console import Console
    from rich.table import Table
    from mullande.performance import PerformanceCollector

    console = Console()
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
        console.print("[yellow]No performance data collected yet.[/yellow]")
        console.print("Run some model calls with 'mullande run ...' to collect data.")
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
    console.print("\nData stored in .mullande/.memory/performance/ as JSONL files")


@main.command()
def version() -> None:
    """Show version information"""
    click.echo(f"mullande version {__version__}")


if __name__ == "__main__":
    main()
