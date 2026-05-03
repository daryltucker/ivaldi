//! # Toggle Checkbox Tool
//!
//! ## PURPOSE
//! Semantic checkbox toggling in Markdown files - the "What Not How" philosophy.
//!
//! ## PHILOSOPHY
//! Instead of: "Replace line 15 with '- [x] Task Name'"
//! We do: "Mark the 'Task Name' checkbox as completed"
//!
//! ## USAGE
//! toggle_checkbox(path, match="implement caching", state=true)
//! - Finds checkbox items containing "implement caching"
//! - Toggles [ ] → [x] or [x] → [ ]
//! - Provides rich success/failure context

use serde::{Deserialize, Serialize};
use serde_json::Value;
use vecq::{parse_file, convert_to_json, FileType};
use crate::IvaldiResponse;
use crate::error::IvaldiError;
use crate::undo::Journal;
use super::types::EditFileArgs;
use super::edit::edit_file;

/// Arguments for toggle_checkbox tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToggleCheckboxArgs {
    /// Path to the Markdown file
    pub path: std::path::PathBuf,
    /// Pattern to match checkbox content
    pub pattern: String,
    /// Desired state: true (checked), false (unchecked), or "toggle"
    pub state: CheckboxState,
}

/// Checkbox states
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckboxState {
    /// Mark as checked ([x])
    True,
    /// Mark as unchecked ([ ])
    False,
    /// Toggle current state
    Toggle,
}

/// Result of finding a checkbox
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckboxMatch {
    pub line: usize,
    pub content: String,
    pub current_state: bool,
    pub full_line: String,
}

/// Toggle checkbox in Markdown file
pub async fn toggle_checkbox(
    root: &std::path::Path,
    args: ToggleCheckboxArgs,
    journal: &Journal,
) -> IvaldiResponse<CheckboxResult> {
    // 1. Read and parse the file
    let content = match std::fs::read_to_string(&args.path) {
        Ok(c) => c,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Io(e)),
    };

    // 2. Parse as Markdown and find checkboxes
    let file_type = FileType::Markdown;
    let parsed = match parse_file(&content, file_type).await {
        Ok(p) => p,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Internal(format!("Failed to parse Markdown: {}", e))),
    };

    let json = match convert_to_json(parsed) {
        Ok(j) => j,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Internal(format!("Failed to convert to JSON: {}", e))),
    };

    // 3. Find matching checkboxes
    let matches = match find_matching_checkboxes(&content, &json, &args.pattern) {
        Ok(m) => m,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Internal(format!("Failed to search checkboxes: {}", e))),
    };

    if matches.is_empty() {
        // No matches - provide helpful context
        let available_checkboxes = get_all_checkboxes(&content, &json).unwrap_or_default();
        return IvaldiResponse::from_error(IvaldiError::Query("No checkbox matched pattern".into()))
            .with_advisory(crate::advisory::AdvisoryMessage::adt_suggest(
                serde_json::json!({
                    "pattern": args.pattern,
                    "available_checkboxes": available_checkboxes.iter().map(|cb| {
                        serde_json::json!({
                            "content": cb.content,
                            "checked": cb.current_state,
                            "line": cb.line
                        })
                    }).collect::<Vec<_>>(),
                    "suggestions": find_similar_checkboxes(&args.pattern, &available_checkboxes)
                }),
                format!("Found {} checkboxes. Check your pattern or see available options.", available_checkboxes.len())
            ));
    }

    if matches.len() > 1 {
        // Multiple matches - ambiguous
        return IvaldiResponse::from_error(IvaldiError::Query("Multiple checkboxes matched pattern".into()))
            .with_advisory(crate::advisory::AdvisoryMessage::adt_suggest(
                serde_json::json!({
                    "pattern": args.pattern,
                    "matches": matches.iter().map(|m| {
                        serde_json::json!({
                            "content": m.content,
                            "checked": m.current_state,
                            "line": m.line
                        })
                    }).collect::<Vec<_>>(),
                    "count": matches.len()
                }),
                "Use a more specific pattern to match exactly one checkbox".to_string()
            ));
    }

    // 4. We have exactly one match - perform the toggle
    let checkbox = &matches[0];
    let target_state = match args.state {
        CheckboxState::True => true,
        CheckboxState::False => false,
        CheckboxState::Toggle => !checkbox.current_state,
    };

    // 5. Generate the replacement
    let replacement_line = generate_checkbox_line(&checkbox.full_line, target_state);
    let original_line = checkbox.full_line.clone();

    // 6. Perform the edit using our existing edit_file infrastructure
    let edit_args = EditFileArgs {
        path: args.path.clone(),
        query: None,
        grep: Some(format!("^{}$", regex::escape(original_line.trim()))),
        replacement: replacement_line.clone(),
        from_line: None,
        to_line: None,
        preview: false,
    };

    let edit_result = edit_file(root, edit_args, journal).await;

    if edit_result.is_error {
        // Edit failed - convert the error response to our return type
        IvaldiResponse {
            is_error: true,
            content: None,
            ui_diffs: Vec::new(),
            advisory: edit_result.advisory,
            error: edit_result.error,
        }
    } else {
        // Success! Return detailed result
        let all_checkboxes = get_all_checkboxes(&content, &json).unwrap_or_default();

        let result = CheckboxResult {
            path: args.path,
            pattern: args.pattern,
            matched_checkbox: checkbox.clone(),
            previous_state: checkbox.current_state,
            new_state: target_state,
            line_changed: checkbox.line,
            total_checkboxes: all_checkboxes.len(),
            checked_count: all_checkboxes.iter().filter(|cb| cb.current_state).count(),
        };

        IvaldiResponse::success(result)
    }
}

