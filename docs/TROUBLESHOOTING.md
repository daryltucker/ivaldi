# Troubleshooting Guide

This guide helps resolve common issues with `ivaldi-mcp`.

## Search Operations Timeout

### Problem
AST queries time out with: `"Search operation timed out after 30 seconds"`

### Cause
Complex queries across large codebases can exceed the default 30-second timeout, especially with operations like `contains()` on many files.

### Solutions

#### 1. Increase Timeout (Recommended)
```bash
# Increase to 60 seconds for complex searches
IVALDI_SEARCH_TIMEOUT=60 ivaldi-server

# Or set permanently in your MCP config
{
  "mcp": {
    "ivaldi": {
      "environment": {
        "IVALDI_SEARCH_TIMEOUT": "60"
      }
    }
  }
}
```

#### 2. Narrow Search Scope
Instead of searching the entire codebase:
```bash
# Search specific directories
ivaldi_search_code --path src/response --query '.functions[] | select(.name | contains("format"))'

# Use multiple targeted queries
ivaldi_search_code --path src/main.rs --query '.functions[]'
ivaldi_search_code --path src/lib.rs --query '.functions[]'
```

#### 3. Optimize Query Complexity
```bash
# Avoid expensive operations like contains() on large result sets
# ❌ Slow: search entire codebase + filter
ivaldi_search_code --path . --query '.functions[] | select(.name | contains("test"))'

# ✅ Fast: search specific file + simple filter
ivaldi_search_code --path src --query '.functions[] | select(.name == "test_function")'
```

### Prevention
- Use targeted paths instead of `--path .` for large projects
- Break complex queries into smaller, focused searches
- Set appropriate timeouts based on your codebase size

## OpenCode Crashes

### Problem
OpenCode crashes when Ivaldi takes too long to respond to queries.

### Cause
OpenCode's MCP client has strict timeouts and crashes instead of handling them gracefully.

### Solution
The search timeout feature (above) prevents this by ensuring Ivaldi responds within reasonable time limits. If you still experience crashes, reduce the search timeout:

```bash
IVALDI_SEARCH_TIMEOUT=10 ivaldi-server  # Very short timeout
```

## Configuration Issues

### Problem
Environment variables aren't being passed to the MCP server.

### Solution
Always set environment variables in your MCP client configuration, not just in the shell:

```json
{
  "mcp": {
    "ivaldi": {
      "command": "/path/to/ivaldi-server",
      "environment": {
        "IVALDI_ROOT": "/your/project",
        "IVALDI_LOG": "info",
        "IVALDI_SEARCH_TIMEOUT": "60"
      }
    }
  }
}
```

## Performance Tuning

### Large Codebases
```bash
# Increase timeouts and memory limits
IVALDI_SEARCH_TIMEOUT=120
IVALDI_LOG=warn  # Reduce log verbosity
```

### Slow Queries
- Use `find_files` first to narrow scope
- Prefer `category` + `name_pattern` over complex `query`
- Break large searches into smaller batches

## Getting Help

1. Check logs: `IVALDI_LOG=debug ivaldi-server`
2. Test manually: `echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | ivaldi-server`
3. File an issue with: OS, codebase size, query used, and error message</content>
<parameter name="filePath">docs/TROUBLESHOOTING.md