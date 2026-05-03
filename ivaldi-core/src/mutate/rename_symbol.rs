#![allow(clippy::collapsible_if)]

use super::types::{RenameSymbolArgs, WriteFileArgs};
use super::write::write_file;
use crate::response::IvaldiResponse;
use crate::advisory::AdvisoryMessage;
use crate::undo::Journal;
use crate::error::IvaldiError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use vecq::{parse_file, convert_to_json, query_json, FileType};

/// Result of a rename_symbol operation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RenameResult {
    pub files_modified: usize,
    pub symbols_renamed: usize,
    pub backups_created: usize,
}

pub async fn rename_symbol(
    root: &Path,
    args: RenameSymbolArgs,
    journal: &Journal,
) -> IvaldiResponse<RenameResult> {
    let path = if args.path.starts_with('/') {
        PathBuf::from(&args.path)
    } else {
        root.join(&args.path)
    };
    
    // 1. Read current content
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Io(e)),
    };

    // 2. Build precision query based on symbol_type
    // This ensures we don't accidentally rename a variable that has the same name as a function
    let query = match args.symbol_type.as_deref() {
        Some("function") => format!(".functions[] | select(.name == \"{}\")", args.old_name),
        Some("struct") | Some("class") => format!("(.structs[] // .classes[]) | select(.name == \"{}\")", args.old_name),
        Some("import") => format!(".imports[] | select(.name == \"{}\")", args.old_name),
        _ => format!("(.. | select(.name? == \"{}\"))", args.old_name), // Fallback: deep search for any named node
    };

    let file_type = FileType::from_path(&path);
    
    // Only attempt Scalpel logic for supported structured file types
    let is_scalpel_supported = matches!(file_type, 
        FileType::Rust | FileType::Python | FileType::Go | 
        FileType::C | FileType::Cpp | FileType::Markdown | FileType::Json
    );

    if is_scalpel_supported {
        // 3. Parse AST and Locate Symbol(s)
        let parsed_result = parse_file(&content, file_type).await;
        
        if let Ok(parsed) = parsed_result {
            if let Ok(json) = convert_to_json(parsed) {
                // Determine if we should be broad (file scope) or surgical
                let use_broad_query = args.scope.as_deref() == Some("file");
                
                let effective_query = if use_broad_query {
                    // Find any node that has this name.
                    // This is safer than global string replace because it stays within AST boundaries
                    // (e.g. it won't replace the name inside a large comment block if the AST doesn't label it as a node with .name)
                    format!(".. | select(.name? == \"{}\")", args.old_name)
                } else {
                    query.clone()
                };

                if let Ok(results) = query_json(&json, &effective_query) {
                    if !results.is_empty() {
                        // AST Match Found!
                        let mut final_content: String;
                        let symbols_count: usize;
                        let is_file_scope = args.scope.as_deref() == Some("file");

                        if is_file_scope {
                            // SMART HAMMER: We confirmed the symbol exists via AST, 
                            // now perform global replace to catch all references (definitions, calls, etc.)
                            // as the current AST might be too shallow to catch them all surgically.
                            final_content = content.replace(&args.old_name, &args.new_name);
                            symbols_count = results.len(); // Approximate, or we could count matches in string.
                        } else {
                            // SURGICAL SCALPEL: Only replace the specific nodes found by the query.
                             let mut nodes_to_patch = Vec::new();
                            for node in &results {
                                let line_start = node.get("line_start").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(0);
                                let line_end = node.get("line_end").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(0);
                                let node_content = node.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                
                                if line_start != 0 && !node_content.is_empty() {
                                    nodes_to_patch.push((line_start, line_end, node_content.to_string()));
                                }
                            }
                            
                            // Sort by line_start descending
                            nodes_to_patch.sort_by(|a, b| b.0.cmp(&a.0));
                            
                            let mut current_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                            let has_trailing_newline = content.ends_with('\n');

                            for (start, end, node_content) in nodes_to_patch {
                                let new_node_content = node_content.replace(&args.old_name, &args.new_name);
                                let new_node_lines: Vec<String> = new_node_content.lines().map(|s| s.to_string()).collect();
                                current_lines.splice((start-1)..end, new_node_lines);
                            }
                            
                            final_content = current_lines.join("\n");
                            if has_trailing_newline && !final_content.ends_with('\n') {
                                final_content.push('\n');
                            }
                            symbols_count = results.len();
                        }

                        if symbols_count > 0 {
                            let write_args = WriteFileArgs {
                                path: path.clone(),
                                content: final_content,
                                overwrite: true,
                                append: false,
                            };
    
                            let write_resp = write_file(root, write_args, journal);
                            
                            let mut response = IvaldiResponse {
                                is_error: write_resp.is_error,
                                content: if write_resp.is_error { None } else {
                                    Some(RenameResult {
                                        files_modified: 1,
                                        symbols_renamed: symbols_count,
                                        backups_created: 1,
                                    })
                                },
                                ui_diffs: Vec::new(),
                                error: write_resp.error,
                                advisory: write_resp.advisory,
                            };
    
                            response.advisory.push(AdvisoryMessage::tool_info(format!(
                                "Renamed '{}' -> '{}' in {} locations using {} (scope: {}).",
                                args.old_name, args.new_name, symbols_count,
                                if is_file_scope { "Smart Hammer (AST-validated global replace)" } else { "Surgical Scalpel (AST node only)" },
                                args.scope.as_deref().unwrap_or("default")
                            )));
    
                            return response;
                        }
                    } else if !use_broad_query {
                        // If specific query failed, try broad one as a courtesy fallback before erroring
                        return IvaldiResponse::from_error(IvaldiError::Query(format!(
                            "Symbol '{}' not found in {} structure ({})", 
                            args.old_name, args.path, query
                        ))).with_advisory(AdvisoryMessage::tool_info("Try setting scope='file' for a broader search."));
                    }
                }
            }
        }
    }

    // FALLBACK: "Hammer Lite"
    // If AST logic is not supported for this file type, or parsing/structure check fails completely,
    // we fall back to a global string replacement with a warning.
    let new_content = content.replace(&args.old_name, &args.new_name);
    
    let write_args = WriteFileArgs {
        path: path.clone(),
        content: new_content,
        overwrite: true,
        append: false,
    };

    let write_resp = write_file(root, write_args, journal);
    
    let mut response = IvaldiResponse {
        is_error: write_resp.is_error,
        content: if write_resp.is_error { None } else {
            Some(RenameResult {
                files_modified: 1,
                symbols_renamed: 1,
                backups_created: 1,
            })
        },
        ui_diffs: Vec::new(),
        error: write_resp.error,
        advisory: write_resp.advisory,
    };

    if !response.is_error {
        response.advisory.push(AdvisoryMessage::tool_warn(serde_json::json!({
            "message": "AST-aware renaming not possible for this file type. Fell back to global replacement (The Hammer).",
            "issue": "hammer_fallback",
            "suggestion": "Use surgical 'edit_file' for more sensitive changes in unsupported file types."
        })));
    }

    response
}