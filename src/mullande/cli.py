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
        click.echo("Use 'mullande --help' or 'mullande -h' to see available commands")
        click.echo()
        click.echo("To enable shell autocompletion:")
        click.echo(
            "  Bash:  echo 'eval \"\\$\\(_MULLANDE_COMPLETE=bash_source mullande\\)\"' >> ~/.bashrc"
        )
        click.echo(
            "  Zsh:   echo 'eval \"\\$\\(_MULLANDE_COMPLETE=zsh_source mullande\\)\"' >> ~/.zshrc"
        )
        click.echo(
            "  Fish:  echo '_MULLANDE_COMPLETE=fish_source mullande | source' >> ~/.config/fish/completions/mullande.fish"
        )
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


