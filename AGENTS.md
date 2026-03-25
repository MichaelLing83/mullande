# OpenCode Work Instructions for mullande Project

## Project Overview
This is a large model Agent system **completely rewritten in Rust** called `mullande`.
- **Package management**: Uses cargo
- **CLI**: Uses clap for command-line interface
- **Compilation**: Static linking produces single independent binary (~4.7MB)
- **Release**: Cross-compile support via cross-rs + Docker for macOS/Linux/Windows
- **Output**: Installs to `~/.cargo/bin/mullande`

## Project Structure

```
mullande/
├── src/
│   ├── rust/                # Rust source code
│   │   ├── main.rs          # Entry point
│   │   ├── cli/mod.rs       # CLI commands (run/chat/config/version)
│   │   ├── config/mod.rs    # Configuration management
│   │   ├── workspace/mod.rs # Workspace and git operations
│   │   ├── memory/mod.rs    # .mullande/.memory operations + conversation history
│   │   ├── agent/mod.rs     # Core Agent system
│   │   ├── agent/ollama.rs  # Ollama API client
│   │   ├── performance/     # Performance statistics
│   │   └── logging/mod.rs   # Logging to .mullande/.logs
│   └── mullande/            # Original Python implementation (kept for reference)
├── tests/                   # Tests
└── Cargo.toml               # Rust project configuration
```

## Coding Guidelines

Follow these conventions:

1. **File Structure**:
   - Rust source: `src/rust/` with modules
   - Each module in its own directory
   - Tests in `tests/`

2. **Code Style**:
   - Use type hints for all functions (Rust native)
   - Keep functions focused and reasonably sized
   - No comments unless explicitly requested
   - Follow existing Rust patterns
   - Use existing dependencies already in Cargo.toml

3. **Testing**:
   - Run tests with `cargo test`
   - Ensure all tests pass before finishing

4. **Modules Separation**:
   - `workspace`: Workspace initialization and git operations
   - `memory`: All `.mullande/.memory` operations, conversation history loading/saving
   - `logging`: Logging to `.mullande/.logs/`, daily summary + per-interaction files
   - `agent`: Core Agent system with conversation context

5. **Git Memory**:
   - All working conversation state stored in `.mullande/.memory` git repository
   - All writes must go through `Memory::write_atomic` for atomicity
   - Changes are automatically committed to git after each conversation turn

6. **Logging**:
   - All interactions logged to `.mullande/.logs/`
   - Daily summary: `.mullande/.logs/YYYY-MM-DD.log`
   - Per-interaction: `.mullande/.logs/interactions/interaction_YYYYMMDD_HHMMSS_ffffff.log`
   - Each log includes timestamp, model, user input, full prompt (with history), response

7. **Always**:
   - Run tests after making changes
   - Use existing libraries already in dependencies
   - Keep API documentation in code through types
   - After completing a task:
     1.  Verify all tests pass
     2.  **总结工作内容，将修改、删除、添加的文件commit到代码仓**

## Current Features Implemented

- ✅ Full Rust implementation
- ✅ Workspace initialization: `.mullande/.memory` created and initialized as git repo
- ✅ Memory API with atomic read/write operations: all writes atomic, rolled back on failure
- ✅ Dedicated `memory` module for `.mullande/.memory` operations
- ✅ Persistent conversation history: loaded from `CONVERSATIONS.md` on startup
- ✅ CLI with `run`, `chat`, `config`, `version` subcommands
- ✅ Interactive chat with context (full history included in prompt)
- ✅ Special chat commands: `/models`, `/model`, `/stats`, `/version`, `/config`, `/help`, `/exit`
- ✅ `mullande config --import ollama`: auto-sync models from local ollama
- ✅ `mullande config --edit`: interactive configuration editor
- ✅ `mullande run` output includes header with model name, estimated tokens, total time
- ✅ Ollama API client integration
- ✅ Performance statistics collection and display
- ✅ Full logging: daily summary + per-interaction separate files
- ✅ Cross-compilation release script `test_and_release.sh`
