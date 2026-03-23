# Lab: Free LLM

Zoro includes a built-in **Free LLM** feature that aggregates free-tier AI model access from multiple providers into a single, local **OpenAI-compatible proxy server**. Any tool that supports the OpenAI API (Cursor, Continue, ChatBox, Open WebUI, etc.) can connect to this proxy and use free AI models — no paid subscription required.

## How It Works

```
+-----------------+      +-----------------------+      +-------------------+
| Your AI Tool    |      | Zoro LLM Proxy        |      | Free Providers    |
| (Cursor, etc.)  | ---> | localhost:29170        | ---> | OpenRouter        |
|                 |      |                       |      | GitHub Models     |
| base_url:       |      | - Routing             |      | Groq              |
|  localhost:29170 |      | - Retry & fallback    |      | Gemini            |
| api_key:        |      | - Health tracking     |      | Mistral           |
|  (anything)     |      | - Gemini conversion   |      | Cerebras          |
+-----------------+      +-----------------------+      | SambaNova         |
                                                        | OpenCode Zen      |
                                                        +-------------------+
```

1. You sign up for free API keys from one or more providers (takes ~1 minute each)
2. Enter the keys in Zoro's Settings → Lab → Free LLM
3. Zoro starts a local proxy server on `http://localhost:29170`
4. Point any OpenAI-compatible tool at `http://localhost:29170/v1` — done!

The proxy handles routing, retry, failover, and even on-the-fly conversion for non-OpenAI APIs (like Google Gemini).

## Supported Providers

### Primary Providers (shown by default)

