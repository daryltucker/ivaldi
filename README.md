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
| **Radar** | `find_files` | Glob pattern search (respects .agentignore; .gitignore opt-in). |
| **Map** | `list_dir` | High-fidelity local directory awareness. |
| **Microscope** | `analyze_dir` | Recursive directory structure summary (respects .agentignore). |
| **X-Ray** | `analyze_file` | Deep single-file analysis: symbols, imports, TODOs. |
| **X-Ray** | `search_code` | AST-aware structural code search (jq-style queries). |
| **Telescope** | `read_file` | Focused file reading with blast shields (size limits, binary protection). |
| **Scalpel** | `edit_file` | AST-aware surgical mutations (Rust, Python, etc.). |
| **Hammer** | `write_file` | Atomic, collision-safe file creation and updates. |
| **History** | `git_read` | Read-only access to git history (blame, diff, log). |
| **Diagnostics**| `read_syslogs`| Structured access to systemd-journald logs. |

## 🏗️ Getting Started

### 1. Installation

> ⚠️ **Important:** Always use `--locked` when installing from git. This pins
> dependency versions (including any binary dependencies) to the workspace
> `Cargo.lock`. Without `--locked`, cargo may resolve newer dependencies
> that download incompatible prebuilt binaries.

**Option A: Install via Cargo (Recommended)**
```bash
cargo install --git https://github.com/daryltucker/ivaldi-mcp --locked ivaldi-cli ivaldi-server
```

**Option B: Install individual binaries**
```bash
cargo install --git https://github.com/daryltucker/ivaldi-mcp --locked ivaldi-cli
cargo install --git https://github.com/daryltucker/ivaldi-mcp --locked ivaldi-server
```

**Option C: Using cargo-binstall (Faster)**
```bash
cargo install cargo-binstall --locked
cargo binstall --git https://github.com/daryltucker/ivaldi-mcp ivaldi-cli ivaldi-server
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
        "IVALDI_ENABLE_GITIGNORE": "false"
      }
    }
  }
}
```

> **Tip**: Create a `.agentignore` file in your project root to filter out noise (build artifacts, lockfiles, vendored code). `.agentignore` works like `.gitignore` but is agent-managed and respected by `find_files`, `analyze_dir`, `list_dir`, and `search_code`. Use `respect_agentignore: false` per tool call to bypass it when needed. `.gitignore` is opt-in via `IVALDI_ENABLE_GITIGNORE=true`.

## 📖 Documentation

- [**Agent Manual (Auto-generated)**](docs/MAN_AGENT.json) - Direct schema for agents.
- [**Configuration Reference**](docs/CONFIGURATION.md) - ENV and CLI flag details.
- [**Advisory Catalog**](docs/ADVISORIES.md) - All heuristic advisory messages.
- [**ACL Reference**](docs/ACL.md) - Access control via Cedar policies.
- [**Troubleshooting Guide**](docs/TROUBLESHOOTING.md) - Common issues and solutions.

## 🧪 Testing

The project uses a tiered testing framework. It is **mandatory** to run the complete test suite before any release or major changes.

```bash
# Run the COMPLETE test suite (All tiers, no exceptions - Release Blocker)
make test

# Run Rust-only tests (Unit & Integration)
make test-unit
```

---

> *"The best forge is one that shapes metal without scars. Every cut is intentional."*

— Ivaldi, Agent of Precision Operations
