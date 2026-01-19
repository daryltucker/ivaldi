use std::path::Path;
use crate::{IvaldiResponse, AdvisoryMessage};
use crate::error::IvaldiError;
use super::{FileContent, ReadInfo};

/// Read file content using a vecq query (AST-based extraction).
/// 
/// Example: `query=".functions[]"` returns all function bodies.
pub fn read_with_query(content: &str, path: &Path, query: &str) -> IvaldiResponse<FileContent> {
    use vecq::{parse_file, convert_to_json, query_json, detect_file_type};
    
    let file_type = detect_file_type(&path.to_string_lossy());
    
    // Create a dedicated, ephemeral runtime for AST parsing to avoid 'nested block_on' panics
    // when called from within an existing runtime (which often happens in tests or servers).
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            return IvaldiResponse::error("runtime_error", format!("Failed to create AST runtime: {}", e));
        }
    };
    
    let result = rt.block_on(async {
        let parsed = parse_file(content, file_type).await?;
        let json = convert_to_json(parsed)?;
        query_json(&json, query)
    });
    
    match result {
        Ok(results) => {
            if results.is_empty() {
                return IvaldiResponse::error("no_matches", format!("Query '{}' matched 0 nodes", query))
                    .with_advisory(AdvisoryMessage::tool_info("Try a broader query like '.functions[]' or '.structs[]'"));
            }
            
            // Format results as readable content
            let formatted: Vec<String> = results.iter()
                .map(|v| {
                    if let Some(s) = v.as_str() {
                        s.to_string()
                    } else {
                        serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
                    }
                })
                .collect();
            
            let output = formatted.join("\n\n---\n\n");
            let lines_returned = output.lines().count();
            
            IvaldiResponse::success(FileContent {
                path: path.to_path_buf(),
                content: output,
                info: ReadInfo {
                    lines_total: content.lines().count(),
                    lines_returned,
                    truncated: false,
                    is_binary: false,
                },
            }).with_advisory(AdvisoryMessage::tool_info(format!("Query matched {} nodes", results.len())))
        }
        Err(e) => {
            IvaldiResponse::from_error(IvaldiError::Query(format!("vecq query failed: {}", e)))
                .with_advisory(AdvisoryMessage::tool_info("Check query syntax. Examples: '.functions[]', '.imports[]', '.structs[] | select(.name == \"Foo\")'"))
        }
    }
}

/// Read file content using a regex pattern (grep-style).
/// 
/// Returns matching lines with optional context lines before/after.
pub fn read_with_grep(content: &str, path: &Path, pattern: &str, context_lines: usize) -> IvaldiResponse<FileContent> {
    use crate::error::IvaldiError;
    let regex = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => {
            return IvaldiResponse::from_error(IvaldiError::Regex(format!("Invalid regex pattern: {}", e)));
        }
    };
    
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    
    // Find matching line indices
    let matches: Vec<usize> = lines.iter()
        .enumerate()
        .filter(|(_, line)| regex.is_match(line))
        .map(|(i, _)| i)
        .collect();
    
    if matches.is_empty() {
        return IvaldiResponse::from_error(IvaldiError::Regex(format!("Pattern '{}' matched 0 lines", pattern)))
            .with_advisory(AdvisoryMessage::tool_info("Try a broader pattern or check regex syntax"));
    }
    
    // Build output with context
    let mut output = String::new();
    let mut included: std::collections::HashSet<usize> = std::collections::HashSet::new();
    
    for &match_idx in &matches {
        let start = match_idx.saturating_sub(context_lines);
        let end = (match_idx + context_lines + 1).min(total_lines);
        
        // Add separator if we've already added content and there's a gap
        if !output.is_empty() && start > 0 {
            let last_included = included.iter().max().copied().unwrap_or(0);
            if start > last_included + 1 {
                output.push_str("\n...\n\n");
            }
        }
        
        for (i, item) in lines.iter().enumerate().take(end).skip(start) {
            if !included.contains(&i) {
                included.insert(i);
                let prefix = if i == match_idx { ">>> " } else { "    " };
                output.push_str(&format!("{:4}: {}{}\n", i + 1, prefix, item));
            }
        }
    }
    
    let lines_returned = included.len();
    
    IvaldiResponse::success(FileContent {
        path: path.to_path_buf(),
        content: output,
        info: ReadInfo {
            lines_total: total_lines,
            lines_returned,
            truncated: false,
            is_binary: false,
        },
    }).with_advisory(AdvisoryMessage::tool_info(format!("Pattern matched {} lines (showing with {} lines context)", matches.len(), context_lines)))
}
