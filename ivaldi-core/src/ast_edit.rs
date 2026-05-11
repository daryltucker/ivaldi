//! # AST-Based Editing (The Scalpel)
//!
//! ## PURPOSE
//! Provides surgical file editing by targeting AST nodes rather than line numbers.
//!
#![allow(clippy::needless_range_loop)]
//! ## PHILOSOPHY
//! - **Stability**: Nodes are more stable than line numbers.
//! - **Precision**: Target exactly what you mean (e.g., "fn main").
//! - **Polyglot**: Leverages `vecq` to support multiple languages.
//! - **Crime Scene**: On failure, provide rich context for self-correction.

use anyhow::{Result, anyhow};
use vecq::{parse_file, convert_to_json, query_json, FileType};
use serde_json::Value;


use crate::heuristics::edit::{
    NoMatchContext, AmbiguousContext, SelectorType, FileInfo, 
    AvailableTargets, TargetInfo, MatchInfo, ListItemInfo, PartialMatch,
    GrepNoMatchContext, GrepAmbiguousContext,
    levenshtein_distance, find_similar_names, extract_target_name_from_query,
    generate_disambiguation_hints,
};

/// Result of an edit operation with rich error context
pub type RichEditResult = Result<EditOutcome, RichEditError>;

/// Success outcome of an edit operation
#[derive(Debug, Clone)]
pub struct EditOutcome {
    /// Resulting file content after modification
    pub content: String,
    /// List of "Smart Surgery" heuristics that were triggered (e.g., "anchor_trimming")
    pub heuristics_triggered: Vec<String>,
}

/// Rich error types for edit operations with Crime Scene context
#[derive(Debug, thiserror::Error)]
pub enum RichEditError {
    #[error("No nodes matched query")]
    NoMatch(Box<NoMatchContext>),

    #[error("Ambiguous edit: multiple nodes matched query")]
    Ambiguous(Box<AmbiguousContext>),

    #[error("Invalid line range: {0}")]
    InvalidLineRange(String),

    #[error("No line matched grep pattern")]
    GrepNoMatch(Box<GrepNoMatchContext>),

    #[error("Ambiguous edit: multiple lines matched grep pattern")]
    GrepAmbiguous(Box<GrepAmbiguousContext>),

    #[error("Node missing line_start")]
    MissingLineStart,

    #[error("Node missing line_end")]
    MissingLineEnd,

    #[error("Indentation mismatch: replacement has {0} spaces, target has {1} spaces. Write at indent 0 (tool shifts) or at exact target indent ({1} spaces).")]
    IndentationMismatch(usize, usize),


