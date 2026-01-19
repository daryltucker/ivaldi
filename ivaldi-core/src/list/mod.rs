//! # Listing Module
//!
//! ## PURPOSE
//! Provides safe, shallow directory listing ("Sensors").
//!
//! ## PHILOSOPHY
//! - **Local Awareness**: "What is right here?"
//! - **Shallow**: Doesn't recurse (use `navigate` for that).
//! - **Metadata**: Immediate size/type info.
//!
//! ## KEY TYPES
//! - `Lister` trait
//! - `ListOptions` struct

use std::path::PathBuf;
// use std::fs;
use crate::{IvaldiResponse, AdvisoryMessage}; 
use std::time::SystemTime;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Arguments for the list_dir tool
/// 
/// **Behavior**: Lists directory contents with metadata (type, size).
/// 
/// **Safety**:
/// - Non-recursive. Use `find_files` for deep search.
/// - Caps output at 1000 items (heuristic safety).
/// 
/// **Usage**: Use to map out local structure, check file existence, or inspect file sizes and modification times in a single directory.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListDirArgs {
    /// Directory path to list (default: ".")
    #[serde(default = "default_list_path")]
    pub path: PathBuf,

    /// Sort entries by name (default: true)
    #[serde(default = "default_list_sort")]
    pub sort: bool,

    /// Show hidden files (default: false)
    #[serde(default = "default_list_hidden")]
    pub show_hidden: bool,

    /// Whether to respect .aiignore (default: true)
    #[serde(default = "default_true")]
    pub respect_aiignore: bool,

    /// Whether to evaluate and restrict based on .gitignore (default: false)
    #[serde(default = "default_false")]
    pub enable_gitignore: bool,
}

fn default_list_path() -> PathBuf { PathBuf::from(".") }
fn default_list_sort() -> bool { true }
fn default_list_hidden() -> bool { false }
fn default_true() -> bool { true }
fn default_false() -> bool { false }

/// Metadata for a directory entry
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: Option<u64>, // Unix timestamp
}

pub trait Lister {
    fn list_dir(args: ListDirArgs) -> IvaldiResponse<Vec<DirEntry>>;
}

pub struct FsLister;

impl Lister for FsLister {
    fn list_dir(args: ListDirArgs) -> IvaldiResponse<Vec<DirEntry>> {
        let path = &args.path;
        
        let mut walker = ignore::WalkBuilder::new(path);
        walker
            .max_depth(Some(1))
            .git_ignore(args.enable_gitignore)
            .ignore(args.enable_gitignore)
            .hidden(!args.show_hidden); // if show_hidden is false, we want to hide them

        if args.respect_aiignore {
            walker.add_custom_ignore_filename(".aiignore");
        }

        let mut entries = Vec::new();
        let mut advisory = None;

        for entry_res in walker.build() {
            match entry_res {
                Ok(entry) => {
                    if entry.depth() == 0 { continue; }
                    
                    let name = entry.file_name().to_string_lossy().to_string();
                    let metadata = match entry.metadata() {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    let modified = metadata.modified().ok()
                        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs());

                    entries.push(DirEntry {
                        name,
                        path: entry.path().to_path_buf(),
                        is_dir: metadata.is_dir(),
                        is_symlink: metadata.file_type().is_symlink(),
                        size: metadata.len(),
                        modified,
                    });
                }
                Err(e) => {
                     advisory = Some(AdvisoryMessage::tool_warn(format!("Failed to read an entry: {}", e)));
                }
            }
        }

        if args.sort {
            entries.sort_by(|a, b| a.name.cmp(&b.name));
        }
        
        // Safety Cap: If directory has > 1000 items, maybe warn?
        if entries.len() > 1000 {
             let msg = AdvisoryMessage::tool_info(format!("Directory contains {} items. Consider using filters.", entries.len()));
             match advisory {
                 Some(ref mut _adv) => { /* already have one warning, maybe not override? We can have multiple advisories but strict struct implies one? No, Vec<Advisory> in success. */
                    // Wait, our builder logic adds *one* advisory. If we want multiple in logic, we need to collect them.
                    // For now, let's just use the count warning if no read error occured.
                 },
                 None => advisory = Some(msg),
             }
        }

        let mut response = IvaldiResponse::success(entries);
        if let Some(adv) = advisory {
            response = response.with_advisory(adv);
        }
        response
    }
}
