# mullande

A powerful large model Agent system.

## Features

- Large language model based agent system
- Command line interface powered by click
- Managed with hatch and uv
- Support for multiple LLM providers

## Installation

With uv:
```bash
uv pip install mullande
```

With hatch:
```bash
hatch install
```

From source:
```bash
git clone https://github.com/mullande/mullande.git
cd mullande
uv pip install -e .
```

## Usage

The main command is `mullande`:

```bash
# Show help
mullande --help

# Run a prompt
mullande run --prompt "Your question here"

# Start interactive chat
mullande chat

# Show configuration
mullande config

# Show version
mullande version
```

## Development

This project uses:
- [hatch](https://hatch.pypa.io/) for project management
- [uv](https://github.com/astral-sh/uv) for dependency management
- [click](https://click.palletsprojects.com/) for the command line interface

### Setup development environment

```bash
# Clone the repository
git clone https://github.com/mullande/mullande.git
cd mullande

# Create environment with hatch
hatch env create

# Or with uv
uv venv
source .venv/bin/activate
uv pip install -e .[dev]
```

## License

MIT
