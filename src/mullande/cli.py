"""
mullande - Large Model Agent System command line interface
"""

import click
import sys
from typing import Optional

from mullande import __version__
from mullande.agent import AgentSystem
from mullande.workspace import WorkspaceManager


@click.group(invoke_without_command=True)
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
        with open(input, "r") as f:
            content = f.read()
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
            click.echo("Please provide input via argument, --prompt, or stdin")


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
@click.pass_context
def config(ctx: click.Context, output: Optional[str], check: bool, edit: bool) -> None:
    """Show, validate, or interactively edit configuration"""
    from mullande.config import get_config, validate_config, create_config_interactive

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
def version() -> None:
    """Show version information"""
    click.echo(f"mullande version {__version__}")


if __name__ == "__main__":
    main()