/// Result of a successful checkbox toggle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckboxResult {
    pub path: std::path::PathBuf,
    pub pattern: String,
    pub matched_checkbox: CheckboxMatch,
    pub previous_state: bool,
    pub new_state: bool,
    pub line_changed: usize,
    pub total_checkboxes: usize,
    pub checked_count: usize,
}

/// Find checkboxes matching the pattern
fn find_matching_checkboxes(content: &str, json: &Value, pattern: &str) -> Result<Vec<CheckboxMatch>, String> {
    let mut matches = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    if let Some(list_items) = json.get("list_items").and_then(|v| v.as_array()) {
        for item in list_items {
            if let (Some(content_text), Some(task), Some(checked), Some(line_start)) = (
                item.get("content").and_then(|v| v.as_str()),
                item.get("task").and_then(|v| v.as_bool()),
                item.get("checked").and_then(|v| v.as_bool()),
                item.get("line_start").and_then(|v| v.as_u64()),
            ) && task && content_text.to_lowercase().contains(&pattern.to_lowercase()) {
                let line_idx = (line_start as usize).saturating_sub(1); // Convert to 0-based
                if line_idx < lines.len() {
                    let full_line = lines[line_idx].to_string();

                    matches.push(CheckboxMatch {
                        line: line_start as usize,
                        content: content_text.to_string(),
                        current_state: checked,
                        full_line,
                    });
                }
            }
        }
    }

    Ok(matches)
}

/// Get all checkboxes in the document
fn get_all_checkboxes(content: &str, json: &Value) -> Result<Vec<CheckboxMatch>, String> {
    let mut checkboxes = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    if let Some(list_items) = json.get("list_items").and_then(|v| v.as_array()) {
        for item in list_items {
            if let (Some(content_text), Some(task), Some(checked), Some(line_start)) = (
                item.get("content").and_then(|v| v.as_str()),
                item.get("task").and_then(|v| v.as_bool()),
                item.get("checked").and_then(|v| v.as_bool()),
                item.get("line_start").and_then(|v| v.as_u64()),
            ) && task {
                let line_idx = (line_start as usize).saturating_sub(1); // Convert to 0-based
                let full_line = if line_idx < lines.len() {
                    lines[line_idx].to_string()
                } else {
                    format!("- [{}] {}", if checked { "x" } else { " " }, content_text)
                };

                checkboxes.push(CheckboxMatch {
                    line: line_start as usize,
                    content: content_text.to_string(),
                    current_state: checked,
                    full_line,
                });
            }
        }
    }

    Ok(checkboxes)
}

/// Find similar checkboxes for suggestions
fn find_similar_checkboxes(pattern: &str, checkboxes: &[CheckboxMatch]) -> Vec<String> {
    let pattern_lower = pattern.to_lowercase();
    let mut similar = Vec::new();

    for checkbox in checkboxes {
        let content_lower = checkbox.content.to_lowercase();
        if content_lower.contains(&pattern_lower) ||
           levenshtein_distance(&pattern_lower, &content_lower) <= 3 {
            similar.push(format!("'{}' (line {})", checkbox.content, checkbox.line));
            if similar.len() >= 3 {
                break;
            }
        }
    }

    similar
}

/// Generate the new checkbox line
fn generate_checkbox_line(original_line: &str, new_state: bool) -> String {
    // Simple replacement: [ ] → [x] or [x] → [ ]
    let checkbox_pattern = if new_state { r"\[ \]" } else { r"\[x\]" };
    let replacement = if new_state { "[x]" } else { "[ ]" };

    regex::Regex::new(checkbox_pattern)
        .unwrap()
        .replace(original_line, replacement)
        .to_string()
}

/// Calculate Levenshtein distance for fuzzy matching
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for (i, row) in matrix.iter_mut().enumerate().take(a_len + 1) {
        row[0] = i;
    }
    for (j, val) in matrix[0].iter_mut().enumerate().take(b_len + 1) {
        *val = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_checkbox_line() {
        assert_eq!(
            generate_checkbox_line("- [ ] Implement caching", true),
            "- [x] Implement caching"
        );
        assert_eq!(
            generate_checkbox_line("- [x] Write tests", false),
            "- [ ] Write tests"
        );
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("cache", "caching"), 3);
        assert_eq!(levenshtein_distance("test", "tests"), 1);
        assert_eq!(levenshtein_distance("exact", "exact"), 0);
    }
}