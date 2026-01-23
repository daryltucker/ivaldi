//! # Append to Section Tool
//!
//! ## PURPOSE
//! Semantic content appending to document sections without line number math.
//!
//! ## PHILOSOPHY
//! Instead of: "Find line 25, insert after the Features header"
//! We do:    "Add this to the Features section"
//!
//! ## USAGE
//! append_to_section(path, section="## Features", content="- [ ] New feature", position="end")
//! - Finds the section boundary using AST
//! - Inserts content at the correct location
//! - Preserves document structure and formatting

use serde::{Deserialize, Serialize};
use serde_json::Value;
use vecq::{parse_file, convert_to_json, FileType};
use crate::IvaldiResponse;
use crate::error::IvaldiError;
use crate::undo::Journal;
use super::types::EditFileArgs;
use super::edit::edit_file;

/// Arguments for append_to_section tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendToSectionArgs {
    /// Path to the file
    pub path: std::path::PathBuf,
    /// Section selector (header name, struct name, etc.)
    pub section: String,
    /// Content to append
    pub content: String,
    /// Where to insert: "end", "start", "after_header" (markdown-specific)
    pub position: InsertPosition,
}

/// Insertion positions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertPosition {
    /// Append to end of section
    End,
    /// Insert at start of section (after header)
    Start,
    /// For markdown: insert after header line but before content
    AfterHeader,
}

/// Result of appending to section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendResult {
    pub path: std::path::PathBuf,
    pub section: String,
    pub inserted_at_line: usize,
    pub section_bounds: SectionBounds,
    pub content_length: usize,
}

/// Section boundaries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionBounds {
    pub start_line: usize,
    pub end_line: usize,
    pub section_type: SectionType,
}

/// Types of sections we can append to
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionType {
    MarkdownHeader,
    CodeStruct,
    CodeFunction,
    CodeImpl,
    JsonObject,
    Unknown,
}

/// Append content to a section in a file
pub async fn append_to_section(
    root: &std::path::Path,
    args: AppendToSectionArgs,
    journal: &Journal,
) -> IvaldiResponse<AppendResult> {
    // 1. Read and parse the file
    let content = match std::fs::read_to_string(&args.path) {
        Ok(c) => c,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Io(e)),
    };

    // 2. Determine file type and parse
    let file_type = FileType::from_path(&args.path);

    match file_type {
        FileType::Markdown => {
            append_to_markdown_section(&content, root, args, journal).await
        },
        FileType::Rust | FileType::Python | FileType::Go | FileType::C | FileType::Cpp => {
            append_to_code_section(&content, file_type, root, args, journal).await
        },
        FileType::Json => {
            append_to_json_section(&content, root, args, journal).await
        },
        _ => {
            IvaldiResponse::from_error(IvaldiError::InvalidArgument(
                format!("append_to_section not supported for file type: {:?}", file_type)
            ))
        }
    }
}

/// Append to markdown section (header-based)
async fn append_to_markdown_section(
    content: &str,
    root: &std::path::Path,
    args: AppendToSectionArgs,
    journal: &Journal,
) -> IvaldiResponse<AppendResult> {
    let parsed = match parse_file(content, FileType::Markdown).await {
        Ok(p) => p,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Internal(format!("Failed to parse Markdown: {}", e))),
    };

    let json = match convert_to_json(parsed) {
        Ok(j) => j,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Internal(format!("Failed to convert to JSON: {}", e))),
    };

    // Find the header that matches our section
    let bounds = match find_markdown_section_bounds(&json, &args.section) {
        Ok(b) => b,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Query(e)),
    };

    // Determine insertion line based on position
    let insert_line = match args.position {
        InsertPosition::End => bounds.end_line,
        InsertPosition::Start | InsertPosition::AfterHeader => bounds.start_line + 1,
    };

    // Prepare content with proper indentation (match the section's indentation)
    let indented_content = prepare_markdown_content(&args.content, content, bounds.start_line);

    // Perform the insertion using edit_file
    let edit_args = EditFileArgs {
        path: args.path.clone(),
        query: None,
        grep: None,
        replacement: format!("\n{}", indented_content),
        from_line: Some(insert_line),
        to_line: Some(insert_line),
        overwrite: false,
    };

    let edit_result = edit_file(root, edit_args, journal).await;

    match edit_result {
        IvaldiResponse { is_error: false, content: Some(_), .. } => {
            let result = AppendResult {
                path: args.path,
                section: args.section,
                inserted_at_line: insert_line,
                section_bounds: bounds,
                content_length: args.content.len(),
            };
            IvaldiResponse::success(result)
        },
        _ => {
            // Convert edit error to our return type
            IvaldiResponse {
                is_error: edit_result.is_error,
                content: None,
                advisory: edit_result.advisory,
                error: edit_result.error,
            }
        }
    }
}

