//! # Navigation Module
//!
//! ## PURPOSE
//! Provides safe, filtered filesystem traversal ("Radar").
//!
//! ## PHILOSOPHY
//! - **Broad Sweep**: "Find me everything interesting"
//! - **Safety Limiters**: Depth cap, Entry cap, Timeout
//! - **Noise Filtering**: Respects .gitignore, .ignore, and .aiignore
//!
//! ## KEY TYPES
//! - `Navigator` trait
//! - `FindOptions` struct

use std::path::PathBuf;
use std::time::{Duration, Instant};
use crate::{IvaldiResponse, AdvisoryMessage}; 
use ignore::WalkBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Arguments for the find_files tool
/// 
/// **Behavior**: Searches for files matching a glob pattern.
/// **Safety**:
/// - Max depth: 5 (prevents infinite recursion).
/// - Max entries: 100 (prevents context flooding).
/// - Respects `.gitignore` (if enabled) and `.aiignore`.
///   **Advisory**: Warns if results are truncated.
///   **Usage**:
///   Use to locate files when you don't know the exact path.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FindFilesArgs {
    /// The root directory to search in (default: ".")
    #[serde(default = "default_root")]
    pub path: PathBuf,
    
    /// Glob pattern to match (e.g., "*.rs", "target/**/lib.rs")
    pub pattern: String,

    /// Maximum depth to traverse (default: 5)
    #[serde(default = "default_max_depth")]
    pub max_depth: usize, // Changed from Option to direct usize with default
    
    /// Maximum number of entries to return (default: 100)
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
    
    /// Timeout in milliseconds (default: 2000)
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    
    /// Whether to evaluate and restrict based on .gitignore (default: false)
    #[serde(default = "default_false")]
    pub enable_gitignore: bool,
    
    /// Whether to respect .aiignore (default: true)
    #[serde(default = "default_true")]
    pub respect_aiignore: bool,

    /// Whether to respect .agentignore (default: true)
    #[serde(default = "default_true")]
    pub respect_agentignore: bool,
}

// Defaults for Serde
fn default_root() -> PathBuf { PathBuf::from(".") }
fn default_max_depth() -> usize { 5 }
fn default_max_entries() -> usize { 100 }
fn default_timeout_ms() -> u64 { 2000 }
fn default_true() -> bool { true }
fn default_false() -> bool { false }

/// Metadata for a matched file
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FileMatch {
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

pub trait Navigator {
    fn find_files(args: FindFilesArgs) -> IvaldiResponse<Vec<FileMatch>>;
}

pub struct FsNavigator;

impl Navigator for FsNavigator {
    fn find_files(args: FindFilesArgs) -> IvaldiResponse<Vec<FileMatch>> {
        let start_time = Instant::now();
        let timeout = Duration::from_millis(args.timeout_ms);
        let max_entries = args.max_entries;
        let max_depth = args.max_depth;

        let mut walker = WalkBuilder::new(&args.path);
        walker
            .max_depth(Some(max_depth))
            .git_ignore(args.enable_gitignore)
            .ignore(args.enable_gitignore) // .ignore files
            .hidden(false); // We usually want hidden files unless specifically ignored

        if args.respect_aiignore {
             walker.add_custom_ignore_filename(".aiignore");
        }
        if args.respect_agentignore {
             walker.add_custom_ignore_filename(".agentignore");
        }

        let mut matches = Vec::new();
        let mut truncated = false;
        let mut timed_out = false;
        
        let glob = glob::Pattern::new(&args.pattern).ok();

        for result in walker.build() {
             if start_time.elapsed() > timeout {
                timed_out = true;
                break;
            }
            if matches.len() >= max_entries {
                truncated = true;
                break;
            }

            if let Ok(entry) = result {
                let path = entry.path();
                if entry.depth() == 0 { continue; }

                // Pattern Matching Logic
                let matches_pattern = if args.pattern.is_empty() || args.pattern == "*" {
                    true
                } else if let Some(ref g) = glob {
                    g.matches_path(path) || 
                    path.file_name().map(|n| g.matches_path(std::path::Path::new(n))).unwrap_or(false)
                } else {
                    let path_str = path.to_string_lossy();
                    path_str.contains(&args.pattern) || 
                    path.file_name().map(|n| n.to_string_lossy().contains(&args.pattern)).unwrap_or(false)
                };

                if matches_pattern {
                     let metadata = entry.metadata().ok();
                     let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                     let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);

                     matches.push(FileMatch {
                         path: path.to_path_buf(),
                         is_dir,
                         size,
                     });
                }
            }
        }

        let mut response = IvaldiResponse::success(matches);

        if truncated {
            response = response.with_advisory(
                AdvisoryMessage::tool_info(
                    format!("Search truncated at {} results. Please refine query.", max_entries)
                )
            );
        }

        if timed_out {
             response = response.with_advisory(
                AdvisoryMessage::tool_warn(
                    format!("Search timed out after {:?}. Results may be incomplete.", timeout)
                )
            );
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_find_files_simple() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_find.txt");
        fs::write(&file_path, "hello").unwrap();
        
        // Simulate parsing from JSON (like MCP would)
        let json = serde_json::json!({
            "path": dir.path(),
            "pattern": "test_find.txt"
        });
        let args: FindFilesArgs = serde_json::from_value(json).unwrap();
        
        let resp = FsNavigator::find_files(args);
        let results = resp.content.unwrap();
        assert!(!results.is_empty(), "Should find the file");
        assert_eq!(results[0].path.file_name().unwrap(), "test_find.txt");
    }

    #[test]
    fn test_find_files_glob() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("find_me.rs"), "").unwrap();
        
        let json = serde_json::json!({
            "path": dir.path(),
            "pattern": "*.rs"
        });
        let args: FindFilesArgs = serde_json::from_value(json).unwrap();

        let resp = FsNavigator::find_files(args);
        assert!(!resp.content.unwrap().is_empty());
    }
}
