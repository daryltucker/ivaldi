# ivaldi-mcp 🔍🛡️

**Professional, Safety-First File Operations for AI Agents.**

`ivaldi-mcp` is a robust Model Context Protocol (MCP) server designed for autonomous agents that need to interact with the filesystem. Unlike generic tools, Ivaldi is built with an **RAII-based safety architecture**, **structured error handling**, and **transactional integrity**.

## 🚀 Key Features

- **🛡️ RAII Process Safety**: All subprocesses (Git, Syslogs) are managed via `ProcessGuard` to prevent zombie processes and ensure clean cleanup.
- **🚥 Structured Errors**: Every failure returns a machine-readable code (e.g., `binary_detected`, `file_too_large`) allowing agents to recover programmatically.
- **📜 Transactional Mutability**: Every file write or edit is journaled. The "Scalpel" (`edit_file`) and "Hammer" (`write_file`) tools support multi-file rollback.
- **🏗️ Professional Rust**: Zero-warning build, `Arc<Self>` state management, and 100% test pass-rate with coverage enforcement.

## 🛠️ Tool Suite

| Area | Tool | Description |
| :--- | :--- | :--- |
| **Radar** | `find_files` | Broad, filtered search (respects .gitignore, .aiignore). |
| **Telescope** | `read_file` | Deep focus with blast shields (size limits, binary protection). |
| **Sensors** | `list_dir` | High-fidelity local directory awareness. |
| **Scalpel** | `edit_file` | AST-aware surgical mutations (Rust, Python, etc.). |
| **Hammer** | `write_file` | Atomic, collision-safe file creation and updates. |
| **History** | `git_read` | Read-only access to git history (blame, diff, log). |
| **Diagnostics**| `read_syslogs`| Structured access to systemd-journald logs. |

## 🏗️ Getting Started

### 1. Installation

| Method | speed | Pre-compiled? | Action |
| :--- | :--- | :--- | :--- |
| **Cargo Binstall** | ⚡ Instant | **YES** | `cargo binstall --git https://github.com/daryltucker/ivaldi-mcp ivaldi-cli ivaldi-server` |
| **Direct Download**| ⚡ Instant | **YES** | [Download latest release](https://github.com/daryltucker/ivaldi-mcp/releases) |
| **Cargo Install** | 🐢 Slow | **NO** (Source) | `cargo install --git https://github.com/daryltucker/ivaldi-mcp ivaldi-cli ivaldi-server` |

> [!CAUTION]
> **Compilation vs. Download**
> - `cargo install` **ALWAYS** compiles from source on your machine.
> - `cargo binstall` and **Direct Download** use the pre-compiled binaries produced by GitHub Actions.


### 1. Install

```bash
cargo install --git https://github.com/daryltucker/ivaldi ivaldi-server
```

### 2. Configure

#### Claude Code (User-Global)

```bash
claude mcp add --scope user ivaldi \
  -e IVALDI_ROOT=/home/you/Projects/ \
  -e IVALDI_RESPONSE_MODE=auto \
  -- ivaldi-server
```

#### Claude Desktop
Add this to your `claude_config.json`:

```json
{
  "mcpServers": {
    "ivaldi": {
      "command": "/usr/local/bin/ivaldi-server",
      "args": [],
      "env": {
        "IVALDI_ENABLE_GITIGNORE": "true"
      }
    }
  }
}
```

> It is recommended to create `.vectorignore` and leave `IVALDI_ENABLE_GITIGNORE=false`.


## 📖 Documentation

- [**Architecture Guide**](docs/ARCHITECTURE.md) - Deep dive into state and safety.
- [**Agent Manual (Auto-generated)**](docs/MAN_AGENT.json) - Direct schema for agents.
- [**Configuration Reference**](docs/CONFIGURATION.md) - ENV and CLI flag details.
- [**Testing Standards**](docs/TESTING_STANDARDS.md) - Coverage and stability protocols.
- [**Troubleshooting Guide**](docs/TROUBLESHOOTING.md) - Common issues and solutions.

## 🤝 Mahal Integration
This project is optimized for use within the **Mahal** ecosystem. It provides rich metadata headers and structured JSON responses that Mahal's reasoning engine can optimize against.