    #[error("Vecq error: {0}")]
    Vecq(#[from] vecq::error::VecqError),

    #[error("Anyhow error: {0}")]
    Anyhow(#[from] anyhow::Error),
}

/// Selection method for editing.
#[derive(Debug, Clone)]
pub enum EditSelector {
    /// Target by vecq query (AST)
    Node(String),
    /// Target by grep pattern
    Grep(String),
    /// Target by exact line range (1-indexed)
    Lines(usize, usize),
}

/// Perform a surgical edit on a file.
pub async fn edit_content(
    content: &str,
    file_type: FileType,
    selector: EditSelector,
    replacement: &str,
) -> RichEditResult {
    match selector {
        EditSelector::Node(query) => {
            edit_node(content, file_type, &query, replacement).await
        }
        EditSelector::Lines(start, end) => {
            edit_lines(content, start, end, replacement, true)
        }
        EditSelector::Grep(pattern) => {
            edit_grep(content, &pattern, replacement)
        }
    }
}

async fn edit_node(
    content: &str,
    file_type: FileType,
    query: &str,
    replacement: &str,
) -> RichEditResult {
    // 1. Parse
    let parsed = parse_file(content, file_type).await?;
    let json = convert_to_json(parsed)?;
    
    // 2. Query
    let results = query_json(&json, query)?;
    
    if results.is_empty() {
        let context = build_no_match_context(query, file_type, content, &json).await?;
        return Err(RichEditError::NoMatch(Box::new(context)));
    }
    if results.len() > 1 {
        let context = build_ambiguous_context(query, file_type, content, &json, &results).await?;
        return Err(RichEditError::Ambiguous(Box::new(context)));
    }
    
    let node = &results[0];
    
    // 3. Extract Line Range
    let line_start = node.get("line_start")
        .and_then(|v: &Value| v.as_u64())
        .ok_or(RichEditError::MissingLineStart)? as usize;
    let line_end = node.get("line_end")
        .and_then(|v: &Value| v.as_u64())
        .ok_or(RichEditError::MissingLineEnd)? as usize;
        
    edit_lines(content, line_start, line_end, replacement, false)
}

/// Build context for no match errors
async fn build_no_match_context(
    query: &str,
    file_type: FileType,
    content: &str,
    json: &Value,
) -> Result<NoMatchContext> {
    let file_info = FileInfo {
        path: "unknown".to_string(), // Will be set by the calling function
        file_type: file_type.to_string(),
        total_lines: content.lines().count(),
        total_bytes: content.len(),
    };

    // Extract target name from query for similarity matching
    let target_name = extract_target_name_from_query(query);
    
    // Build available targets based on file type
    let available_targets = build_available_targets(file_type, json).await?;
    
    // Find similar names
    let similar_names = if let Some(target) = &target_name {
        let candidates = find_candidates_for_file_type(file_type, json).await?;
        let candidate_refs: Vec<(&str, &str)> = candidates.iter()
            .map(|(name, cat)| (name.as_str(), cat.as_str()))
            .collect();
        find_similar_names(target, &candidate_refs, 5)
    } else {
        Vec::new()
    };

    Ok(NoMatchContext {
        query: query.to_string(),
        selector_type: SelectorType::AstQuery,
        file_info,
        available_targets: Some(available_targets),
        similar_names,
        partial_matches: Vec::new(),
    })
}

/// Build context for ambiguous match errors
async fn build_ambiguous_context(
    query: &str,
    file_type: FileType,
    content: &str,
    _json: &Value,
    results: &[Value],
) -> Result<AmbiguousContext> {
    let file_info = FileInfo {
        path: "unknown".to_string(), // Will be set by the calling function
        file_type: file_type.to_string(),
        total_lines: content.lines().count(),
        total_bytes: content.len(),
    };

    // Convert results to match info
    let matches: Vec<MatchInfo> = results.iter()
        .map(|node| {
            let name = node.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            
            let line_start = node.get("line_start")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as usize;
                
            let line_end = node.get("line_end")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as usize;
                
            let signature = node.get("signature")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
                
            let parent = node.get("parent")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
                
            let content_preview = node.get("content")
                .and_then(|v| v.as_str())
                .map(|s| {
                    let mut s = s.to_string();
                    if s.len() > 50 {
                        s.truncate(47);
                        s.push_str("...");
                    }
                    s
                });

            MatchInfo {
                name,
                line_start,
                line_end,
                signature,
                parent,
                content_preview,
            }
        })
        .collect();

    let disambiguation_hints = generate_disambiguation_hints(&matches, query);

    Ok(AmbiguousContext {
        query: query.to_string(),
        selector_type: SelectorType::AstQuery,
        file_info,
        matches,
        disambiguation_hints,
    })
}

/// Build available targets for a file type
async fn build_available_targets(
    file_type: FileType,
    json: &Value,
) -> Result<AvailableTargets> {
    let mut targets = AvailableTargets::default();

    match file_type {
        FileType::Rust | FileType::Go | FileType::Python | FileType::C | FileType::Cpp => {
            // Extract functions
            if let Some(functions) = json.get("functions").and_then(|v| v.as_array()) {
                for func in functions {
                    if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
                        targets.functions.push(TargetInfo {
                            name: name.to_string(),
                            line_start: func.get("line_start")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(1) as usize,
                            visibility: func.get("visibility")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            parent: func.get("parent")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        });
                    }
                }
            }

            // Extract structs/classes
            if let Some(structs) = json.get("structs").or_else(|| json.get("classes")).and_then(|v| v.as_array()) {
                for struct_def in structs {
                    if let Some(name) = struct_def.get("name").and_then(|v| v.as_str()) {
                        targets.structs.push(TargetInfo {
                            name: name.to_string(),
                            line_start: struct_def.get("line_start")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(1) as usize,
                            visibility: struct_def.get("visibility")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            parent: None,
                        });
                    }
                }
            }

            // Extract imports
            if let Some(imports) = json.get("imports").and_then(|v| v.as_array()) {
                for imp in imports {
                    if let Some(name) = imp.get("name").and_then(|v| v.as_str()) {
                        targets.imports.push(name.to_string());
                    }
                }
            }
        }
        
        FileType::Markdown => {
            // Extract headers
            if let Some(headers) = json.get("headers").and_then(|v| v.as_array()) {
                for header in headers {
                    if let Some(content) = header.get("content").and_then(|v| v.as_str()) {
                        targets.headers.push(TargetInfo {
                            name: content.to_string(),
                            line_start: header.get("line_start")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(1) as usize,
                            visibility: None,
                            parent: None,
                        });
                    }
                }
            }

            // Extract list items (including checkboxes)
            if let Some(list_items) = json.get("list_items").and_then(|v| v.as_array()) {
                for item in list_items {
                    if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                        targets.list_items.push(ListItemInfo {
                            content: content.to_string(),
                            line: item.get("line_start")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(1) as usize,
                            task: item.get("task").and_then(|v| v.as_bool()),
                            checked: item.get("checked").and_then(|v| v.as_bool()),
                        });
                    }
                }
            }
        }
        
        _ => {
            // For other file types, provide basic info
            if let Some(elements) = json.get("elements").and_then(|v| v.as_array()) {
                for element in elements {
                    if let Some(name) = element.get("name").and_then(|v| v.as_str()) {
                        targets.functions.push(TargetInfo {
                            name: name.to_string(),
                            line_start: element.get("line_start")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(1) as usize,
                            visibility: None,
                            parent: None,
                        });
                    }
                }
            }
        }
    }

