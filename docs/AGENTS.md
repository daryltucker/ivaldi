# AGENT INITIALIZATION CHECKLIST

Welcome to **ivaldi-mcp**. Follow this checklist to operate with precision, safety, and correctly prioritized tooling.

## 0. MCP Initialization (Session Setup)

When an MCP client connects to Ivaldi, it sends an `initialize` request. This is optional but recommended:

**What it does:**
- Creates or retrieves a session by ID
- Discovers project root (walks up from CWD looking for `.git/`, `Cargo.toml`, etc.)
- Returns session metadata: `id`, `root`, `project_root`, timestamps, `label`

**Why it matters:**
- Provides context about the project structure
- Enables session-specific state tracking
- Sets up the journal for undo operations

**When to call it:**
- At the start of a new agent session
- When switching to a different project context
- Optional but recommended for full functionality

> **Note**: Ivaldi gracefully handles missing initialization - tools work without it using defaults.

## 1. Exploration (The Radar)
- [ ] **Map**: Use `list_dir(path)` to understand local structure and metadata.
- [ ] **Ingest**: Use `read_file(path)` to explore content.
    - *Tip*: If >1000 lines, use `from_line`/`to_line` for targeted reading.
- [ ] **Batched Ingest**: Use `read_files([paths])` when reading multiple related files to save context.

## 2. Mutation (The Scalpel)
- [ ] **Prefer Structural Edits**: Always try `edit_file(path, content, pattern)` first.
    - *Example*: Use `vecq` patterns like `.functions[] | select(.name == "target")`.
- [ ] **Fallbacks**: Use `grep` or `from_line`/`to_line` only if AST matching is impossible.
- [ ] **Full Overwrites**: Use `write_file(path, content)` only for new files or complete refactors.

### edit_file Selector Guide

| Selector | When to Use | Example |
|----------|-------------|---------|
| `query` (AST) | Editing functions, classes, structured code blocks. You know the symbol name. | `query=".functions[] \| select(.name==\"main\")"` |
| `grep` (regex) | Matching a specific line pattern. **Single-line replacements only**. | `grep="^TODO:"` |
| `from_line/to_line` | You've viewed the file and know the exact range. Non-code files (markdown, config). | `from_line=10, to_line=15` |

> **⚠️ Note**: `grep` replaces exactly ONE matched line. For multi-line edits, use `from_line/to_line` or `query`.

## 3. Deep Analysis (The Microscope)
- [ ] **Project Structure**: Use `analyze_dir` for a high-level recursive summary.
- [ ] **Code Intelligence**: Use `search_code` to find functions, classes, or references.
    - *Friendly Mode*: `category="functions", name_pattern=".*controller"`
    - *Power Mode*: `query=".functions[] | select(.visibility == \"pub\")"`
    - **⚠️ Note**: Complex AST queries may timeout. Use `IVALDI_SEARCH_TIMEOUT=60` for large codebases or complex queries.

## 4. System Observation (The Black Box)
- [ ] **History**: Use `git_read(action="blame|log|diff")` to understand *why* code changed.
- [ ] **Runtime**: Use `read_syslogs` to debug service behavior (if running on Linux systemd).

## 5. The Third Channel (Advisories)
- [ ] **Scan Responses**: Always check the `advisory` array for coaching.
- [ ] **Heeding Warnings**: 
    - `⚠ GitAwareness`: Don't ignore git-ignore warnings unless necessary.
    - `ℹ SyntaxGuard`: If suggested, run `cargo check` immediately after surgery.

    - `ℹ SyntaxGuard`: If suggested, run `cargo check` immediately after surgery.

## 6. Operational Excellence
- [ ] **No Guessing**: Never attempt to write to a path you haven't verified exists or is intended.
- [ ] **Atomic Journaling**: All changes are journaled. Use the `undo` tool (if available) for recovery.
- [ ] **Read the Spec**: If in doubt, consult [MAN_AGENT.md](./MAN_AGENT.md).

---
**Protocol**: precision > volume. Minimize context drift by being surgical.