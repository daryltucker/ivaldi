use std::path::{Path, PathBuf};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufReader, BufRead};
use std::cmp::Ordering;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::{IvaldiResponse, util};
use ignore::WalkBuilder;

/// Arguments for the analyze_dir tool
/// 
/// **Behavior**: Provides a high-level summary of a directory's structure and contents.
/// **Usage**: Use to quickly understand the layout of a project or a large subdirectory.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeDirArgs {
    /// Path to analyze (default: ".")
    #[serde(default = "default_root")]
    pub path: PathBuf,
    
    /// Maximum depth (default: 5)
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    
    /// Whether to respect .agentignore (default: true)
    /// .agentignore is a signal-to-noise filter, not a security boundary.
    /// Agents can bypass it with `respect_agentignore: false`.
    #[serde(default = "default_true")]
    pub respect_agentignore: bool,
}

fn default_root() -> PathBuf { PathBuf::from(".") }
fn default_max_depth() -> usize { 5 }
fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DirAnalysis {
    pub path: PathBuf,
    pub file_count: usize,
    pub dir_count: usize,
    pub total_size_bytes: u64,
    pub extensions: BTreeMap<String, usize>,
    pub structure: FileNode,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FileNode {
    pub name: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileNode>>,
    pub size: Option<u64>,
}

/// Arguments for the analyze_file tool
/// 
/// **Behavior**: Performs deep analysis of a single file, extracting symbols (functions, classes) and metadata.
/// **Usage**: Use to search the codebase for specific patterns or symbols. Supports friendly and power modes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeFileArgs {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FileAnalysis {
    pub path: PathBuf,
    pub lines: usize,
    pub size_bytes: u64,
    pub complexity_score: u32, // Heuristic based on indentation/length
    pub symbols: Vec<String>, // Functions, Structs
    pub dependencies: Vec<String>, // Imports based on simple parsing
    pub todos: Vec<String>,
}

pub struct Analyzer;