/// Find bounds of a markdown section by header
fn find_markdown_section_bounds(json: &Value, section_name: &str) -> Result<SectionBounds, String> {
    let headers = json.get("headers").and_then(|v| v.as_array()).ok_or("No headers found")?;

    // Find the target header
    let target_header_idx = headers.iter().position(|h| {
        h.get("content")
            .and_then(|c| c.as_str())
            .map(|s| s.trim_start_matches('#').trim() == section_name.trim_start_matches('#').trim())
            .unwrap_or(false)
    }).ok_or_else(|| format!("Header '{}' not found", section_name))?;

    let target_header = &headers[target_header_idx];
    let start_line = target_header.get("line_start")
        .and_then(|v| v.as_u64())
        .ok_or("Header missing line_start")? as usize;

    // Find the next header or end of file
    let end_line = if target_header_idx + 1 < headers.len() {
        let next_header = &headers[target_header_idx + 1];
        next_header.get("line_start")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize - 1
    } else {
        // Last header, find end of file
        json.get("metadata")
            .and_then(|m| m.get("line_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize
    };

    Ok(SectionBounds {
        start_line,
        end_line,
        section_type: SectionType::MarkdownHeader,
    })
}

/// Prepare markdown content with proper indentation
fn prepare_markdown_content(content: &str, file_content: &str, header_line: usize) -> String {
    let lines: Vec<&str> = file_content.lines().collect();
    if header_line >= lines.len() {
        return content.to_string();
    }

    // Check indentation of the header line
    let header_line_content = lines[header_line - 1]; // Convert to 0-based
    let header_indent = header_line_content.len() - header_line_content.trim_start().len();

    // Apply same indentation to content
    content.lines()
        .map(|line| {
            if line.trim().is_empty() {
                line.to_string()
            } else {
                format!("{}{}", " ".repeat(header_indent), line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Append to code section (struct, function, etc.)
async fn append_to_code_section(
    content: &str,
    file_type: FileType,
    root: &std::path::Path,
    args: AppendToSectionArgs,
    journal: &Journal,
) -> IvaldiResponse<AppendResult> {
    let parsed = match parse_file(content, file_type).await {
        Ok(p) => p,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Internal(format!("Failed to parse code: {}", e))),
    };

    let json = match convert_to_json(parsed) {
        Ok(j) => j,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Internal(format!("Failed to convert to JSON: {}", e))),
    };

    // Find the code section bounds
    let bounds = match find_code_section_bounds(&json, &args.section, file_type) {
        Ok(b) => b,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Query(e)),
    };

    // For code, we typically want to append before the closing brace/bracket
    let insert_line = bounds.end_line - 1;

    // Prepare content with proper indentation
    let indented_content = prepare_code_content(&args.content, content, insert_line);

    // Perform the insertion
    let edit_args = EditFileArgs {
        path: args.path.clone(),
        query: None,
        grep: None,
        replacement: format!("\n{}", indented_content),
        from_line: Some(insert_line),
        to_line: Some(insert_line),
        overwrite: false,
    };

    let edit_result = edit_file(root, edit_args, journal).await;

    match edit_result {
        IvaldiResponse { is_error: false, content: Some(_), .. } => {
            let result = AppendResult {
                path: args.path,
                section: args.section,
                inserted_at_line: insert_line,
                section_bounds: bounds,
                content_length: args.content.len(),
            };
            IvaldiResponse::success(result)
        },
        _ => {
            IvaldiResponse {
                is_error: edit_result.is_error,
                content: None,
                advisory: edit_result.advisory,
                error: edit_result.error,
            }
        }
    }
}

/// Find bounds of a code section (struct, function, etc.)
fn find_code_section_bounds(json: &Value, section_name: &str, file_type: FileType) -> Result<SectionBounds, String> {
    match file_type {
        FileType::Rust | FileType::Go => {
            // Look for structs first
            if let Some(structs) = json.get("structs").and_then(|v| v.as_array()) {
                for struct_def in structs {
                    if let Some(name) = struct_def.get("name").and_then(|v| v.as_str()) {
                        if name == section_name {
                            let start_line = struct_def.get("line_start")
                                .and_then(|v| v.as_u64()).ok_or("Struct missing line_start")? as usize;
                            let end_line = struct_def.get("line_end")
                                .and_then(|v| v.as_u64()).ok_or("Struct missing line_end")? as usize;

                            return Ok(SectionBounds {
                                start_line,
                                end_line,
                                section_type: SectionType::CodeStruct,
                            });
                        }
                    }
                }
            }

            // Look for functions
            if let Some(functions) = json.get("functions").and_then(|v| v.as_array()) {
                for func in functions {
                    if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
                        if name == section_name {
                            let start_line = func.get("line_start")
                                .and_then(|v| v.as_u64()).ok_or("Function missing line_start")? as usize;
                            let end_line = func.get("line_end")
                                .and_then(|v| v.as_u64()).ok_or("Function missing line_end")? as usize;

                            return Ok(SectionBounds {
                                start_line,
                                end_line,
                                section_type: SectionType::CodeFunction,
                            });
                        }
                    }
                }
            }
        },
        _ => {
            return Err(format!("Code section finding not implemented for {:?}", file_type));
        }
    }

    Err(format!("Section '{}' not found", section_name))
}

/// Prepare code content with proper indentation
fn prepare_code_content(content: &str, file_content: &str, insert_line: usize) -> String {
    let lines: Vec<&str> = file_content.lines().collect();
    if insert_line >= lines.len() {
        return content.to_string();
    }

    // Get indentation from the context line (usually the closing brace)
    let context_line = lines[insert_line - 1]; // Convert to 0-based
    let _base_indent = context_line.len() - context_line.trim_start().len();

    // For struct fields, we want the same level as other fields
    // Look for existing field indentation by checking a few lines back
    let field_indent = find_field_indentation(file_content, insert_line);

    content.lines()
        .map(|line| {
            if line.trim().is_empty() {
                line.to_string()
            } else {
                format!("{}{}", " ".repeat(field_indent), line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Find the indentation level used for fields in a struct
fn find_field_indentation(file_content: &str, insert_line: usize) -> usize {
    let lines: Vec<&str> = file_content.lines().collect();

    // Look backwards from insert line to find existing field indentation
    for i in (0..insert_line.saturating_sub(1)).rev() {
        let line = lines[i].trim();
        if line.contains(':') && line.ends_with(',') {
            // This looks like a struct field
            let full_line = lines[i];
            return full_line.len() - full_line.trim_start().len();
        }
    }

    // Default to 4 spaces if we can't find existing fields
    4
}

/// Append to JSON section
async fn append_to_json_section(
    content: &str,
    _root: &std::path::Path,
    args: AppendToSectionArgs,
    _journal: &Journal,
) -> IvaldiResponse<AppendResult> {
    // For JSON, we can use our existing edit_json tool
    // But for appending, we need to parse and find insertion points

    let json: Value = match serde_json::from_str(content) {
        Ok(j) => j,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Serialization(e)),
    };

    // For simplicity, assume we're appending to an array or object
    // This is a basic implementation - could be extended
    let _insert_position = match args.position {
        InsertPosition::End => {
            if let Some(arr) = json.as_array() {
                arr.len()
            } else {
                return IvaldiResponse::from_error(IvaldiError::InvalidArgument(
                    "JSON append_to_section only supports arrays for now".to_string()
                ));
            }
        },
        _ => {
            return IvaldiResponse::from_error(IvaldiError::InvalidArgument(
                "JSON append_to_section only supports 'end' position for arrays".to_string()
            ));
        }
    };

    // For now, this is a placeholder - we'd need more sophisticated JSON editing
    // The edit_json tool handles the full JSON replacement case
    IvaldiResponse::from_error(IvaldiError::InvalidArgument(
        "JSON append_to_section not fully implemented yet. Use edit_json for JSON operations.".to_string()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_markdown_content() {
        let file_content = "## Header\nSome content\n";
        let content = "- [ ] New item\n- [ ] Another item";
        let result = prepare_markdown_content(content, file_content, 1);
        // Header has 0 indent, content should match
        assert_eq!(result, "- [ ] New item\n- [ ] Another item");
    }

    #[test]
    fn test_prepare_code_content() {
        let file_content = "struct Config {\n    field1: String,\n}\n";
        let content = "pub new_field: String,";
        let result = prepare_code_content(content, file_content, 3);
        // Basic functionality test - contains the content
        assert!(result.contains("pub new_field: String,"));
    }
}