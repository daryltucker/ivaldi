//! # Edit Operation Heuristics
//!
//! ## PURPOSE
//! Provide rich "Crime Scene" context when edit operations fail.
//! This eliminates the need for agents to make additional round trips
//! to investigate why their edit didn't work.
//!
//! ## HEURISTICS
//! - `EditNoMatch`: Query returned 0 results - show what IS available
//! - `EditAmbiguous`: Query returned >1 results - show all matches
//! - `EditGrepNoMatch`: Grep pattern found nothing - show similar lines
//! - `EditGrepAmbiguous`: Grep matched multiple - show all matching lines

use crate::advisory::AdvisoryMessage;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Detailed context for a "no match" edit failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoMatchContext {
    /// The query/pattern that was attempted
    pub query: String,
    /// Type of selector used
    pub selector_type: SelectorType,
    /// File information
    pub file_info: FileInfo,
    /// Available targets (for AST queries)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_targets: Option<AvailableTargets>,
    /// Similar names (Levenshtein suggestions)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub similar_names: Vec<SimilarName>,
    /// For grep: lines that partially matched
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub partial_matches: Vec<PartialMatch>,
}

/// Detailed context for an "ambiguous match" edit failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmbiguousContext {
    /// The query/pattern that matched multiple
    pub query: String,
    /// Type of selector used
    pub selector_type: SelectorType,
    /// File information
    pub file_info: FileInfo,
    /// All the matches found
    pub matches: Vec<MatchInfo>,
    /// Hints for disambiguation
    pub disambiguation_hints: Vec<String>,
}

/// Type of selector used in edit
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectorType {
    AstQuery,
    Grep,
    LineRange,
}

/// Basic file information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub file_type: String,
    pub total_lines: usize,
    pub total_bytes: usize,
}

/// Available targets in a file (for AST queries)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableTargets {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<TargetInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub structs: Vec<TargetInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<TargetInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<TargetInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub list_items: Vec<ListItemInfo>,
}

impl Default for AvailableTargets {
    fn default() -> Self {
        Self {
            functions: Vec::new(),
            structs: Vec::new(),
            classes: Vec::new(),
            imports: Vec::new(),
            headers: Vec::new(),
            list_items: Vec::new(),
        }
    }
}

/// Info about a single target (function, struct, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetInfo {
    pub name: String,
    pub line_start: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// Info about a list item (for Markdown)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItemInfo {
    pub content: String,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
}

/// Context for grep pattern no match
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepNoMatchContext {
    pub pattern: String,
    pub file_info: FileInfo,
    pub partial_matches: Vec<PartialMatch>,
    pub similar_lines: Vec<String>,
}

/// Context for grep pattern ambiguous match
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepAmbiguousContext {
    pub pattern: String,
    pub file_info: FileInfo,
    pub matches: Vec<MatchInfo>,
}

/// A similar name suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarName {
    pub name: String,
    pub distance: usize,
    pub category: String,
}

/// A partial match for grep patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialMatch {
    pub line: usize,
    pub content: String,
    pub match_type: String, // "substring", "case_insensitive", etc.
}

/// Info about a match in ambiguous results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchInfo {
    pub name: String,
    pub line_start: usize,
    pub line_end: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_preview: Option<String>,
}

// ============================================================================
// ADVISORY GENERATION
// ============================================================================

