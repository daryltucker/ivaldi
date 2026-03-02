use std::path::PathBuf;
use std::fs;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::IvaldiResponse;
use crate::error::IvaldiError;
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
/// - Respects `.gitignore`.
/// 
/// **Usage**: Use to locate function definitions, class structures, or specific imports across a project without reading every file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchCodeArgs {
    /// Path to the file or directory to search
    pub path: PathBuf,
    
    /// AST Query (jq-style) (e.g., .functions[], .classes[], .structs[], .imports[], .comments[]).
    /// If provided, this takes precedence (Power Mode).
    /// If omitted, use `category` and `name_pattern` (Friendly Mode).
    pub query: Option<String>,
    
    /// Friendly Mode: Category to search "functions", "classes", "structs", "imports", "comments"
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
}

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
        // Add .aiignore respect if needed via builder.add_custom_ignore_filename(".aiignore");
        
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
    
    let query_string = if let Some(q) = &args.query {
        q.clone()
    } else {
        // Construct friendly query
        // Default to all nodes if no category
        let base = match args.category.as_deref().map(|s| s.to_lowercase()) {
            Some(c) if c.contains("function") => ".functions[]",
            Some(c) if c.contains("class") => ".classes[]",
            Some(c) if c.contains("struct") => ".structs[]",
            Some(c) if c.contains("import") => ".imports[]",
            Some(c) if c.contains("comment") => ".comments[]",
            // TODO: Add more mappings as vecq standardizes them
            Some(other) => return IvaldiResponse::from_error(IvaldiError::InvalidArgument(format!("Unknown category: {}. Try functions, classes, structs, imports, comments.", other))),
            None => ".[] | .[]?" // Flatten array of files, then try to get any top level array? No, that's risky.
                                // If no category, we default to "functions" or maybe we need to search ALL.
                                // Searching ALL is hard in simple mode. Let's error if neither query nor category.
        };

        // If no category and no query, error out or default to something safe?
        // Let's require at least one for now.
        if args.category.is_none() && args.query.is_none() {
             return IvaldiResponse::from_error(IvaldiError::InvalidArgument("Must provide either 'query' (jq) OR 'category' (friendly).".into()));
        }

        let mut q = if args.category.is_none() {
            // Fallback for purely regex based search? Not supported yet without category.
            // Maybe ".[] | to_entries[] | .value[]" ? Too generic.
             return IvaldiResponse::from_error(IvaldiError::InvalidArgument("Friendly mode requires a 'category'.".into()));
        } else {
             base.to_string()
        };

        // Append filters
        if let Some(pattern) = &args.name_pattern {
             // select(.name | test("pattern"))
             // Escape quotes in pattern? User provides regex string.
             q.push_str(&format!(" | select(.name? | test(\"{}\"))", pattern.replace("\"", "\\\"")));
        }
        
        // Add implicit array iterator for slurp root?
        // The root is an Array of Files. 
        // Our 'base' (e.g. .functions[]) expects to operate on a File Object.
        // So we need to map over the root array.
        // Query: `.[] | .functions[] | ...`
        format!(".[] | {}", q)
    };
    
    let results = match query_json(&root, &query_string) {
        Ok(r) => r,
        Err(e) => return IvaldiResponse::error("query_error", e.to_string()),
    };
    
    // 4. Return
    IvaldiResponse::success(serde_json::Value::Array(results))
        .with_advisory(crate::AdvisoryMessage::tool_info(
            format!("Scanned {} files.", processed_count)
        ))
}
