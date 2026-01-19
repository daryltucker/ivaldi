# MCP Server Configuration

This guide explains how to configure and run the `ivaldi-mcp` server in various environments.

## Quick Reference

| Mode | Use Case | Command |
|------|----------|---------|
| **Native** | Development, Local Filesystem | `cargo run --bin ivaldi-server` |
| **Docker** | Isolated, Production | `docker run daryltucker/ivaldi-mcp` |
| **Gateway** | Docker Desktop / Multi-Agent | `docker mcp run ivaldi` |

---

## 1. Native Execution (Cargo)

Run the server directly on your host machine. This gives the agent full access to your filesystem permissions.

**Prerequisites:**
- Rust/Cargo installed
- `vecq` installed (optional, but recommended for full features)

**Configuration (`mcpservers.json`):**
```json
"ivaldi-native": {
  "command": "cargo",
  "args": [
    "run",
    "--release",
    "--bin", "ivaldi-server",
    "--",
    "--transport", "stdio"
  ],
  "env": {
    "IVALDI_LOG": "info",
    "IVALDI_ROOT": "/your/project/root"
  }
}
```

---

## 2. Docker Execution (Standard)

Run the server in a standard Docker container. This isolates the agent and restricts access to only mounted volumes.

**Prerequisites:**
- Docker installed
- Image built: `make build` (or `docker build -t daryltucker/ivaldi-mcp:latest .`)

**Configuration (`mcpservers.json`):**
```json
"ivaldi-docker": {
  "command": "docker",
  "args": [
    "run",
    "-i",
    "--rm",
    "--env", "IVALDI_LOG=info",
    "--volume", "${HOME}/Projects/:/projects",
    "daryltucker/ivaldi-mcp:latest",
    "--transport", "stdio"
  ]
}
```

> **Note:** The `-i` flag is critical for stdio communication.
---

## 3. Docker MCP Gateway

Use Docker's new MCP Gateway features to orchestrate the server.

**Prerequisites:**
- Docker Desktop with MCP Toolkit enabled (Beta)
- OR `docker-mcp` CLI plugin installed

### Configuration File (`docker-mcp.yaml`)
Create a YAML file to define your servers for the gateway catalog:

```yaml
servers:
  ivaldi:
    image: daryltucker/ivaldi-mcp:latest
    env:
      IVALDI_LOG: "debug"
      IVALDI_ROOT: "/projects"
    volumes:
      - "${HOME}/Projects/:/projects"
```

### Initialization
Initialize the catalog with your configuration:
```bash
docker mcp catalog init --config ./docker-mcp.yaml
```

### Running via Client
Configure your client (e.g., Claude Desktop) to connect to the Gateway:

```json
"ivaldi-gateway": {
  "command": "docker",
  "args": ["mcp", "gateway", "run"]
}
```

The gateway will automatically spin up the `ivaldi` container as needed when tools are requested.
