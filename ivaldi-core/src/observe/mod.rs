//! # Observation Module
//!
//! ## PURPOSE
//! Provides safe file reading capabilities ("Telescope").
//!
//! ## PHILOSOPHY
//! - **Focused View**: "Read this specific thing"
//! - **Blast Shields**: Protect Agent from binary dumps, huge files, and memory exhaustion.
//! - **Smart Truncation**: Give context (head/tail) rather than exploding.
//!
//! ## KEY TYPES
//! - `Observer` trait
//! - `ReadOptions` struct

use std::path::PathBuf;
use std::fs::File;
use std::io::{Read, BufReader, Seek, SeekFrom};
use crate::{IvaldiResponse, AdvisoryMessage}; 
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Arguments for the read_file tool
/// 
/// **Behavior**: Reads file content.
/// **Safety**:
/// - **Blast Shield**: 10MB limit.
/// - **Binary Protection**: Fails on binary files.
/// - **Smart Truncation**: If no lines specified and >1000 lines, returns head (500) + tail (500).
///   **Usage**:
///   1. Read without args first.
///   2. If truncated, use `from_line`/`to_line` to read specific sections.
///   3. Use `query` for AST-based extraction (e.g., `.functions[]`).
///   4. Use `grep` for regex pattern matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ReadFileArgs {
    /// Path to the file to read
    pub path: PathBuf,

    /// Start line (1-indexed, inclusive)
    pub from_line: Option<usize>,

    /// End line (1-indexed, inclusive)
    pub to_line: Option<usize>,

    /// Force read even if binary/large (dangerous!)
    #[serde(default = "default_read_force")]
    pub force: bool,
    
    /// vecq query for AST-based extraction (e.g., ".functions[]", ".imports[]")
    pub query: Option<String>,
    
    /// Regex pattern for line matching (e.g., "^use ", "TODO:")
    pub grep: Option<String>,
    
    /// Number of context lines around grep matches (default: 2)
    #[serde(default = "default_context_lines")]
    pub context_lines: Option<usize>,
}

fn default_read_force() -> bool { false }
fn default_context_lines() -> Option<usize> { Some(2) }

