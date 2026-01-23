## GLOBAL CONFIGURATION

These parameters change the running state of the system and can be set via CLI flags or namespaced environment variables.

### Server Options

| Option | Environment Variable | CLI Flag | Description | Default |
|--------|----------------------|----------|-------------|---------|
| `conversation_id` | `IVALDI_CONVERSATION_ID` | `--conversation-id` | Conversation ID for naked/stdio drivers (overrides IDE metadata) | None |
| `conversation_mode` | `IVALDI_CONVERSATION_MODE` | `--conversation-mode` | Conversation mode: persist (default, full tracking) or incognito (ephemeral, no vecdb) | None |
| `api_key` | `IVALDI_API_KEY` | `--api-key` | API Key for authenticated services | None |
| `tool_namespace` | `IVALDI_TOOL_NAMESPACE` | `--tool-namespace` | Tool namespace prefix (helps avoid clashes with other MCP servers) | None |
| `config` | `IVALDI_CONFIG` | `--config` | Path to a custom configuration file | None |
| `exec_sandboxing` | `IVALDI_EXEC_SANDBOXING` | `--exec-sandboxing` | Example: --exec-sandboxing=fs,net | None |
| `transport` | `IVALDI_TRANSPORT` | `--transport` | Transport mode: stdio (default) or http | None |
| `port` | `IVALDI_PORT` | `--port` | Port for HTTP server (default: 8080) | None |

### Core Options

| Option | Environment Variable | CLI Flag | Description | Default |
|--------|----------------------|----------|-------------|---------|
| `api_key` | `IVALDI_API_KEY` | `--api-key ENV: IVALDI_API_KEY` | API Key for authenticated services (e.g. VecDB Cloud) | `false` |
| `config_path` | `IVALDI_CONFIG` | `--config ENV: IVALDI_CONFIG` | Path to a custom configuration file | `false` |
| `enable_gitignore` | `IVALDI_ENABLE_GITIGNORE` | `--enable-gitignore ENV: IVALDI_ENABLE_GITIGNORE` | Whether to evaluate and restrict operations based on .gitignore. | `false` |
| `safety` | `` | `` | Execution safety configuration | `false` |
