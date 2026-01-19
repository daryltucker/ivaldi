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

### 3. Verification
```bash
make check  # Runs fmt, clippy, and coverage-enforced tests
```

## 📜 Pull Request Process

1.  Update the `CHANGELOG.md` (if exists).
2.  Ensure `make check` passes with 0 warnings and 0 uncovered files.
3.  Include a brief description of how you verified your changes.