    Ok(targets)
}

/// Find candidate names for similarity matching
async fn find_candidates_for_file_type(
    file_type: FileType,
    json: &Value,
) -> Result<Vec<(String, String)>> {
    let mut candidates = Vec::new();

    match file_type {
        FileType::Rust | FileType::Go | FileType::Python | FileType::C | FileType::Cpp => {
            // Extract function names
            if let Some(functions) = json.get("functions").and_then(|v| v.as_array()) {
                for func in functions {
                    if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
                        candidates.push((name.to_string(), "function".to_string()));
                    }
                }
            }

            // Extract struct/class names
            if let Some(structs) = json.get("structs").or_else(|| json.get("classes")).and_then(|v| v.as_array()) {
                for struct_def in structs {
                    if let Some(name) = struct_def.get("name").and_then(|v| v.as_str()) {
                        candidates.push((name.to_string(), "struct".to_string()));
                    }
                }
            }

            // Extract import names
            if let Some(imports) = json.get("imports").and_then(|v| v.as_array()) {
                for imp in imports {
                    if let Some(name) = imp.get("name").and_then(|v| v.as_str()) {
                        candidates.push((name.to_string(), "import".to_string()));
                    }
                }
            }
        }

        FileType::Markdown => {
            // Extract header names
            if let Some(headers) = json.get("headers").and_then(|v| v.as_array()) {
                for header in headers {
                    if let Some(content) = header.get("content").and_then(|v| v.as_str()) {
                        candidates.push((content.to_string(), "header".to_string()));
                    }
                }
            }

            // Extract list item content
            if let Some(list_items) = json.get("list_items").and_then(|v| v.as_array()) {
                for item in list_items {
                    if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                        candidates.push((content.to_string(), "list_item".to_string()));
                    }
                }
            }
        }

        _ => {
            // Generic element names
            if let Some(elements) = json.get("elements").and_then(|v| v.as_array()) {
                for element in elements {
                    if let Some(name) = element.get("name").and_then(|v| v.as_str()) {
                        candidates.push((name.to_string(), "element".to_string()));
                    }
                }
            }
        }
    }

    Ok(candidates)
}