/// Content of a read file
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FileContent {
    pub path: PathBuf,
    pub content: String,
    pub info: ReadInfo,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReadInfo {
    pub lines_total: usize,
    pub lines_returned: usize,
    pub truncated: bool,
    pub is_binary: bool,
}

/// Arguments for the read_files tool
/// 
/// **Behavior**: Batch-reads multiple files at once.
/// **Safety**: Inherits individual safety checks from `read_file`.
/// **Usage**: Use to read multiple files at once. Supports safety boundaries for file size and count.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadFilesArgs {
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FilesContent {
    /// Map of Path -> FileContent
    pub results: std::collections::HashMap<PathBuf, IvaldiResponse<FileContent>>,
}

pub trait Observer {
    fn read_file(args: ReadFileArgs) -> IvaldiResponse<FileContent>;
    
    // Default implementation can loop, but specific impls might optimize
    fn read_files(args: ReadFilesArgs) -> IvaldiResponse<FilesContent> {
        let mut results = std::collections::HashMap::new();
        for path in args.paths {
            let read_args = ReadFileArgs {
                path: path.clone(),
                from_line: None,
                to_line: None,
                force: false,
                query: None,
                grep: None,
                context_lines: None,
            };
            // Self::read_file is static? No, trait method.
            // Wait, read_file is defined as `fn read_file(...)` (static) in the trait?
            // Yes: `fn read_file(args: ReadFileArgs) -> IvaldiResponse<FileContent>;`
            // So we can call it.
            let resp = Self::read_file(read_args);
            results.insert(path, resp);
        }
        IvaldiResponse::success(FilesContent { results })
    }
}

pub struct FsObserver;

impl Observer for FsObserver {
    fn read_file(args: ReadFileArgs) -> IvaldiResponse<FileContent> {
        use crate::error::IvaldiError;
        let path = &args.path;
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                // Heuristic apply uses &e, so we must not move e yet.
                // We'll create the hint first.
                let hint = crate::heuristics::SiblingTyposHint::apply(path, &e);
                let mut response = IvaldiResponse::from_error(IvaldiError::Io(e));
                
                if let Some(h) = hint {
                    response = response.with_advisory(h);
                }

                return response;
            },
        };

        // 1. SAFETY: Size Limit (10MB)
        const MAX_SIZE: u64 = 10 * 1024 * 1024;
        if metadata.len() > MAX_SIZE && !args.force {
             return IvaldiResponse::from_error(IvaldiError::FileTooLarge(path.to_path_buf()));
        }

        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) => return IvaldiResponse::from_error(IvaldiError::Io(e)),
        };
        let mut reader = BufReader::new(file);

        // 2. SAFETY: Binary Check (Peek first 1024 bytes)
        let mut buffer = [0; 1024];
        let bytes_read = reader.read(&mut buffer).unwrap_or(0); // Ignore errors here, just checking
        
        let null_count = buffer[..bytes_read].iter().filter(|&&b| b == 0).count();
        // Heuristic: If > 0 null bytes in first 1KB, it's likely binary.
        // Some text encodings (UTF-16) generally have nulls, but for source code (UTF-8) zero nulls is expected.
        if null_count > 0 && !args.force {
             return IvaldiResponse::from_error(IvaldiError::BinaryDetected(path.to_path_buf()));
        }

        // Reset cursor
        if let Err(e) = reader.seek(SeekFrom::Start(0)) {
            return IvaldiResponse::from_error(IvaldiError::Io(e));
        }

        // 3. READING & TRUNCATION
        // Read full content into buffer (protected by MAX_SIZE check above)
        let mut content_buffer = Vec::new();
        if let Err(e) = reader.read_to_end(&mut content_buffer) {
             return IvaldiResponse::from_error(IvaldiError::Io(e));
        }

        let full_string = if args.force {
            String::from_utf8_lossy(&content_buffer).to_string()
        } else {
            match String::from_utf8(content_buffer) {
                Ok(s) => s,
                Err(e) => return IvaldiResponse::from_error(IvaldiError::Internal(format!("Invalid UTF-8 sequence: {}", e))),
            }
        };

        // 4. SELECTOR DISPATCH: query > grep > lines
        // Query selector (AST-based extraction via vecq)
        if let Some(query) = &args.query {
            return read_with_query(&full_string, path, query);
        }
        
        // Grep selector (regex pattern matching)
        if let Some(pattern) = &args.grep {
            let context = args.context_lines.unwrap_or(2);
            return read_with_grep(&full_string, path, pattern, context);
        }

        // 5. LINE-RANGE PROCESSING (existing behavior)
        let all_lines: Vec<&str> = full_string.lines().collect();

        let total_lines = all_lines.len();
        let from = args.from_line.unwrap_or(1).max(1) - 1; // 0-indexed start
        let to = args.to_line.unwrap_or(total_lines).min(total_lines);
        
        // Logic check: if from > to, simple error or empty?
        if from >= total_lines && total_lines > 0 {
             return IvaldiResponse::success(FileContent {
                path: path.to_path_buf(),
                content: String::new(),
                info: ReadInfo { lines_total: total_lines, lines_returned: 0, truncated: false, is_binary: false },
             }).with_advisory(AdvisoryMessage::tool_warn("Start line is beyond end of file."));
        }

        let mut final_content = String::new();
        let mut truncated = false;
        let mut advisory = None;

        if args.from_line.is_some() || args.to_line.is_some() {
            // Explicit range
            let end_idx = to.min(total_lines);
            for line in all_lines.iter().take(end_idx).skip(from) {
                final_content.push_str(line);
                final_content.push('\n');
            }
        } else {
            // No range specified - Smart Truncation
            const MAX_LINES: usize = 1000;
            if total_lines > MAX_LINES && !args.force {
                truncated = true;
                // Head (500)
                for line in all_lines.iter().take(500) {
                    final_content.push_str(line);
                    final_content.push('\n');
                }
                final_content.push_str("\n... [ TRUNCATED ] ...\n");
                // Tail (500)
                for line in all_lines.iter().skip(total_lines.saturating_sub(500)) {
                    final_content.push_str(line);
                    final_content.push('\n');
                }
                
                advisory = Some(AdvisoryMessage::tool_warn(
                    format!("File truncated ({} lines). Showing head/tail. Use --from/--to to read specific sections.", total_lines)
                ));
            } else {
                // Full read
                final_content = full_string; // Use original string if no truncation needed
            }
        }
        
        // Trim last newline if added? No, keep it faithful.
        
        let lines_returned = final_content.lines().count(); // approximate

        let mut response = IvaldiResponse::success(FileContent {
            path: path.to_path_buf(),
            content: final_content,
            info: ReadInfo {
                lines_total: total_lines,
                lines_returned,
                truncated,
                is_binary: false,
            }
        });

        if let Some(adv) = advisory {
            response = response.with_advisory(adv);
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_read_readme() {
        // Need to find README.md relative to current dir during test
        // Cargo tests run in the crate root (ivaldi-core)
        // README.md is in project root (../README.md)
        let path = PathBuf::from("../README.md");
        if !path.exists() {
            // Try current dir just in case
            let path = PathBuf::from("README.md");
            if !path.exists() {
                 return; // skip if can't find it
            }
        }
        
        let args = ReadFileArgs {
            path: PathBuf::from("../README.md"),
            ..Default::default()
        };
        let response = FsObserver::read_file(args);
        assert!(!response.is_error, "Response should not be an error: {:?}", response.error);
        let content = response.content.expect("Response should have content");
        assert!(!content.content.is_empty(), "Content should not be empty");
        assert!(content.info.lines_total > 0, "Should have more than 0 lines");
    }
}

// ============================================================================
// SELECTOR HELPER FUNCTIONS
// ============================================================================

mod selectors;
use selectors::{read_with_query, read_with_grep};

pub mod analyze;
pub mod search;
pub mod git;
pub mod syslogs;

pub use analyze::{Analyzer, AnalyzeDirArgs, AnalyzeFileArgs};
pub use search::{search_code, SearchCodeArgs};
pub use git::*;
pub use syslogs::*;
