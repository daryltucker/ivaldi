//! # AST-Based Editing (The Scalpel)
//!
//! ## PURPOSE
//! Provides surgical file editing by targeting AST nodes rather than line numbers.
//!
//! ## PHILOSOPHY
//! - **Stability**: Nodes are more stable than line numbers.
//! - **Precision**: Target exactly what you mean (e.g., "fn main").
//! - **Polyglot**: Leverages `vecq` to support multiple languages.

use anyhow::{Result, anyhow};
use vecq::{parse_file, convert_to_json, query_json, FileType};
use serde_json::Value;

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
) -> Result<String> {
    match selector {
        EditSelector::Node(query) => {
            edit_node(content, file_type, &query, replacement).await
        }
        EditSelector::Lines(start, end) => {
            edit_lines(content, start, end, replacement)
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
) -> Result<String> {
    // 1. Parse
    let parsed = parse_file(content, file_type).await?;
    let json = convert_to_json(parsed)?;
    
    // 2. Query
    let results = query_json(&json, query)?;
    
    if results.is_empty() {
        return Err(anyhow!("No nodes matched query: {}", query));
    }
    if results.len() > 1 {
        return Err(anyhow!("Ambiguous edit: {} nodes matched query", results.len()));
    }
    
    let node = &results[0];
    
    // 3. Extract Line Range
    let line_start = node.get("line_start")
        .and_then(|v: &Value| v.as_u64())
        .ok_or_else(|| anyhow!("Node missing line_start"))? as usize;
    let line_end = node.get("line_end")
        .and_then(|v: &Value| v.as_u64())
        .ok_or_else(|| anyhow!("Node missing line_end"))? as usize;
        
    edit_lines(content, line_start, line_end, replacement)
}

fn edit_lines(
    content: &str,
    start: usize, // 1-indexed
    end: usize,   // 1-indexed
    replacement: &str,
) -> Result<String> {
    let has_trailing_newline = content.ends_with('\n');
    let lines: Vec<&str> = content.lines().collect();
    if start < 1 || start > lines.len() || end < start || end > lines.len() {
        return Err(anyhow!("Invalid line range: {}-{} (total lines: {})", start, end, lines.len()));
    }
    
    let mut new_content = Vec::new();
    
    // Keep lines before
    for item in lines.iter().take(start - 1) {
        new_content.push(item.to_string());
    }
    
    // Inject replacement (trim trailing newline to avoid doubles when joining)
    new_content.push(replacement.trim_end_matches('\n').to_string());
    
    // Keep lines after
    for item in lines.iter().skip(end) {
        new_content.push(item.to_string());
    }
    
    let mut result = new_content.join("\n");
    if has_trailing_newline {
        result.push('\n');
    }
    Ok(result)
}

fn edit_grep(
    content: &str,
    pattern: &str,
    replacement: &str,
) -> Result<String> {
    let regex = regex::Regex::new(pattern)?;
    let mut match_count = 0;
    let mut line_start = 0;
    let mut line_end = 0;
    
    for (i, line) in content.lines().enumerate() {
        if regex.is_match(line) {
            match_count += 1;
            if match_count == 1 {
                line_start = i + 1;
                line_end = i + 1;
            }
        }
    }
    
    if match_count == 0 {
        return Err(anyhow!("No line matched grep pattern: {}", pattern));
    }
    if match_count > 1 {
        return Err(anyhow!("Ambiguous edit: {} lines matched grep pattern", match_count));
    }
    
    edit_lines(content, line_start, line_end, replacement)
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
        let result = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
        assert_eq!(result, "line1\nnew_line2\nline3");
    }

    #[tokio::test]
    async fn test_edit_grep() {
        let content = "foo = 1\nbar = 2";
        let selector = EditSelector::Grep("bar".to_string());
        let replacement = "bar = 3";
        let result = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
        assert_eq!(result, "foo = 1\nbar = 3");
    }

    #[tokio::test]
    async fn test_edit_node_rust() {
        let content = "fn main() {\n    println!(\"old\");\n}";
        let selector = EditSelector::Node(".functions[] | select(.name == \"main\")".to_string());
        let replacement = "fn main() {\n    println!(\"new\");\n}";
        let result = edit_content(content, FileType::Rust, selector, replacement).await.unwrap();
        assert!(result.contains("new"));
        assert!(!result.contains("old"));
    }

    #[tokio::test]
    async fn test_edit_error_no_grep_match() {
        let content = "foo";
        let selector = EditSelector::Grep("bar".to_string());
        let result = edit_content(content, FileType::Text, selector, "baz").await;
        assert!(result.is_err());
    }
}