fn edit_lines(
    content: &str,
    start: usize, // 1-indexed
    end: usize,   // 1-indexed
    replacement: &str,
    is_explicit_range: bool, // true if from_line/to_line was explicitly provided
) -> RichEditResult {
    let has_trailing_newline = content.ends_with('\n');
    let lines: Vec<&str> = content.lines().collect();
    if start < 1 || start > lines.len() + 1 || end < start.saturating_sub(1) || end > lines.len() {
        return Err(RichEditError::InvalidLineRange(
            format!("Invalid line range: {}-{} (total lines: {})", start, end, lines.len())
        ));
    }

    // --- HEURISTIC 1: Indentation Balancing (The Scalpel's Steady Hand) ---
    // Detect "Base Whitespace" holistically from the target range.
    let target_base_ws = if start <= lines.len() {
        // Sample lines in the range to find the intended base indentation level
        let mut target_ws = "";
        for i in (start - 1)..end.min(lines.len()) {
            if !lines[i].trim().is_empty() {
                let ws_count = lines[i].chars().take_while(|c| c.is_whitespace()).count();
                target_ws = &lines[i][..ws_count];
                break;
            }
        }
        // Fallback to the line before if range is empty/whitespace
        if target_ws.is_empty() && start > 1 {
            let ws_count = lines[start - 2].chars().take_while(|c| c.is_whitespace()).count();
            target_ws = &lines[start - 2][..ws_count];
        }
        target_ws
    } else {
        ""
    };

    let mut replacement_lines: Vec<String> = replacement.lines().map(|s| s.to_string()).collect();
    let mut heuristics_triggered = Vec::new();

    if !target_base_ws.is_empty() && !replacement_lines.is_empty() {
        // Find indentation of the first non-empty line of the replacement block
        let replacement_first_ws = replacement_lines.iter()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
            .unwrap_or(0);

        let target_ws_count = target_base_ws.chars().count();
        
        // CASE A: Replacement is "naked" (starts at col 0) — heal by adding target indentation.
        // We allow up to 100 lines for this surgery to handle typical logic blocks.
        if replacement_first_ws == 0 && replacement_lines.len() < 100 && target_ws_count > 0 {
            let shift_by = target_ws_count;
            let extra_ws = if shift_by <= target_ws_count {
                &target_base_ws[..shift_by]
            } else {
                target_base_ws
            };

            for line in &mut replacement_lines {
                if !line.is_empty() {
                    *line = format!("{}{}", extra_ws, line);
                }
            }
            heuristics_triggered.push("indentation_healing".to_string());
        
        // CASE B: Replacement already has indentation that differs from target — return error.
        } else if replacement_first_ws > 0 && replacement_first_ws != target_ws_count {
            // Mode C: R≠0 AND R≠T → Hard error, no apply
            return Err(RichEditError::IndentationMismatch(replacement_first_ws, target_ws_count));
        }
    }

// --- HEURISTIC 2: Anchor Overlap Trimming ---
    // Prevent duplicated lines if the Agent included surrounding context anchors.
    // Compare TRIMMED content to handle whitespace differences between
    // agent replacement and file content.
    
    // Skip anchor detection when explicit line range was provided
    if !is_explicit_range {
    // Trim leading overlaps (compared to line immediately BEFORE start)
    if start > 1 && !replacement_lines.is_empty() {
        let before_line = lines[start - 2];
        let before_trimmed = before_line.trim();
        
        // Check: does the FIRST replacement line match (ignoring whitespace)?
        if replacement_lines[0].trim() == before_trimmed {
            replacement_lines.remove(0);
            heuristics_triggered.push("anchor_trimming_leading".to_string());
        }
    }
    } // end if !is_explicit_range for leading anchor
    
    // Skip if explicit range was provided
    if !is_explicit_range {
    // Trim trailing overlaps (compared to line immediately AFTER end)
    if end < lines.len() && !replacement_lines.is_empty() {
        let after_line = lines[end];
        let after_trimmed = after_line.trim();
        
        // Check: does the LAST replacement line match (ignoring whitespace)?
        if replacement_lines.last().unwrap().trim() == after_trimmed {
            replacement_lines.pop();
            heuristics_triggered.push("anchor_trimming_trailing".to_string());
        }
    } // end if !is_explicit_range for trailing anchor
    }

    let mut new_content = Vec::new();

    // Keep lines before
    for item in lines.iter().take(start - 1) {
        new_content.push(item.to_string());
    }

    // Inject replacement
    for line in replacement_lines {
        new_content.push(line);
    }

    // Keep lines after
    for item in lines.iter().skip(end) {
        new_content.push(item.to_string());
    }

    let mut result = new_content.join("\n");
    if has_trailing_newline {
        result.push('\n');
    }
    Ok(EditOutcome {
        content: result,
        heuristics_triggered,
    })
}

