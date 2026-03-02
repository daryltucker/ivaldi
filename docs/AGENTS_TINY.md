# PROTOCOL: IVALDI_MCP_V1 (TINY)
# CTX: HIGH_DENSITY_INSTRUCTION_SET

## 1. DISCOVERY (READ-ONLY)
| OP | TOOL | ARGS | RET | COST |
|:---|:---|:---|:---|:---|
| `SCAN` | `analyze_dir` | `path="."`, `depth=N` | `Tree` | LOW |
| `FIND` | `find_files` | `pattern="*.rs"`, `path="."` | `[Path]` | LOW |
| `MAP` | `list_dir` | `path="."` | `[{Meta}]` | LOW |
| `PEEK` | `read_file` | `path`, `from`, `to` | `blob` | MED |
| `Q:AST` | `search_code` | `query=".fn[]|select(.pub)"` | `[Node]` | HIGH |
| `Q:LZ` | `search_code` | `cat="fn"`, `name="main"` | `[Node]` | MED |
| `GIT` | `git_read` | `action="blame|log|diff"` | `GitObj` | MED |

> **⚠️ Note**: AST queries may timeout on large codebases. Use `IVALDI_SEARCH_TIMEOUT=60` for complex searches.
| `LOGS` | `read_syslogs` | `service="foo"`, `level="err"` | `[Log]` | LOW |

## 2. MUTATION (STATE-CHANGE)
| OP | TOOL | ARGS | SAFETY | UNDO? |
|:---|:---|:---|:---|:---|
| `NEW` | `write_file` | `path`, `content`, `overwrite=false` | `Backup` | YES |
| `EDIT` | `edit_file` | `path`, `query` (AST) | `SyntaxGuard` | YES |
| `PATCH` | `edit_file` | `path`, `grep` (Regex) | `LineMatch` | YES |
| `CUT` | `edit_file` | `path`, `from`, `to` | `Range` | YES |
| `OOPS` | `undo` | `path` | `Journal` | N/A |

## 3. HEURISTICS (AUTO-RESPONSE)
`EACCES` -> `PermissionFixer` (Chk `uid`, `mode`)
`ENOENT` -> `SiblingTyposHint` (Did you mean `config.toml`?)
`GIT_IGN` -> `GitAwareness` (WARN: Ignored file)

## 4. CRITICAL PATHS
1. **UNKNOWN_CTX**: `analyze_dir` -> `find_files` -> `read_file`
2. **DEBUG_BUG**: `read_syslogs` -> `git_read(blame)` -> `search_code`
3. **REFACTOR**: `search_code(AST)` -> `read_file` -> `edit_file(AST)` -> `test`
4. **PANIC**: `undo` -> `read_file` (verify state)

## 5. AXIOMS
- **NO_GUESS**: `path` must exist OR you explain why creating.
- **NO_BLIND**: `read_file` before `edit_file`. Always.
- **NO_MAGIC**: `edit_file(grep)` changes ONE line. Use `query` for blocks.
- **SANDBOX**: `tests/` uses `tempfile`. No User `$HOME` Access.
