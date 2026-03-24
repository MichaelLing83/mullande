# OpenCode Work Instructions for mullande Project

## Project Overview
This is a large model Agent system written in Python called `mullande`.
- **Package management**: Uses hatch and uv
- **CLI**: Uses click for command-line interface
- **Main command**: `mullande` with subcommands to be added incrementally

## Coding Guidelines
Follow these conventions:

1. **File Structure**: Standard Python project structure:
   - Source code: `src/mullande/`
   - Tests: `tests/`
   - Configuration: `pyproject.toml`

2. **Code Style**:
   - Use type hints for all functions
   - Keep functions focused and reasonably sized
   - No comments unless explicitly requested
   - Follow existing patterns when adding new code

3. **Testing**:
   - Add tests to `tests/` directory
   - Run tests with `hatch test`
   - Ensure all tests pass before finishing

4. **Git Memory**:
   - All working state stored in `.mullande/.memory` git repository
   - All writes must go through `Memory` API for atomicity
   - Use git for versioning all agent operations

5. **Always**:
   - Run tests after making changes
   - Use existing libraries already in dependencies (click, pydantic, openai, anthropic, rich)
   - Keep API documentation in code through type hints
   - Verify changes with hatch test
   - After completing a task, use add changed and added files to git repo, summarize a commit message and commit

## Current Features Implemented
- ✅ Workspace initialization: `.mullande/.memory` is created on startup and initialized as git repo
- ✅ Memory API with atomic read/write operations: all writes are atomic, rolled back on failure, committed on success
- ✅ CLI main group with `run`, `chat`, `config`, `version` subcommands defined