fn edit_grep(
    content: &str,
    pattern: &str,
    replacement: &str,
) -> RichEditResult {
    let regex = regex::Regex::new(pattern)
        .map_err(|e| anyhow!("Invalid regex pattern: {}", e))?;

    let mut matches = Vec::new();
    let mut partial_matches = Vec::new();

    for (i, line) in content.lines().enumerate() {
        if regex.is_match(line) {
            matches.push((i + 1, line.to_string()));
        } else if line.to_lowercase().contains(&pattern.to_lowercase()) {
            // Partial match (case-insensitive substring)
            partial_matches.push(PartialMatch {
                line: i + 1,
                content: line.to_string(),
                match_type: "case_insensitive".to_string(),
            });
        }
    }

    if matches.is_empty() {
        let file_info = FileInfo {
            path: "unknown".to_string(), // Will be set by the calling function
            file_type: "text".to_string(),
            total_lines: content.lines().count(),
            total_bytes: content.len(),
        };

        let context = GrepNoMatchContext {
            pattern: pattern.to_string(),
            file_info,
            partial_matches,
            similar_lines: find_similar_lines(pattern, content),
        };

        return Err(RichEditError::GrepNoMatch(Box::new(context)));
    }

    if matches.len() > 1 {
        let file_info = FileInfo {
            path: "unknown".to_string(), // Will be set by the calling function
            file_type: "text".to_string(),
            total_lines: content.lines().count(),
            total_bytes: content.len(),
        };

        let match_infos: Vec<MatchInfo> = matches.into_iter()
            .map(|(line, content)| MatchInfo {
                name: content.clone(),
                line_start: line,
                line_end: line,
                signature: None,
                parent: None,
                content_preview: Some(content),
            })
            .collect();

        let context = GrepAmbiguousContext {
            pattern: pattern.to_string(),
            file_info,
            matches: match_infos,
        };

        return Err(RichEditError::GrepAmbiguous(Box::new(context)));
    }

    // Single match
    let (line_start, _) = matches[0];
    let line_end = line_start;

    let mut outcome = edit_lines(content, line_start, line_end, replacement, false)?;

    // Heuristic: if grep matched 1 line but replacement is multi-line, note it
    if replacement.lines().count() > 1 {
        outcome.heuristics_triggered.push("grep_multi_line_replacement".to_string());
    }

    Ok(outcome)
}