| Provider | Sign-up Link | Key Prefix | API Format |
|---|---|---|---|
| **OpenRouter** | [openrouter.ai/keys](https://openrouter.ai/keys) | `sk-or-` | OpenAI |
| **GitHub Models** | [github.com/settings/tokens](https://github.com/settings/personal-access-tokens/new?description=Used+by+Zoro+to+access+GitHub+Models+for+free+AI+inference&name=Zoro+-+GitHub+Models&user_models=read) | `ghp_` | OpenAI |
| **OpenCode Zen** | [opencode.ai/auth](https://opencode.ai/auth) | — | OpenAI |

### Secondary Providers (collapsed by default)

| Provider | Sign-up Link | Key Prefix | API Format |
|---|---|---|---|
| **Groq** | [console.groq.com/keys](https://console.groq.com/keys) | `gsk_` | OpenAI |
| **Google Gemini** | [aistudio.google.com](https://aistudio.google.com/app/apikey) | `AIza` | Gemini (auto-converted) |
| **Mistral AI** | [console.mistral.ai](https://console.mistral.ai/api-keys) | — | OpenAI |
| **Cerebras** | [cloud.cerebras.ai](https://cloud.cerebras.ai) | — | OpenAI |
| **SambaNova** | [cloud.sambanova.ai](https://cloud.sambanova.ai) | — | OpenAI |

> **Tip:** You don't need to configure all providers. One is enough to get started. The more you add, the more reliable and diverse your free model pool becomes.

## Quick Start

### 1. Get a Free API Key

Pick any provider from the table above and sign up for a free API key. For example, with **OpenRouter**:

1. Go to [openrouter.ai/keys](https://openrouter.ai/keys)
2. Sign in with Google or GitHub
3. Create a new API key
4. Copy the key (starts with `sk-or-`)

### 2. Enable Free LLM in Zoro

1. Open Zoro and go to **Settings**
2. Scroll down to the **Lab** section
3. Toggle **"Enable Free AI Service"** on
4. Paste your API key into the corresponding provider field
5. Click **Save**

Zoro will automatically fetch the list of available free models from the provider.

### 3. Connect Your AI Tool

Configure your AI tool to use the Zoro proxy:

| Setting | Value |
|---|---|
| **API Base URL** | `http://localhost:29170/v1` |
| **API Key** | Any non-empty string (e.g. `zoro`) |
| **Model** | `__lab_auto__` (for automatic routing) or a specific model ID |

#### Example: Cursor

In Cursor's settings, add a custom OpenAI-compatible model:

- Base URL: `http://localhost:29170/v1`
- API Key: `zoro`
- Model: `__lab_auto__`

#### Example: Continue (VS Code)

In your `~/.continue/config.yaml`:

```yaml
models:
  - title: "Zoro Free LLM"
    provider: openai
    apiBase: http://localhost:29170/v1
    apiKey: zoro
    model: __lab_auto__
```

#### Example: Open WebUI

In Open WebUI's admin settings, add an OpenAI-compatible connection:

- URL: `http://localhost:29170/v1`
- Key: `zoro`

## Routing Strategies

The proxy supports three strategies for selecting which provider and model to use:

### Auto (Default)

Tries the requested model first. If the provider is down or rate-limited, automatically falls back to any other healthy provider. Best for most users.

### Round-Robin

Cycles through all healthy providers evenly, rotating the model on each request. Ideal for maximizing free-tier quotas across providers.

### Manual

Sends requests to exactly the specified model and provider. No automatic fallback — if the provider is down, the request fails immediately. Use this if you need deterministic model selection.

You can change the strategy in **Settings → Lab → Routing Strategy**.

## Model Management

### Viewing Available Models

After configuring at least one provider key, Zoro fetches the list of available models. You can view all models in **Settings → Lab → Available Models**.

### Refreshing Model Lists

Model lists are cached for 24 hours. To manually refresh:

- Click **"Refresh Models"** in the Lab settings

### Enabling / Disabling Models

You can toggle individual models on or off. Disabled models will not be used by the proxy, even in Auto or Round-Robin mode.

> **Note:** For OpenRouter and OpenCode Zen, newly discovered models that don't contain "free" in their ID are automatically disabled. This prevents accidental usage of paid models.

### The `__lab_auto__` Virtual Model

When you use `__lab_auto__` as the model name in your AI tool, the proxy skips exact-model matching and instead picks any available model from any healthy provider. This is the easiest way to get started — you don't need to know specific model names.

## Advanced Settings

### Proxy Port

The proxy listens on port **29170** by default. You can change this in **Settings → Lab**.

### LAN Access

By default, the proxy only listens on `127.0.0.1` (local-only). Enable **LAN Access** to listen on `0.0.0.0`, making the proxy accessible from other devices on your local network.

When LAN access is enabled, other devices can connect to `http://<your-ip>:29170/v1`.

### Access Token

When LAN access is enabled, you can set an **Access Token** to protect the proxy. Clients must include the token as a Bearer token in the `Authorization` header:

```
Authorization: Bearer your-access-token
```

### Retry Behavior

The proxy automatically retries failed requests up to 3 times, switching to a different provider/model on each retry:

- **Rate-limited (429):** Retries with a different model on the same provider (to spread across rate-limit buckets)
- **Server error (5xx):** Retries with a different provider entirely
- **All retries exhausted:** Returns the error to your AI tool

### Health Tracking

The proxy tracks the health of each provider. Providers that return errors are temporarily marked as unhealthy and excluded from routing. They are automatically re-checked and restored when they recover.

## Troubleshooting

### "No healthy provider available"

- Check that at least one provider has a valid API key configured
- Click "Refresh Models" to ensure model lists are loaded
- The provider might be temporarily down — wait a moment and try again

### Models not showing up

- Verify your API key is correct
- Click "Refresh Models" in Settings → Lab
- Some providers require account verification before API access is granted

### Port conflict

If port 29170 is already in use, change the proxy port in Settings → Lab.

### LAN clients can't connect

- Ensure "LAN Access" is enabled in Settings
- Check your firewall settings
- If an access token is set, make sure the client includes it in the Authorization header

## See Also

- [Architecture Overview](architecture.md) — Technical architecture of Zoro
- [MCP Server](mcp-server.md) — Using Zoro as an MCP tool server for AI agents
