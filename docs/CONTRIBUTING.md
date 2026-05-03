# Contributing to Ivaldi-MCP 🤝

We welcome contributions! As part of the Mahal ecosystem, we follow specific engineering and documentation standards.

## 📐 Engineering Principles

1.  **Safety First**: Any new tool that spawns a process or modifies the filesystem must use `ProcessGuard` and the journaling API.
2.  **Machine-Readable Errors**: Never use `anyhow` for top-level tool errors. Add a variant to `IvaldiError` in `ivaldi-core/src/error.rs` and map it to a code.
3.  **Documentation is Code**: Tool argument structs must have rich doc comments (`///`). The `build.rs` script will automatically update the manuals.
4.  **Test Before You Commit**: All logic changes must be accompanied by relevant unit or integration tests.

## 🛠️ Development Workflow

### 1. Environment
- Install `cargo-nextest` and `llvm-cov`.
- Ensure `nproc` is available on your system.

### 2. Implementation
- Add logic to `ivaldi-core`.
- Add the MCP tool definition in `ivaldi-server/src/tools/mod.rs`.
- Add the argument struct to `ivaldi-server/build.rs` to enable documentation generation.

### 3. Feature Completeness Checklist
**A feature is NOT complete until ALL of these are done:**

- [ ] Core implementation (in appropriate ivaldi-core module)
- [ ] MCP handler (in ivaldi-server/src/tools/mod.rs) ← **REQUIRED**
- [ ] Schema registration (in ivaldi-server/build.rs) ← **REQUIRED**
- [ ] Doc comments on args struct
- [ ] Tests pass (`make test`)
- [ ] Verified in live MCP: `tools/list` includes the tool

> **Why this matters**: Agents only have access to what we expose. A feature in core that isn't in MCP doesn't exist for agents.

### 4. Verification
```bash
# Start server in one terminal
cargo run --bin ivaldi-server -- --transport stdio

# In another terminal, check tools are available
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq '.result.tools[].name'
```

### 5. Common Mistakes

| Mistake | Problem | Fix |
|---------|---------|-----|
| Adding to core but not tools/mod.rs | Feature invisible to agents | Always add handler |
| Handler but no build.rs entry | No schema, no docs | Register in build.rs schemas vec |
| Handler returns wrong type | Response malformed | Return `IvaldiResponse<T>` wrapped in `serde_json::to_value` |

## Verification
```bash
make check  # Runs fmt, clippy, and coverage-enforced tests
```

## 📜 Pull Request Process

1.  Update the `CHANGELOG.md` (if exists).
2.  Ensure `make check` passes with 0 warnings and 0 uncovered files.
3.  Include a brief description of how you verified your changes.