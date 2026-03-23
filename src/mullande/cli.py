"""
mullande - Large Model Agent System command line interface
"""
import click
import sys
from typing import Optional

from mullande import __version__
from mullande.agent import AgentSystem


@click.group(invoke_without_command=True)
@click.version_option(version=__version__)
@click.option('--config', '-c', type=click.Path(exists=True), help='Path to configuration file')
@click.pass_context
def main(ctx: click.Context, config: Optional[str]) -> None:
    """
    mullande - A powerful large model Agent system
    
    This is the main command line interface for interacting with
    the mullande large model Agent system.
    """
    if ctx.invoked_subcommand is None:
        # Default behavior when no subcommand is provided
        click.echo(f"mullande v{__version__} - Large Model Agent System")
        click.echo()
        click.echo("Use 'mullande --help' to see available commands")
        sys.exit(0)


@main.command()
@click.option('--model', '-m', help='Specify the LLM model to use')
@click.option('--prompt', '-p', help='Prompt text to process')
@click.argument('input', required=False)
def run(model: Optional[str], prompt: Optional[str], input: Optional[str]) -> None:
    """Run the Agent system with the given input"""
    agent = AgentSystem(model=model)
    if input:
        result = agent.process(input)
        click.echo(result)
    elif prompt:
        result = agent.process(prompt)
        click.echo(result)
    else:
        click.echo("Please provide input via argument or --prompt option")


@main.command()
def chat() -> None:
    """Start interactive chat session with Agent"""
    click.echo("Starting interactive chat session (Ctrl+C to exit)")
    agent = AgentSystem()
    agent.start_chat()


@main.command()
@click.option('--output', '-o', type=click.Path(), help='Export configuration to file')
def config(output: Optional[str]) -> None:
    """Show or export current configuration"""
    from mullande.config import get_config
    config = get_config()
    if output:
        config.save(output)
        click.echo(f"Configuration saved to {output}")
    else:
        click.echo(str(config))


@main.command()
def version() -> None:
    """Show version information"""
    click.echo(f"mullande version {__version__}")


if __name__ == '__main__':
    main()
