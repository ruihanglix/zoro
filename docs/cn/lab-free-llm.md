# Lab：免费 AI 服务

Zoro 内置了一项 **免费 AI 聚合服务**（Free LLM），它将多家 AI 提供商的免费额度整合在一起，在你的电脑上启动一个**本地 OpenAI 兼容代理服务器**。任何支持 OpenAI API 的工具（Cursor、Continue、ChatBox、Open WebUI 等）都可以连接到这个代理，免费使用 AI 模型 — 无需付费订阅。

## 工作原理

```
+-----------------+      +-----------------------+      +-------------------+
| 你的 AI 工具     |      | Zoro LLM 代理         |      | 免费提供商         |
| (Cursor 等)     | ---> | localhost:29170        | ---> | OpenRouter        |
|                 |      |                       |      | GitHub Models     |
| base_url:       |      | - 智能路由             |      | Groq              |
|  localhost:29170 |      | - 重试与故障转移       |      | Gemini            |
| api_key:        |      | - 健康状态追踪         |      | Mistral           |
|  (任意值)        |      | - Gemini 格式转换      |      | Cerebras          |
+-----------------+      +-----------------------+      | SambaNova         |
                                                        | OpenCode Zen      |
                                                        +-------------------+
```

1. 在一个或多个提供商处注册免费 API Key（每个大约 1 分钟）
2. 在 Zoro 的 **设置 → Lab → Free LLM** 中填入 Key
3. Zoro 会在本地启动代理服务器 `http://localhost:29170`
4. 将任何 OpenAI 兼容工具指向 `http://localhost:29170/v1` — 完成！

代理会自动处理路由、重试、故障转移，甚至自动转换非 OpenAI 格式的 API（如 Google Gemini）。

## 支持的提供商

### 推荐提供商（默认展示）

