use std::path::PathBuf;
use std::fs;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::IvaldiResponse;
use crate::error::IvaldiError;
use crate::util;
use vecq::{parse_file, convert_to_json, query_json, FileType};
use glob;



/// Arguments for the search_code tool
/// 
/// **Behavior**: Executes AST-aware structural queries (jq-style) against code files.
/// 
/// **Modes**:
/// - **Friendly Mode**: Use `category` (e.g., "functions") and `name_pattern` (regex) to find elements without writing jq.
/// - **Power Mode**: Use `query` (jq) for high-precision extraction of specific code elements.
/// 
/// **Safety**:
/// - Max depth: 5 by default.
/// - Respects `.agentignore` (signal-to-noise filter). `.gitignore` is opt-in.
/// 
/// **Usage**: Use to locate function definitions, class structures, or specific imports across a project without reading every file.
/// 
/// **Examples (Code)**:
/// ```
/// // Find all functions named "test_"
/// { "category": "functions", "name_pattern": "test_.*" }
/// 
/// // Find public structs
/// { "query": ".structs[] | select(.visibility == \"pub\")" }
/// ```
/// 
/// **Examples (Markdown)**:
/// ```
/// // Find all level-2 headers
/// { "category": "headers" }
/// 
/// // Find unchecked checkboxes
/// { "category": "list_items", "name_pattern": ".*TODO.*" }
/// 
/// // Power query for code blocks
/// { "query": ".code_blocks[] | select(.language? == \"rust\")" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchCodeArgs {
    /// Path to the file or directory to search
    pub path: PathBuf,
    
    /// AST Query (jq-style) (e.g., .functions[], .classes[], .structs[], .imports[], .comments[]).
    /// If provided, this takes precedence (Power Mode).
    /// If omitted, use `category` and `name_pattern` (Friendly Mode).
    pub query: Option<String>,
    
    /// Friendly Mode: Category to search (code: "functions", "classes", "structs", "imports", "comments")
/// or (markdown: "headers", "list_items", "tables", "code_blocks", "paragraphs", "links", "images")
    pub category: Option<String>,

    /// Friendly Mode: Regex to match name (e.g. "search_.*")
    pub name_pattern: Option<String>,
    
    /// Optional language override (e.g., "rust", "python")
    /// Only applies when searching a single file.
    pub language: Option<String>,
    
    /// Max depth for directory recursion (default: 5)
    #[serde(default = "default_depth")]
    pub depth: usize,
    
    /// Optional glob pattern to filter files in directory (e.g. "*.rs")
    pub pattern: Option<String>,

    /// Whether to respect .agentignore (default: true)
    /// .agentignore is a signal-to-noise filter, not a security boundary.
    /// Agents can bypass it with `respect_agentignore: false`.
    #[serde(default = "default_true")]
    pub respect_agentignore: bool,
    
    /// Maximum results to return (default: 0 = no limit)
    /// Cannot exceed IVALDI_MAX_CONTENT if set.
    #[serde(default)]
    pub limit: usize,
    
    /// Skip N results (for pagination)
    #[serde(default)]
    pub offset: usize,
}

fn default_true() -> bool { true }

fn default_depth() -> usize { 5 }

