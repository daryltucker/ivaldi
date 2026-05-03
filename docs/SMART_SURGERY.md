# Smart Surgery (Heuristic-Aided Editing)

Ivaldi MCP includes "Smart Surgery" heuristics that bridge the gap between human/agent communicative intent and literal file modifications. These features prevent common "Agent Errors" like data duplication and structural breakage.

## 1. Anchor Overlap Trimming

### The Problem
Agents often include "anchor lines" (surrounding context) in their surgical edits to show they understand the file state. A literal string replacer would double-write these anchors if they were identical to the existing lines outside the replacement range.

### The Solution
When a replacement string is provided, Ivaldi automatically checks the first and last lines of the replacement against the lines immediately preceding and following the target range.

- **Leading Anchor**: If `replacement[0]` matches `line[start-1]`, it is discarded.
- **Trailing Anchor**: If `replacement[last]` matches `line[end+1]`, it is discarded.

**Advisory**: Triggered as `anchor_trimming_leading` or `anchor_trimming_trailing`.

## 2. Indentation Healing (The Bandage)

### The Problem
When editing indented files (YAML, Python, Nested JSON), Agents often provide "naked" snippets (starting at Column 0). Writing these literally breaks the file's structural integrity.

### The Solution (The Bandage)
Ivaldi detects the "Base Whitespace" of the target site. If a replacement is small (**< 100 lines**) and "naked" (all lines start at column 0), Ivaldi prepends the inherited whitespace to every line in the replacement.

#### Rationale for the 100-Line Limit
- **Safety Window**: 100 lines represents a typical "Surgical" block. Auto-indenting a 1,000-line function carry high risk if the Agent's original intent was structural.
- **Agent Responsibility**: Large-scale refactors are "Architectural" work. We assume the Agent is providing a structured payload and we preserve their intended layout literally.
- **Fail-Safe**: If an Agent makes a mistake at 100 lines, it is trivial to fix. At 10,000 lines, auto-correcting could obfuscate the root cause of a structural mismatch.

**Advisory**: Logged as `indentation_healing` at **Info** level.

## 2.1 Indentation Mismatch Detection (Added 2026-04-30)

### The Problem
Agents sometimes provide replacements that are already indented to a different depth than the target site. Previously, if the replacement already had whitespace, the heuristic skipped silently — the agent got no feedback that their indentation didn't match.

### The Solution
When a replacement starts with whitespace but it differs from the target site's indentation, Ivaldi leaves the replacement verbatim (does not re-indent) but triggers an `indentation_mismatch` heuristic. The advisory reports both the replacement's whitespace count and the target's whitespace count.

**Advisory**: Logged as `indentation_mismatch` at **Info** level with repl/target whitespace counts.

## 3. Advisory Philosophy: Transparent Instrumentation
Ivaldi's advisories are not intended as "opinions" or "coaching" from the tool to the Agent. Instead, they serve as **Transparent Instrumentation**.

By disclosing exactly when and why a heuristic was triggered, we provide the Agent with the "Truth of State." This allows the Agent to:
1.  Verify that the tool's "Safe Surgery" aligned with their intent.
2.  Adjust their future editing patterns if they prefer literal, un-bandaged modifications.
3.  Operate with full visibility into the "Third Channel" of tool activity.

Performance and scale have been verified against **10MB+ files** and deep nesting.