/// Find lines that might be similar to the pattern
fn find_similar_lines(pattern: &str, content: &str) -> Vec<String> {
    let mut similar = Vec::new();
    let pattern_lower = pattern.to_lowercase();

    for line in content.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains(&pattern_lower) ||
           levenshtein_distance(&pattern_lower, &line_lower) <= 3 {
            similar.push(line.to_string());
            if similar.len() >= 3 {
                break;
            }
        }
    }

    similar
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecq::FileType;

    #[tokio::test]
    async fn test_edit_lines() {
        let content = "line1\nline2\nline3";
        let selector = EditSelector::Lines(2, 2);
        let replacement = "new_line2";
        let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
        assert_eq!(outcome.content, "line1\nnew_line2\nline3");
    }

    #[tokio::test]
    async fn test_edit_grep() {
        let content = "foo = 1\nbar = 2";
        let selector = EditSelector::Grep("bar".to_string());
        let replacement = "bar = 3";
        let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
        assert_eq!(outcome.content, "foo = 1\nbar = 3");
    }

    #[tokio::test]
    async fn test_edit_node_rust() {
        let content = "fn main() {\n    println!(\"old\");\n}";
        let selector = EditSelector::Node(".functions[] | select(.name == \"main\")".to_string());
        let replacement = "fn main() {\n    println!(\"new\");\n}";
        let outcome = edit_content(content, FileType::Rust, selector, replacement).await.unwrap();
        assert!(outcome.content.contains("new"));
        assert!(!outcome.content.contains("old"));
    }

    #[tokio::test]
    async fn test_edit_error_no_grep_match() {
        let content = "foo";
        let selector = EditSelector::Grep("bar".to_string());
        let result = edit_content(content, FileType::Text, selector, "baz").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_anchor_trimming() {
        let content = "line1\nline2\nline3\nline4\nline5";
        let selector = EditSelector::Lines(3, 3);
        // Agent includes line2 and line4 as anchors
        let replacement = "line2\nNEW_LINE3\nline4";
        let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
        // When using explicit from_line/to_line (EditSelector::Lines),
        // anchor detection should be DISABLED.
        // So "line2" and "line4" should appear TWICE (duplicated)
        assert!(outcome.content.contains("line2\nline2"));
        assert!(outcome.content.contains("line4\nline4"));
        // Should NOT have anchor_trimming heuristics
        assert!(!outcome.heuristics_triggered.contains(&"anchor_trimming_leading".to_string()));
        assert!(!outcome.heuristics_triggered.contains(&"anchor_trimming_trailing".to_string()));
    }

    #[tokio::test]
    async fn test_indentation_healing() {
        let content = "parent:\n    child: old";
        let selector = EditSelector::Lines(2, 2);
        // Agent provides "naked" replacement
        let replacement = "child: new";
        let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
        // Indentation (4 spaces) should be inherited
        assert_eq!(outcome.content, "parent:\n    child: new");
        assert!(outcome.heuristics_triggered.contains(&"indentation_healing".to_string()));
    }

    #[tokio::test]
    async fn test_no_healing_for_large_blocks() {
        let content = "parent:\n    child: old";
        let selector = EditSelector::Lines(2, 2);
        // Replacement is many lines - we assume agent knows what they are doing
        // Using > 100 lines to avoid the heuristic
        let mut replacement = String::new();
        for i in 0..110 {
            replacement.push_str(&format!("line{}\n", i));
        }
        let outcome = edit_content(content, FileType::Text, selector, &replacement).await.unwrap();
        // Should NOT be indented
        assert!(outcome.content.contains("parent:\nline0"));
        assert!(!outcome.heuristics_triggered.contains(&"indentation_healing".to_string()));
    }

    #[tokio::test]
    async fn test_indentation_healing_tabs() {
        let content = "parent:\n\tchild: old";
        let selector = EditSelector::Lines(2, 2);
        // Agent provides "naked" replacement
        let replacement = "child: new";
        let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
        // Tab indentation should be inherited
        assert_eq!(outcome.content, "parent:\n\tchild: new");
        assert!(outcome.heuristics_triggered.contains(&"indentation_healing".to_string()));
    }

    #[tokio::test]
    async fn test_indentation_healing_multiline() {
        let content = "def my_func():\n    # original code\n    pass";
        let selector = EditSelector::Lines(2, 3);
        // Agent provides "naked" multi-line replacement
        let replacement = "print('hello')\nreturn True";
        let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
        // Every line should get the 4-space indentation
        assert_eq!(outcome.content, "def my_func():\n    print('hello')\n    return True");
        assert!(outcome.heuristics_triggered.contains(&"indentation_healing".to_string()));
    }

    #[tokio::test]
    async fn test_no_double_indentation() {
        let content = "parent:\n    child: old";
        let selector = EditSelector::Lines(2, 2);
        // Agent ALREADY provided indentation
        let replacement = "    child: new";
        let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
        // Should NOT double up to 8 spaces
        assert_eq!(outcome.content, "parent:\n    child: new");
        assert!(!outcome.heuristics_triggered.contains(&"indentation_healing".to_string()));
    }

    #[tokio::test]
    async fn test_no_healing_for_mixed_indentation_with_proper_first_line() {
        let content = "def test():\n    return (\n        True\n    )";
        let selector = EditSelector::Lines(3, 4);
        // Agent correctly indents the first line (8 spaces), but subsequent lines have col 0 (like a top-level var)
        let replacement = "        False\n    )\n\nTOP_LEVEL = 1";
        let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
        // Since first line of replacement is indented, it shouldn't shift everything by target base (which is 8)
        assert_eq!(outcome.content, "def test():\n    return (\n        False\n    )\n\nTOP_LEVEL = 1");
        assert!(!outcome.heuristics_triggered.contains(&"indentation_healing".to_string()));
    }
}
