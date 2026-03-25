# mullande 开发文档

## 架构概览

mullande 是用 Rust 实现的大模型 Agent 系统，通过 Ollama 调用本地或远程模型。

```
mullande run / chat
       │
       ▼
  AgentSystem::process()
       │
       ├─ tools_enabled=false ──▶ call_ollama()        ──▶ OllamaClient::chat_with_timing()  [流式]
       │
       └─ tools_enabled=true  ──▶ call_ollama_with_tools()  [非流式, agentic loop]
                                       │
                                       ▼
                               send_messages() ──▶ Ollama /api/chat
                                       │
                              ┌────────┴────────┐
                              │  有 tool_calls  │  无 tool_calls
                              ▼                 ▼
                         ToolRegistry        最终回答
                         .execute()
                              │
                         结果追加为
                       role="tool" 消息
                         循环 ≤15次
```

## Prompt 组装策略

每次 `mullande run` 调用时：
1. 从 `.mullande/.memory/CONVERSATIONS.md` 加载历史对话
2. 将历史拼接为带标记的文本：
   ```
   ### User:
   上一轮用户输入

   ### Assistant:
   上一轮模型回答

   ### User:
   本轮输入
   ```
3. 整段文本作为单条 `user` 消息发送给模型

工具调用模式下，历史同样拼接为单条 `user` 消息，后续 `tool` 结果和 `assistant` 工具调用请求作为独立消息追加。

## 模型配置管理

### 配置层级（优先级从高到低）

1. **CLI 参数**：`--temperature`, `--top-k`, `--top-p`, `--presence-penalty`, `--think`/`--no-think`, `--tools`/`--no-tools`
2. **模型专属配置**：`config.json` 中 `models["model-name"]` 的字段
3. **默认模型配置**：`config.json` 中 `model` 字段
4. **Ollama 默认值**：未设置的参数由 Ollama 自行决定

### config.json 结构

```json
{
  "model": {
    "provider": "ollama",
    "model_id": "qwen3:8b",
    "base_url": "http://localhost:11434",
    "context_window": 32768,
    "temperature": 0.7,
    "top_k": 40,
    "top_p": 0.9,
    "presence_penalty": 0.0,
    "thinking": true,
    "tools_enabled": false
  },
  "models": {
    "fast": {
      "provider": "ollama",
      "model_id": "qwen3.5:0.8b",
      "tools_enabled": true
    }
  },
  "global_context_window": 8192
}
```

**注意**：API Key 不要存入配置文件，只存环境变量名：`"api_key_env": "MY_API_KEY"`

## 工具调用实现

### 工作原理

Ollama 支持 OpenAI 兼容的 tool calling 格式。工具定义以 JSON Schema 发送给模型，模型决定是否调用工具并返回结构化的调用请求，代码执行工具后将结果追加为 `role: "tool"` 消息，模型继续生成直到不再调用工具为止。

**关键约束**：Ollama tool calling 要求 `stream: false`，因此工具模式下不使用流式输出。

### 消息序列示意

```
[user]      → "列出 src/ 目录下所有 Rust 文件并统计行数"
[assistant] → tool_calls: [glob({pattern: "**/*.rs", path: "src"})]
[tool]      → "Found 8 file(s): src/rust/main.rs ..."
[assistant] → tool_calls: [bash({command: "wc -l src/**/*.rs"})]
[tool]      → "Exit code: 0\nSTDOUT:\n  42 src/rust/main.rs ..."
[assistant] → "src/ 下共有 8 个 .rs 文件，总计 1,234 行代码。"
```

### 代码文件位置

| 文件 | 职责 |
|------|------|
| `src/rust/tools/mod.rs` | 工具注册表、工具执行逻辑 |
| `src/rust/agent/ollama.rs` | `ToolCall`/`ToolCallFunction` 结构体、`send_messages()` 方法 |
| `src/rust/agent/mod.rs` | `call_ollama_with_tools()` agentic loop |
| `src/rust/config/mod.rs` | `ModelConfig.tools_enabled` 字段 |
| `src/rust/cli/mod.rs` | `--tools` / `--no-tools` CLI 参数 |

### 五个内置工具

#### `read_file`
读取文件内容，可指定行范围。