| 提供商 | 注册链接 | Key 前缀 | API 格式 |
|---|---|---|---|
| **OpenRouter** | [openrouter.ai/keys](https://openrouter.ai/keys) | `sk-or-` | OpenAI |
| **GitHub Models** | [github.com/settings/tokens](https://github.com/settings/personal-access-tokens/new?description=Used+by+Zoro+to+access+GitHub+Models+for+free+AI+inference&name=Zoro+-+GitHub+Models&user_models=read) | `ghp_` | OpenAI |
| **OpenCode Zen** | [opencode.ai/auth](https://opencode.ai/auth) | — | OpenAI |

### 更多提供商（默认折叠）

| 提供商 | 注册链接 | Key 前缀 | API 格式 |
|---|---|---|---|
| **Groq** | [console.groq.com/keys](https://console.groq.com/keys) | `gsk_` | OpenAI |
| **Google Gemini** | [aistudio.google.com](https://aistudio.google.com/app/apikey) | `AIza` | Gemini（自动转换） |
| **Mistral AI** | [console.mistral.ai](https://console.mistral.ai/api-keys) | — | OpenAI |
| **Cerebras** | [cloud.cerebras.ai](https://cloud.cerebras.ai) | — | OpenAI |
| **SambaNova** | [cloud.sambanova.ai](https://cloud.sambanova.ai) | — | OpenAI |

> **提示：** 你不需要配置所有提供商。只需一个 Key 就可以开始使用。配置得越多，你的免费模型池就越丰富、越可靠。

## 快速上手

### 1. 获取免费 API Key

从上表中选择任意提供商并注册免费 API Key。以 **OpenRouter** 为例：

1. 前往 [openrouter.ai/keys](https://openrouter.ai/keys)
2. 使用 Google 或 GitHub 登录
3. 创建一个新的 API Key
4. 复制 Key（以 `sk-or-` 开头）

### 2. 在 Zoro 中启用 Free LLM

1. 打开 Zoro，进入 **设置**
2. 滚动到 **Lab** 区域
3. 开启 **"启用免费 AI 服务"** 开关
4. 将 API Key 粘贴到对应提供商的输入框中
5. 点击 **保存**

Zoro 会自动从该提供商获取可用的免费模型列表。

### 3. 连接你的 AI 工具

配置你的 AI 工具连接 Zoro 代理：

| 设置项 | 值 |
|---|---|
| **API Base URL** | `http://localhost:29170/v1` |
| **API Key** | 任意非空字符串（如 `zoro`） |
| **模型** | `__lab_auto__`（自动路由）或指定模型 ID |

#### 示例：Cursor

在 Cursor 的设置中，添加一个自定义 OpenAI 兼容模型：

- Base URL: `http://localhost:29170/v1`
- API Key: `zoro`
- Model: `__lab_auto__`

#### 示例：Continue (VS Code)

在 `~/.continue/config.yaml` 中添加：

```yaml
models:
  - title: "Zoro Free LLM"
    provider: openai
    apiBase: http://localhost:29170/v1
    apiKey: zoro
    model: __lab_auto__
```

#### 示例：Open WebUI

在 Open WebUI 的管理设置中，添加一个 OpenAI 兼容连接：

- URL: `http://localhost:29170/v1`
- Key: `zoro`

## 路由策略

代理支持三种策略来选择使用哪个提供商和模型：

### 自动（Auto）— 默认推荐

优先尝试请求的模型。如果提供商宕机或触发限流，自动切换到其他健康的提供商。适合大多数用户。

### 轮询（Round-Robin）

在所有健康的提供商之间均匀轮转，每次请求使用不同的模型。适合最大化利用各提供商的免费额度。

### 手动（Manual）

将请求精确发送到指定的模型和提供商。没有自动故障转移 — 如果提供商宕机，请求会直接失败。适合需要确定性模型选择的场景。

你可以在 **设置 → Lab → 路由策略** 中更改策略。

## 模型管理

### 查看可用模型

配置至少一个提供商的 Key 后，Zoro 会获取可用模型列表。你可以在 **设置 → Lab → 可用模型** 中查看所有模型。

### 刷新模型列表

模型列表会缓存 24 小时。如需手动刷新：

- 在 Lab 设置中点击 **"刷新模型"**

### 启用 / 禁用模型

你可以单独开关每个模型。被禁用的模型不会被代理使用，即使在自动或轮询模式下也是如此。

> **注意：** 对于 OpenRouter 和 OpenCode Zen，新发现的模型如果 ID 中不包含 "free"，会被自动禁用。这可以防止意外使用付费模型。

### `__lab_auto__` 虚拟模型

当你在 AI 工具中使用 `__lab_auto__` 作为模型名时，代理会跳过精确模型匹配，转而从所有健康提供商中自动选择可用模型。这是最简单的入门方式 — 你不需要知道具体的模型名称。

## 高级设置

### 代理端口

代理默认监听 **29170** 端口。你可以在 **设置 → Lab** 中修改。

### 局域网访问

默认情况下，代理仅监听 `127.0.0.1`（仅本机可访问）。开启 **局域网访问** 后，代理会监听 `0.0.0.0`，允许局域网内其他设备访问。

启用后，其他设备可以通过 `http://<你的IP>:29170/v1` 连接。

### 访问令牌

启用局域网访问后，你可以设置一个 **访问令牌** 来保护代理。客户端必须在 `Authorization` 请求头中携带该令牌：

```
Authorization: Bearer 你的访问令牌
```

### 重试机制

代理会自动重试失败的请求，最多 3 次，每次切换到不同的提供商/模型：

- **限流 (429)：** 在同一提供商上切换到不同模型重试（分散限流压力）
- **服务器错误 (5xx)：** 切换到完全不同的提供商重试
- **所有重试用尽：** 将错误返回给你的 AI 工具

### 健康状态追踪

代理会追踪每个提供商的健康状态。返回错误的提供商会被临时标记为不健康并从路由中排除。当它们恢复时，会被自动重新检测并恢复。

## 常见问题

### "没有可用的健康提供商"

- 检查是否至少有一个提供商配置了有效的 API Key
- 点击 "刷新模型" 确保模型列表已加载
- 提供商可能暂时宕机 — 等一会儿再试

### 模型没有显示出来

- 确认你的 API Key 是否正确
- 在 设置 → Lab 中点击 "刷新模型"
- 部分提供商需要完成账户验证后才能使用 API

### 端口冲突

如果 29170 端口已被占用，可以在 设置 → Lab 中修改代理端口。

### 局域网设备无法连接

- 确认已在设置中启用 "局域网访问"
- 检查防火墙设置
- 如果设置了访问令牌，确保客户端在 Authorization 请求头中包含了该令牌

## 另请参阅

- [架构概览](../architecture.md) — Zoro 的技术架构
- [MCP 服务器](mcp-server.md) — 将 Zoro 用作 AI Agent 的 MCP 工具服务器
