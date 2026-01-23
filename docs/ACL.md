# Access Control Lists (ACL) in Ivaldi

Ivaldi uses [Cedar Policy](https://www.cedarpolicy.com/) for fine-grained tool access control.

## ALLOW ALL by Default

By default, Ivaldi operates in an **ALLOW ALL** state. This means that if no policy files are found in your `.ivaldi/policies` directory, all agents and principals are permitted to execute any tool.

This design ensures a smooth out-of-the-box experience while allowing you to lock down specific high-risk tools as needed.

## Disabling specific tools

To restrict access, create a `.cedar` file in your project's `.ivaldi/policies/` directory (e.g., `.ivaldi/policies/restrictions.cedar`) and use `forbid` rules.

### Example: Disabling Command Execution (`exec`)

The most common security requirement is to prevent agents from running arbitrary shell commands. In Ivaldi, shell commands are handled by the `run_command` tool.

To disable this tool entirely, add the following rule to your policy file:

```cedar
forbid(
    principal,
    action == Action::"exec",
    resource
);
```

### Disabling Multiple Tools

You can blacklist multiple tools by checking for a set of actions:

```cedar
forbid(
    principal,
    action in [Action::"exec", Action::"write_file", Action::"edit_file"],
    resource
);
```

## Available Tools (Actions)

When writing Cedar policies for Ivaldi, the following tool names correspond to `Action` IDs:

| Category | Tool Name / Action ID | Description |
|----------|-----------------------|-------------|
| **Navigation** | `find_files` | Search for files matching patterns |
| | `list_dir` | List directory contents and metadata |
| **Observation** | `read_file` | Read a single file |
| | `read_files` | Batch read multiple files |
| | `analyze_dir` | High-level summary of a directory |
| | `analyze_file` | Deep analysis/symbol extraction of a file |
| | `search_code` | AST-aware structural code search |
| | `git_read` | Access git history, diffs, and blame |
| | `read_syslogs` | Read system logs via journald |
| **Mutation** | `write_file` | Create or overwrite files |
| | `edit_file` | Structural AST-based file editing |
| | `edit_files` | Batch structural editing |
| **CLI Proxy** | `exec` (run_command) | Execute shell commands |
| **Recovery** | `undo` | Revert the last operation |
| **Sessions** | `session_init` | Initialize a new stateful session |
| | `session_list` | List active/archived sessions |
| | `session_get` | Retrieve session details |
| | `session_update` | Update session metadata |

## Policy Enforcement Logic

1. **Explicit Forbid**: If any policy contains a `forbid` rule that matches the request, access is **Denied**.
2. **Explicit Permit**: If a policy contains a `permit` rule that matches, access is **Allowed**.
3. **Default Permit**: If no files are present, or no rules match, Ivaldi injects a base `permit(principal, action, resource);` rule.

Since `forbid` always takes precedence over `permit` in Cedar, your custom blacklist rules will always be respected.