```
参数:
  path: string          必填，文件路径（相对于当前工作目录）
  start_line: integer   可选，起始行（1-indexed）
  end_line: integer     可选，结束行（含）
```

#### `write_file`
创建或覆写文件，自动创建不存在的父目录。

```
参数:
  path: string     必填，目标文件路径
  content: string  必填，写入内容
```

#### `bash`
执行 shell 命令，返回 stdout + stderr + 退出码。

```
参数:
  command: string  必填，shell 命令（通过 bash -c 执行）
```

#### `glob`
按 glob 模式查找文件，返回排序后的匹配路径列表。

```
参数:
  pattern: string  必填，glob 模式（如 **/*.rs, src/**/*.ts）
  path: string     可选，搜索基础目录（默认为当前工作目录）
```

#### `grep`
在文件内容中正则搜索，优先使用 ripgrep (`rg`)，不可用时 fallback 到系统 `grep`。

```
参数:
  pattern: string          必填，正则表达式
  path: string             可选，搜索目录或文件（默认当前目录）
  glob: string             可选，文件过滤模式（如 *.rs）
  case_insensitive: bool   可选，大小写不敏感（默认 false）
```

### 终端输出格式

```
[tool:1] bash({"command":"cargo test"})        ← 青色，显示工具名和参数
Exit code: 0                                    ← 灰色，结果预览（最多200字符）

► Model: qwen3:8b
► Input tokens: ~512
► Time: 8.43s

测试全部通过，共 12 个测试用例...             ← 最终回答（白色正常输出）
```

## 使用示例

### 基本工具调用

```bash
# 一次性开启工具
mullande run --tools "读取 Cargo.toml，列出所有依赖"
mullande run --tools "src/ 目录下有多少行 Rust 代码？"
mullande run --tools "运行 cargo test 并解释失败原因"

# 指定模型 + 工具
mullande run --model fast --tools "找出所有包含 TODO 的文件"
```

### 在 config.json 中永久启用

```json
{
  "model": {
    "tools_enabled": true
  }
}
```

启用后无需 `--tools`，`mullande run "..."` 默认带工具能力。用 `--no-tools` 临时关闭。

### 优先级示例

```bash
# config.json 设置了 tools_enabled: true
mullande run "..."              # 使用工具（来自 config）
mullande run --no-tools "..."   # 不使用工具（CLI 覆盖）
mullande run --tools "..."      # 使用工具（CLI 显式指定）
```

## 性能指标采集

每次 `mullande run`（非工具模式）都会记录以下指标：

| 指标 | 含义 |
|------|------|
| TTFT | Time To First Token：请求发出到收到第一个 token 的时间 |
| TT | Thinking Time：模型思考阶段总时长 |
| AT | Answering Time：思考结束后生成回答的时长 |
| Think tokens | 思考阶段产生的 token 数（估算） |
| Answer tokens | 回答阶段产生的 token 数（估算） |
| Think speed | 思考 tokens / 思考时间 |
| Ans speed | 回答 tokens / 回答时间 |
| Ans/Total | 回答 tokens / 总时间 |

数据存储于 `.mullande/performance/{model}.jsonl`，用 `mullande stats` 查看汇总统计表。

> **注意**：工具调用模式（`--tools`）使用非流式请求，不采集 TTFT/TT/AT 等流式指标，统计表中该次调用的时序指标为 0。

## 文件位置

| 路径 | 用途 |
|------|------|
| `.mullande/config.json` | 项目级配置 |
| `.mullande/.memory/CONVERSATIONS.md` | 对话历史（git 跟踪） |
| `.mullande/.logs/YYYY-MM-DD.log` | 每日日志汇总 |
| `.mullande/.logs/interactions/` | 每次交互的独立日志文件 |
| `.mullande/performance/{model}.jsonl` | 性能数据（非 git 跟踪） |
| `.mullande/performance/system_info.json` | 本机系统信息 |
| `src/rust/tools/mod.rs` | 工具注册表 |
| `src/rust/agent/ollama.rs` | Ollama API 客户端（含工具调用） |
| `src/rust/agent/mod.rs` | Agent 主逻辑（含 agentic loop） |

