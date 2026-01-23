# HUMAN MANUAL: ivaldi
Version: 0.1.0

## PHILOSOPHY
`ivaldi` is the scalpel and hammer for file operations. It is designed to be **safe**, **precise**, and **composable**.

While Agents use the MCP server, you (the Human) use the CLI. Both speak the same protocol.

### 3. Smart Hints (The Revolution)
Unlike standard tools that just say "No such file", `ivaldi` tries to help:
-   **Missing File?** It checks the parent directory and lists siblings.
-   **Missing Dir?** It shows you what *is* in the parent.
-   **Collision Prevention?** It **appends** by default and warns you with original line/byte counts for safety.

This "Advisory Channel" (stdinfo) allows Agents to self-correct without crashing.

## COMMANDS


### Radar (`find`)
Find files without the noise.
```bash
ivaldi find src "*.rs"      # Find Rust files (respects .gitignore)
ivaldi find . --depth 2     # Shallow search
```

### Microscope (`search_code`)
Search code structure with AST queries.
```bash
# Find all public functions
ivaldi search src --query '.functions[] | select(.visibility == "pub")'

# Find a specific function
ivaldi search src --category functions --name-pattern ".*main.*"

# Note: Complex queries timeout after 30s. Use IVALDI_SEARCH_TIMEOUT=60 for large codebases
```

### Telescope (`read`)
Read files safely.
```bash
ivaldi read main.rs         # Read file (auto-truncates large files)
ivaldi read main.rs --from 10 --to 20  # Read lines 10-20
ivaldi read binary.bin      # Error: Binary file detected! (Use --force to override)
```

### Sensors (`list`)
See what's around you.
```bash
ivaldi list .               # List current directory
ivaldi list docs -a         # Show hidden files
```

### Hammer & Scalpel (`write`/`edit`)
Surgical control for the cautious.
```bash
# The Hammer: Atomic writes
ivaldi write config.json "{}"        # Defaults to APPEND if exists
ivaldi write log.txt "entry" --force  # Force OVERWRITE
ivaldi write log.txt "entry" --append # Force APPEND

# The Scalpel: Structural edits
ivaldi edit src/lib.rs --query ".functions[] | select(.name == 'main')" --replacement "..."
ivaldi edit config.txt --grep "old-value" --replacement "new-value"

### Time Machine (`undo`)
Made a mistake? Revert it.
```bash
ivaldi undo                 # Revert the last operation (and log the revert)
```
```

## COMPOSABILITY (The Power Move)
`ivaldi` loves `vecq`. Use JSON output (`--json`) to pipe data between them.

### Example 1: Syntax Highlight Found Files
Find all Rust files and syntax-highlight them with `vecq`.
```bash
ivaldi find src "*.rs" --json | jq -r '.result[].path' | xargs vecq syntax
```

### Example 2: Analyze Structure of Large Files
Read a large file (safe truncation) and check its structure elements.
```bash
ivaldi read src/main.rs --json | jq -r '.result.content' | vecq elements rs
```

### Example 3: The "Agent View"
Simulate what the Agent sees (JSON structure).
```bash
ivaldi list . --json | jq .
```
Output:
```json
{
  "status": "success",
  "result": [
    { "name": "Cargo.toml", "type": "file", "size": 1234, ... }
  ],
  "advisory": []
}
```