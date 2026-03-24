# Development Summary: Prompt Management and Assembly in mullande

## Prompt Assembly Strategy

### Current Implementation (ollama provider)

For every `mullande run` or `mullande chat` user input:
1. **Single-turn conversation** - Directly send the user input as a single user message to the model:
   ```python
   messages = [{"role": "user", "content": prompt}]
   ```
   
2. **No conversation history is kept in API calls** - Currently, each `mullande run` invocation is stateless and independent. `self.conversation_history` only keeps a list of input texts in memory for the current process instance, but this is not sent to the model.

3. **In interactive chat (`mullande chat`)**: Each user input is processed as an independent request to the model (single-turn). Full conversation history is not accumulated in the current implementation, but every turn is logged to CONVERSATIONS.md.

### Conversation Logging

- All conversations (both from `mullande run` and `mullande chat`) are **automatically appended** to `.mullande/.memory/CONVERSATIONS.md`
- Format:
  ```markdown
  
  ---
  
  **[timestamp-iso]** Model: `model-name`
  
  **User:** user-prompt-text
  
  **Agent:** agent-response-text
  ```
- Each conversation turn is atomically committed via the Memory API, so all conversation history is version controlled in git
- The file starts with a header explaining what it contains

## Model Configuration Management

### Configuration Hierarchy

1. **Project configuration file**: `.mullande/config.json`
   - `model`: Default model configuration (provider, model_id, base_url, context_window, api_key_env)
   - `models`: Dictionary of additional configured models: `{ "model-name": { ...config... } }`
   - `global_context_window`: Default fallback context window if not specified per model

2. **Runtime model selection**:
   - `mullande run`: `--model model-name` argument overrides default
   - `mullande chat`: `/model model-name` command switches current model
   - When switching to a model not explicitly configured in config.json, it inherits default provider settings (provider, base_url, api_key_env from default model)

3. **Context window resolution**:
   1. Use per-model `context_window` if configured
   2. Fallback to `global_context_window` if configured globally
   3. Final fallback to default 4096

4. **Authentication**:
   - Never store API keys in configuration.json
   - Only store the environment variable name: `api_key_env: "VOLCENGINE_API_KEY"`
   - The API key is read from environment at runtime

## Prompt Management Summary

| Aspect | Current Behavior |
|--------|----------------|
| Multi-turn in `mullande run` | ❌ No (each call is independent) |
| Multi-turn in `mullande chat` | ❌ No (each turn is independent; history not sent to model) |
| Persistent logging of all calls | ✅ Yes → `.mullande/.memory/CONVERSATIONS.md` |
| Multiple model configuration | ✅ Yes → `config.json` `models` dict |
| Dynamic model switching in chat | ✅ Yes → `/model model-name` |
| Environment variable auth | ✅ Yes |
| Context window configuration | ✅ Global + per-model |

## File Locations

| Path | Purpose |
|------|---------|
| `config/config.schema.json` | JSON Schema definition for configuration validation |
| `.mullande/config.json` | Project-level configuration (created on first use) |
| `.mullande/.memory/CONVERSATIONS.md` | Conversation history log |
| `.mullande/.memory/performance/*.jsonl` | Performance data (one file per model) |
| `.mullande/.memory/performance/system_info.json` | Local system information |