pub async fn search_code(args: SearchCodeArgs) -> IvaldiResponse<serde_json::Value> {
    let path = args.path.clone();
    
    // 1. Collect Target Files
    let mut targets = Vec::new();
    
    if path.is_file() {
        targets.push(path);
    } else if path.is_dir() {
        let mut builder = ignore::WalkBuilder::new(&path);
        builder.max_depth(Some(args.depth));
        builder.git_ignore(true);
        util::agentignore::apply(&mut builder, args.respect_agentignore);
        
        let glob = args.pattern.as_deref().and_then(|p| glob::Pattern::new(p).ok());
        
        for entry in builder.build().flatten() {
            let p = entry.path();
            if p.is_file() {
                // Pattern filter
                if let Some(ref g) = glob && !g.matches_path(p) && !p.file_name().map(|n| g.matches_path(std::path::Path::new(n))).unwrap_or(false) {
                    continue;
                }
                targets.push(p.to_path_buf());
            }
        }
    } else {
         return IvaldiResponse::error("io_error", format!("Path not found: {}", path.display()));
    }
    
    if targets.is_empty() {
         return IvaldiResponse::success(serde_json::Value::Array(Vec::new()))
             .with_advisory(crate::AdvisoryMessage::tool_info("No matching files found to search."));
    }

    // 2. Parse & Aggregate ("Slurp")
    let mut asts = Vec::new();
    let mut processed_count = 0;
    let single_file_mode = targets.len() == 1; // Capture len before move/borrow
    
    for target in &targets {
        // Skip binary check detailed logic for now, naive read
        if let Ok(content) = fs::read_to_string(target) {
             let ftype = if single_file_mode {
                 // Single file: allow override
                 if let Some(lang) = &args.language {
                      match lang.to_lowercase().as_str() {
                        "rust" | "rs" => FileType::Rust,
                        "python" | "py" => FileType::Python,
                        "go" => FileType::Go,
                        "c" => FileType::C,
                        "cpp" | "c++" => FileType::Cpp,
                        _ => FileType::from_path(target), 
                    }
                 } else {
                     FileType::from_path(target)
                 }
             } else {
                 // Batch: auto-detect
                 FileType::from_path(target)
             };
             
             // Optimization: Skip unsupported types to reduce noise
             if ftype == FileType::Unknown { continue; }
             
             // User expects "Are there any functions...".
             // If we just aggregate ASTs: `[.functions[]]` -> flattened?
             // Let's follow vecq's `slurp` behavior: Array of AST Roots.
             if let Ok(parsed) = parse_file(&content, ftype).await && let Ok(json) = convert_to_json(parsed) {
                 // Tag with filename for context in results
                 let mut annotated = json;
                 if let Some(obj) = annotated.as_object_mut() {
                     obj.insert("file".to_string(), serde_json::json!(target.to_string_lossy()));
                 }
                 asts.push(annotated);
                 processed_count += 1;
             }
        }
    }

    // 3. Query
    // Construct the "Slurped" root
    let root = serde_json::Value::Array(asts);
    
    // Build query string
    let query_string = if let Some(q) = &args.query {
        // User provided a query - needs ".[]" for array iteration unless already included
        if q.contains(".[]") {
            q.clone()
        } else {
            format!(".[] | {}", q)
        }
    } else if let Some(cat) = &args.category {
        // Friendly mode with category - need to add array iteration prefix
        let base = match cat.to_lowercase().as_str() {
            c if c.contains("function") => ".functions[]",
            c if c.contains("class") => ".classes[]",
            c if c.contains("struct") => ".structs[]",
            c if c.contains("import") => ".imports[]",
            c if c.contains("comment") => ".comments[]",
            // Markdown categories
            c if c.contains("header") => ".headers[]",
            c if c.contains("list_item") => ".list_items[]",
            c if c.contains("table") => ".tables[]",
            c if c.contains("code_block") => ".code_blocks[]",
            c if c.contains("paragraph") => ".paragraphs[]",
            c if c.contains("link") => ".links[]",
            c if c.contains("image") => ".images[]",
            other => return IvaldiResponse::from_error(IvaldiError::InvalidArgument(
                format!("Unknown category: {}. Try: functions, classes, structs, imports, comments, headers, list_items, tables, code_blocks, paragraphs, links, images.", other)
            )),
        };
        let mut q = format!(".[] | {}", base);
        
        // For Markdown categories, we filter by content, not name
        if cat.to_lowercase().contains("header") || cat.to_lowercase().contains("list_item") || cat.to_lowercase().contains("table") {
            if let Some(pattern) = &args.name_pattern {
                q.push_str(&format!(" | select(.content? | test(\"{}\"))", pattern.replace("\"", "\\\"")));
            }
        } else {
            // Code categories use name filter
            if let Some(pattern) = &args.name_pattern {
                q.push_str(&format!(" | select(.name? | test(\"{}\"))", pattern.replace("\"", "\\\"")));
            }
        }
        q
    } else {
        return IvaldiResponse::from_error(IvaldiError::InvalidArgument(
            "Must provide either 'query' (jq) OR 'category' (friendly).".into()
        ));
    };
    
    let results = match query_json(&root, &query_string) {
        Ok(r) => r,
        Err(e) => return IvaldiResponse::error("query_error", e.to_string()),
    };
    
    // Apply offset
    let start = std::cmp::min(args.offset, results.len());
    let mut results: Vec<serde_json::Value> = results.into_iter().skip(start).collect();
    
    // Apply limit (with IVALDI_MAX_CONTENT cap)
    let effective_limit = if let Some(max_content) = std::env::var("IVALDI_MAX_CONTENT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok()) 
    {
        let agent_limit = if args.limit > 0 { args.limit } else { max_content };
        std::cmp::min(agent_limit, max_content)
    } else if args.limit > 0 { 
        args.limit 
    } else { 
        results.len() 
    };
    
    let mut was_limited = false;
    let results_len = results.len();
    if effective_limit > 0 && results_len > effective_limit {
        results.truncate(effective_limit);
        was_limited = true;
    }
    
    // 4. Return with advisory
    let final_results = results;
    let returned_count = final_results.len();
    let mut response = IvaldiResponse::success(serde_json::Value::Array(final_results));
    
    // Add advisory about truncation if needed
    #[allow(clippy::collapsible_if)]
    if let Some(max_content) = std::env::var("IVALDI_MAX_CONTENT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok()) 
    {
        if args.limit > 0 && args.limit > max_content {
            response = response.with_advisory(
                crate::AdvisoryMessage::tool_warn(
                    format!("Requested limit {} exceeds IVALDI_MAX_CONTENT={}. Capped.", 
                             args.limit, max_content)
                )
            );
        }
    }
    
    if was_limited {
        response = response.with_advisory(
            crate::AdvisoryMessage::tool_warn(
                format!("Content truncated to {} results (IVALDI_MAX_CONTENT cap or limit)", effective_limit)
            )
        );
    }
    
    if returned_count > 0 {
        response = response.with_advisory(
            crate::AdvisoryMessage::tool_info(
                format!("Scanned {} files. Returned {} results.", processed_count, returned_count)
            )
        );
    } else {
        response = response.with_advisory(
            crate::AdvisoryMessage::tool_info(
                format!("Scanned {} files. Query returned 0 matches.", processed_count)
            )
        );
    }
    
    response
}

// ============================================================================
// TESTS for IVALDI_MAX_CONTENT
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_search_code_no_max_content() {
        // Test: Without IVALDI_MAX_CONTENT set, all results returned
        unsafe { env::remove_var("IVALDI_MAX_CONTENT"); }
        
        let temp_dir = tempfile::tempdir().unwrap();
        // Create a single Rust file with functions
        let rust_code = r#"
fn function_0() { let x = 1; }
fn function_1() { let x = 2; }
fn function_2() { let x = 3; }
fn function_3() { let x = 4; }
fn function_4() { let x = 5; }
"#;
        let file_path = temp_dir.path().join("test.rs");
        std::fs::write(&file_path, rust_code).unwrap();
        
        let args = SearchCodeArgs {
            path: file_path.clone(),  // Use single file for simplicity
            query: Some(".functions[]".to_string()),
            category: None,
            name_pattern: None,
            language: Some("rust".to_string()),
            depth: 1,
            pattern: None,
            respect_agentignore: false,
            limit: 0,
            offset: 0,
        };
        
        let response = search_code(args).await;
        assert!(!response.is_error, "Should not error: {:?}", response.error);
        if let Some(content) = response.content {
            let arr = content.as_array().unwrap();
            assert_eq!(arr.len(), 5, "Should return all 5 functions, got: {}", arr.len());
        } else {
            panic!("No content in response");
        }
    }

    #[tokio::test]
    async fn test_search_code_with_limit() {
        // Test: limit parameter works
        unsafe { env::remove_var("IVALDI_MAX_CONTENT"); }
        
        let temp_dir = tempfile::tempdir().unwrap();
        let rust_code = r#"
fn function_0() { let x = 1; }
fn function_1() { let x = 2; }
fn function_2() { let x = 3; }
fn function_3() { let x = 4; }
fn function_4() { let x = 5; }
fn function_5() { let x = 6; }
fn function_6() { let x = 7; }
fn function_7() { let x = 8; }
fn function_8() { let x = 9; }
fn function_9() { let x = 10; }
"#;
        let file_path = temp_dir.path().join("test.rs");
        std::fs::write(&file_path, rust_code).unwrap();
        
        let args = SearchCodeArgs {
            path: file_path,
            query: Some(".functions[]".to_string()),
            category: None,
            name_pattern: None,
            language: Some("rust".to_string()),
            depth: 1,
            pattern: None,
            respect_agentignore: false,
            limit: 3,
            offset: 0,
        };
        
        let response = search_code(args).await;
        assert!(!response.is_error, "Should not error: {:?}", response.error);
        if let Some(content) = response.content {
            let arr = content.as_array().unwrap();
            assert_eq!(arr.len(), 3, "Should return only 3 results due to limit, got: {}", arr.len());
        } else {
            panic!("No content in response");
        }
    }

    #[tokio::test]
    async fn test_search_code_with_offset() {
        // Test: offset parameter works for pagination
        unsafe { env::remove_var("IVALDI_MAX_CONTENT"); }
        
        let temp_dir = tempfile::tempdir().unwrap();
        let rust_code = r#"
fn function_0() { let x = 1; }
fn function_1() { let x = 2; }
fn function_2() { let x = 3; }
fn function_3() { let x = 4; }
fn function_4() { let x = 5; }
"#;
        let file_path = temp_dir.path().join("test.rs");
        std::fs::write(&file_path, rust_code).unwrap();
        
        let args = SearchCodeArgs {
            path: file_path,
            query: Some(".functions[]".to_string()),
            category: None,
            name_pattern: None,
            language: Some("rust".to_string()),
            depth: 1,
            pattern: None,
            respect_agentignore: false,
            limit: 2,
            offset: 2,
        };
        
        let response = search_code(args).await;
        assert!(!response.is_error, "Should not error: {:?}", response.error);
        if let Some(content) = response.content {
            let arr = content.as_array().unwrap();
            assert_eq!(arr.len(), 2, "Should return 2 results starting from offset 2, got: {}", arr.len());
        } else {
            panic!("No content in response");
        }
    }
}