impl Analyzer {
    pub fn analyze_dir(args: AnalyzeDirArgs) -> IvaldiResponse<DirAnalysis> {
        let root = args.path;
        
        // Phase 1: Walk the directory tree using ignore::WalkBuilder.
        // This respects .agentignore (by default) and enforces depth limits.
        let mut walker = WalkBuilder::new(&root);
        walker
            .max_depth(Some(args.max_depth))
            .git_ignore(false)   // .gitignore is opt-in; matches Ivaldi philosophy
            .ignore(false)       // .ignore is opt-in
            .hidden(true);       // skip dotfiles by default
        util::agentignore::apply(&mut walker, args.respect_agentignore);
        
        // Phase 2: Collect all entries into a path → metadata map.
        // Track which paths are directories vs files, and their sizes.
        // Key is the canonical relative path from root.
        #[derive(Default)]
        struct EntryMeta {
            is_dir: bool,
            size: u64,
        }
        
        let mut entries: HashMap<PathBuf, EntryMeta> = HashMap::new();
        let mut file_count: usize = 0;
        let mut dir_count: usize = 0;
        let mut total_size: u64 = 0;
        let mut extensions: BTreeMap<String, usize> = BTreeMap::new();
        
        for result in walker.build() {
            let Ok(entry) = result else { continue };
            if entry.depth() == 0 {
                // Root entry — we already know this exists
                continue;
            }
            
            let path = entry.path().to_path_buf();
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let size = entry.metadata().ok().map(|m| m.len()).unwrap_or(0);
            
            entries.insert(path.clone(), EntryMeta { is_dir, size });
            
            if is_dir {
                dir_count += 1;
            } else {
                file_count += 1;
                total_size += size;
                
                // Track file extension
                let ext = entry.path().extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_else(|| "no_ext".to_string());
                *extensions.entry(ext).or_default() += 1;
            }
        }
        
        // Phase 3: Build parent → children mapping for tree reconstruction.
        let mut children_by_parent: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        for path in entries.keys() {
            if let Some(parent) = path.parent() {
                children_by_parent.entry(parent.to_path_buf())
                    .or_default()
                    .push(path.clone());
            }
        }
        
        // Sort each parent's children: directories first, then by name.
        for children in children_by_parent.values_mut() {
            children.sort_by(|a, b| {
                let a_is_dir = entries.get(a).map(|e| e.is_dir).unwrap_or(false);
                let b_is_dir = entries.get(b).map(|e| e.is_dir).unwrap_or(false);
                match (a_is_dir, b_is_dir) {
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                    _ => a.file_name().cmp(&b.file_name()),
                }
            });
        }
        
        // Phase 4: Recursively build the tree from the root.
        fn build_tree(
            path: &Path,
            entries: &HashMap<PathBuf, EntryMeta>,
            children_by_parent: &HashMap<PathBuf, Vec<PathBuf>>,
        ) -> FileNode {
            let name = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let meta = entries.get(path);
            
            let is_dir = meta.map(|m| m.is_dir).unwrap_or_else(|| path.is_dir());
            let size = meta.map(|m| m.size);
            
            let children = if is_dir {
                let kids = children_by_parent.get(path)
                    .map(|child_paths| {
                        child_paths.iter()
                            .map(|cp| build_tree(cp, entries, children_by_parent))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                Some(kids)
            } else {
                None
            };
            
            FileNode { name, is_dir, children, size }
        }
        
        // The root is depth 0 — it won't be in `entries` (we skip depth==0 above).
        // But it might not be in the walker results at all if WalkBuilder doesn't yield it.
        // Build the root node manually.
        let root_meta = EntryMeta { is_dir: true, size: 0 };
        entries.insert(root.clone(), root_meta);
        dir_count += 1; // count root
        let root_node = build_tree(&root, &entries, &children_by_parent);
        
        IvaldiResponse::success(DirAnalysis {
            path: root,
            file_count,
            dir_count,
            total_size_bytes: total_size,
            extensions,
            structure: root_node,
        })
    }
    
    pub fn analyze_file(args: AnalyzeFileArgs) -> IvaldiResponse<FileAnalysis> {
        use crate::error::IvaldiError;
        let path = args.path;
        if !path.exists() || !path.is_file() {
            return IvaldiResponse::from_error(IvaldiError::FileNotFound(path));
        }
        
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => return IvaldiResponse::from_error(IvaldiError::Io(e)),
        };
        
        if metadata.len() > 10 * 1024 * 1024 { // 10MB limit
             return IvaldiResponse::from_error(IvaldiError::Internal("File > 10MB".into()));
        }
        
        let file = match File::open(&path) {
             Ok(f) => f,
             Err(e) => return IvaldiResponse::from_error(IvaldiError::Io(e)),
        };
        let reader = BufReader::new(file);
        
        let mut lines_count = 0;
        let mut symbols = Vec::new();
        let mut dependencies = Vec::new();
        let mut todos = Vec::new();
        let mut complexity_score = 0;
        
        // Simple regex-based parsing (Robust enough for overview)
        // Rust patterns
        let re_fn = regex::Regex::new(r"^\s*pub\s*fn\s+([a-zA-Z0-9_]+)").unwrap();
        let re_struct = regex::Regex::new(r"^\s*pub\s*(struct|enum|trait|type)\s+([a-zA-Z0-9_]+)").unwrap();
        let re_use = regex::Regex::new(r"^\s*use\s+([^;]+)").unwrap();
        let re_todo = regex::Regex::new(r"(TODO|FIXME|XXX):?\s*(.*)").unwrap();
        
        for line in reader.lines().map_while(Result::ok) {
            lines_count += 1;
            
            // Complexity: +1 for every 4 spaces of indentation
            let indent = line.chars().take_while(|c| *c == ' ').count();
            complexity_score += (indent / 4) as u32;
            
            if line.len() > 80 {
                complexity_score += 1;
            }
            
            if let Some(cap) = re_fn.captures(&line) {
                symbols.push(format!("fn {}", &cap[1]));
            }
            if let Some(cap) = re_struct.captures(&line) {
                symbols.push(format!("{} {}", &cap[1], &cap[2]));
            }
            if let Some(cap) = re_use.captures(&line) {
                dependencies.push(cap[1].trim().to_string());
            }
            if let Some(cap) = re_todo.captures(&line) {
                todos.push(format!("{}: {}", &cap[1], &cap[2]));
            }
        }

        // Limit lists to avoid flooding
        if symbols.len() > 50 {
            symbols.truncate(50);
            symbols.push("... (truncated)".to_string());
        }
        
        IvaldiResponse::success(FileAnalysis {
            path,
            lines: lines_count,
            size_bytes: metadata.len(),
            complexity_score,
            symbols,
            dependencies,
            todos,
        })
    }
}