impl NoMatchContext {
    /// Generate an advisory message for a no-match error
    pub fn to_advisory(&self) -> AdvisoryMessage {
        let action = if !self.similar_names.is_empty() {
            let suggestion = &self.similar_names[0];
            format!(
                "Did you mean '{}'? (distance: {})",
                suggestion.name, suggestion.distance
            )
        } else if let Some(targets) = &self.available_targets {
            if !targets.functions.is_empty() {
                format!(
                    "Available functions: {}",
                    targets
                        .functions
                        .iter()
                        .take(5)
                        .map(|f| f.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else if !targets.headers.is_empty() {
                format!(
                    "Available headers: {}",
                    targets
                        .headers
                        .iter()
                        .take(5)
                        .map(|h| h.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else {
                "Check available targets in the response".to_string()
            }
        } else {
            "Verify the query syntax and file content".to_string()
        };

        AdvisoryMessage::adt_suggest(
            json!({
                "query": self.query,
                "selector_type": self.selector_type,
                "file": self.file_info,
                "available_targets": self.available_targets,
                "similar_names": self.similar_names,
                "partial_matches": self.partial_matches,
            }),
            action,
        )
    }
}

impl AmbiguousContext {
    /// Generate an advisory message for an ambiguous match error
    pub fn to_advisory(&self) -> AdvisoryMessage {
        let match_summary: Vec<String> = self
            .matches
            .iter()
            .take(5)
            .map(|m| {
                if let Some(parent) = &m.parent {
                    format!("'{}' in {} (line {})", m.name, parent, m.line_start)
                } else {
                    format!("'{}' at line {}", m.name, m.line_start)
                }
            })
            .collect();

        let action = if !self.disambiguation_hints.is_empty() {
            self.disambiguation_hints[0].clone()
        } else {
            format!(
                "Matched {} nodes. Add more specific selector constraints.",
                self.matches.len()
            )
        };

        AdvisoryMessage::adt_suggest(
            json!({
                "query": self.query,
                "selector_type": self.selector_type,
                "file": self.file_info,
                "match_count": self.matches.len(),
                "matches": self.matches,
                "match_summary": match_summary,
                "disambiguation_hints": self.disambiguation_hints,
            }),
            action,
        )
    }
}

impl GrepNoMatchContext {
    /// Generate an advisory message for a grep no match error
    pub fn to_advisory(&self) -> AdvisoryMessage {
        let similar_lines: Vec<String> = self
            .similar_lines
            .iter()
            .take(3)
            .map(|line: &String| {
                if line.len() > 60 {
                    format!("{}...", &line[..57])
                } else {
                    line.clone()
                }
            })
            .collect();

        let action = if !self.partial_matches.is_empty() {
            format!(
                "Found {} case-insensitive matches. Try a more specific pattern.",
                self.partial_matches.len()
            )
        } else if !self.similar_lines.is_empty() {
            format!("Similar lines found: {}", similar_lines.join(", "))
        } else {
            "Check your pattern syntax or try a broader search".to_string()
        };

        AdvisoryMessage::adt_suggest(
            json!({
                "pattern": self.pattern,
                "selector_type": SelectorType::Grep,
                "file": self.file_info,
                "partial_matches": self.partial_matches,
                "similar_lines": similar_lines,
            }),
            action,
        )
    }
}

impl GrepAmbiguousContext {
    /// Generate an advisory message for a grep ambiguous match error
    pub fn to_advisory(&self) -> AdvisoryMessage {
        let match_summary: Vec<String> = self
            .matches
            .iter()
            .take(5)
            .map(|m| format!("Line {}: {}", m.line_start, m.name))
            .collect();

        let action = format!(
            "Matched {} lines. Use line numbers or a more specific pattern.",
            self.matches.len()
        );

        AdvisoryMessage::adt_suggest(
            json!({
                "pattern": self.pattern,
                "selector_type": SelectorType::Grep,
                "file": self.file_info,
                "match_count": self.matches.len(),
                "matches": self.matches,
                "match_summary": match_summary,
            }),
            action,
        )
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Calculate Levenshtein distance between two strings
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    // Initialize first row and column
    for i in 0..=a_len {
        matrix[i][0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }

    // Fill the matrix
    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

/// Find similar names from a list
pub fn find_similar_names(
    target: &str,
    candidates: &[(&str, &str)],
    max_distance: usize,
) -> Vec<SimilarName> {
    let target_lower = target.to_lowercase();
    let mut similar: Vec<SimilarName> = candidates
        .iter()
        .map(|(name, category)| {
            let distance = levenshtein_distance(&target_lower, &name.to_lowercase());
            SimilarName {
                name: name.to_string(),
                distance,
                category: category.to_string(),
            }
        })
        .filter(|s| s.distance <= max_distance && s.distance > 0)
        .collect();

    similar.sort_by_key(|s| s.distance);
    similar.truncate(5);
    similar
}

/// Extract the target name from a query like `.functions[] | select(.name == "foo")`
pub fn extract_target_name_from_query(query: &str) -> Option<String> {
    // Look for patterns like: .name == "foo" or .name == 'foo'
    let patterns = [
        r#".name == "([^"]+)""#,
        r#".name == '([^']+)'"#,
        r#"select\(.name == "([^"]+)"\)"#,
        r#"select\(.name == '([^']+)'\)"#,
        r#"\.name == "([^"]+)""#,
    ];

    for pattern in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(query) {
                if let Some(m) = caps.get(1) {
                    return Some(m.as_str().to_string());
                }
            }
        }
    }
    None
}

/// Generate disambiguation hints based on matches
pub fn generate_disambiguation_hints(matches: &[MatchInfo], query: &str) -> Vec<String> {
    let mut hints = Vec::new();

    // Check if matches have different parents
    let parents: Vec<&str> = matches.iter().filter_map(|m| m.parent.as_deref()).collect();
    if parents.len() > 1 {
        hints.push(format!(
            "Add parent filter: select(.parent == \"{}\")",
            parents[0]
        ));
    }

    // Check if matches have different line ranges (suggest specific line)
    if matches.len() >= 2 {
        let first = &matches[0];
        hints.push(format!(
            "Use line range: from_line={}, to_line={}",
            first.line_start, first.line_end
        ));
    }

    // Check if we can suggest a more specific name match
    let names: Vec<&str> = matches.iter().map(|m| m.name.as_str()).collect();
    if names.iter().collect::<std::collections::HashSet<_>>().len() > 1 {
        hints.push(format!(
            "Be more specific with name: select(.name == \"{}\")",
            names[0]
        ));
    }

    // If query doesn't have .name selector, suggest adding one
    if !query.contains(".name") && !matches.is_empty() {
        hints.push(format!(
            "Add name filter: select(.name == \"{}\")",
            matches[0].name
        ));
    }

    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("foo", "foo"), 0);
        assert_eq!(levenshtein_distance("foo", "bar"), 3);
        assert_eq!(levenshtein_distance("foo", "foobar"), 3);
        assert_eq!(levenshtein_distance("process", "procces"), 2);
        assert_eq!(levenshtein_distance("", "abc"), 3);
    }

    #[test]
    fn test_extract_target_name() {
        assert_eq!(
            extract_target_name_from_query(r#".functions[] | select(.name == "main")"#),
            Some("main".to_string())
        );
        assert_eq!(
            extract_target_name_from_query(r#".functions[] | select(.name == 'process')"#),
            Some("process".to_string())
        );
        assert_eq!(extract_target_name_from_query(".functions[]"), None);
    }

    #[test]
    fn test_find_similar_names() {
        let candidates = vec![
            ("process_input", "function"),
            ("process_output", "function"),
            ("main", "function"),
        ];

        let similar = find_similar_names("proccess_input", &candidates, 5);
        assert!(!similar.is_empty());
        assert_eq!(similar[0].name, "process_input");
        assert!(similar[0].distance <= 2);
    }
}
