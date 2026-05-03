# Ivaldi Advisory Catalog

This document tracks all formal advisory messages issued by Ivaldi. These messages are designed as **Transparent Instrumentation** to provide Agents with factual state telemetry.

## 1. Surgical Heuristics (Smart Surgery)

Triggered during `edit_file` operations to ensure structural integrity.

| ID | Level | Message | Rationale |
|----|-------|---------|-----------|
| `indentation_healing` | Info | "Surgical content was indented to match the target site's structural depth." | Applied to small edits (<100 lines) that are missing base whitespace but intended for an indented site. |
| `anchor_trimming_leading` | Info | "Leading anchor line detected and removed from replacement string." | Removes a redundant line from the start of the replacement if it matches the line immediately before the edit site. |
| `anchor_trimming_trailing` | Info | "Trailing anchor line detected and removed from replacement string." | Removes a redundant line from the end of the replacement if it matches the line immediately after the edit site. |
| `indentation_mismatch` | Info | "Replacement indentation (N spaces) differs from target site (M spaces). Replacement was NOT re-indented." | Triggered when replacement already has whitespace that doesn't match the target site's indentation. Replacement kept verbatim; agent gets explicit feedback. |
| `grep_multi_line_replacement` | Info | "NOTE: Grep matched 1 line but replacement has multiple lines. Only the matched line was replaced. Use 'from_line'/'to_line' or an AST query to replace multi-line blocks." | Triggered when grep finds exactly 1 match but the replacement spans multiple lines. Guides the agent toward the correct selector. |

## 2. Investigative Heuristics (Crime Scene)

Triggered on operation failure to provide context for self-correction.

| ID | Level | Observation Type | Factual Data Provided |
|----|-------|------------------|-----------------------|
| `permission_fixer` | Warn | `EACCES` (Permission Denied) | UID/GID of process, target metadata, parent directory metadata (including ownership/modes). |
| `sibling_typos` | Info | `NotFound` (File Missing) | List of alternative matches in the same directory using Levenshtein distance. |
| `edit_no_match` | Suggest* | Query failed to match | Available AST targets (functions/structs) or similar lines for grep patterns. |
| `edit_ambiguous` | Suggest* | Query matched multiple | List of match locations (line numbers/signatures) to help with disambiguation. |

> \* Note: Success-path advisories are strictly Info/Warn. Error-path advisories may use "Suggest" level when retrieving community-best-practice resolutions from ADT.

## 3. Environmental Heuristics

Triggered during pre/post-flight checks.

| ID | Level | Observation Type | Factual Data Provided |
|----|-------|------------------|-----------------------|
| `git_awareness` | Info | Version Control Status | Fact about target file's suffix (e.g., `.tmp`) or gitignored status. |
| `syntax_guard` | Info | Language Consistency | Fact that target file is a specific language (e.g., Rust) and that structural validation is external. |
| `smart_collision` | Warn | Write Conflict | Details about existing file size/length when `overwrite=false`. |
| `smart_read_truncation` | Info | Large File Read | Fact that file exceeded 1000 lines and was truncated to head/tail blocks. |

## 4. Usage Guidelines for Developers

When adding a new Advisory:
1.  **Add to this catalog**: Ensure it is documented with a neutral rationale.
2.  **Use Factual Language**: State what was observed and what the tool did.
3.  **Avoid Opinion**: Never tell the agent what they *should* do.
4.  **Tag with Tool ID**: Ensure the heuristic ID is included in the telemetry.
