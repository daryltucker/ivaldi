pub mod middleware;
pub mod session;
pub mod cli;

use serde_json::Value;
use ivaldi_core::navigate::{FsNavigator, Navigator, FindFilesArgs};
use ivaldi_core::observe::{
    FsObserver, Observer, ReadFileArgs, ReadFilesArgs, Analyzer, AnalyzeDirArgs, AnalyzeFileArgs, 
    SearchCodeArgs, GitReadArgs, ReadSyslogsArgs
};
use ivaldi_core::list::{FsLister, Lister, ListDirArgs};
use ivaldi_core::mutate::{Mutator, WriteFileArgs, EditFileArgs};
use ivaldi_core::undo::{Journal, Undoer, UndoArgs};
use ivaldi_core::session::types::{SessionInitArgs, SessionListArgs, SessionGetArgs, SessionUpdateArgs};
use ivaldi_core::lifecycle::project_root::find_project_root;


/// A unified error type for tool execution
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),
    #[error("Execution failed: {0}")]
    Execution(String),
}

/// Execute a tool by name (Requires State for Session tools)
pub async fn execute_tool(name: &str, args: Value, state: &crate::state::ServerState) -> Result<Value, ToolError> {
    match name {
        "find_files" => {
            let mut args: FindFilesArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            
            if state.config().enable_gitignore && !args.enable_gitignore {
                args.enable_gitignore = true;
            }

            let response = FsNavigator::find_files(args);
            Ok(serde_json::to_value(response).unwrap())
        },
        "read_files" => {
            let args: ReadFilesArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            let response = FsObserver::read_files(args);
            Ok(serde_json::to_value(response).unwrap())
        },
        "read_file" => {
            let args: ReadFileArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            let response = FsObserver::read_file(args);
            Ok(serde_json::to_value(response).unwrap())
        },
        "list_dir" => {
            let mut args: ListDirArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            
            if state.config().enable_gitignore && !args.enable_gitignore {
                args.enable_gitignore = true;
            }

            let response = FsLister::list_dir(args);
            Ok(serde_json::to_value(response).unwrap())
        },
        "run_command" => {
            let args: cli::RunCommandArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            let response = cli::run_command(args, state).await
                .map_err(|e| ToolError::Execution(e.to_string()))?;
            Ok(serde_json::to_value(response).unwrap())
        },
        "write_file" => {
            let args: WriteFileArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            
            // Lifecycle extraction
            let start = args.path.parent().unwrap_or(&args.path);
            let root = find_project_root(start);
            let journal_path = root.join(".ivaldi/journal.jsonl");
            let journal = Journal::open(&journal_path).map_err(|e| ToolError::Execution(format!("Journal error: {}", e)))?;

            let response = Mutator::write_file(&root, args, &journal);
            Ok(serde_json::to_value(response).unwrap())
        },
        "edit_file" => {
            let args: EditFileArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

            // Lifecycle extraction
            let start = args.path.parent().unwrap_or(&args.path);
            let root = find_project_root(start);
            let journal_path = root.join(".ivaldi/journal.jsonl");
            let journal = Journal::open(&journal_path).map_err(|e| ToolError::Execution(format!("Journal error: {}", e)))?;
            
            // Direct await, no nested runtime!
            let response = Mutator::edit_file(&root, args, &journal).await;
            
            Ok(serde_json::to_value(response).unwrap())
        },
        "edit_files" => {
            use ivaldi_core::mutate::EditFilesArgs; // Import local to block if needed, or global
            let args: EditFilesArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

            // Lifecycle extraction (use path of first edit as anchor)
            if args.edits.is_empty() {
                 return Ok(serde_json::to_value(ivaldi_core::IvaldiResponse::<Vec<std::path::PathBuf>>::success(vec![])).unwrap());
            }
            let start = args.edits[0].path.parent().unwrap_or(&args.edits[0].path);
            let root = find_project_root(start);
            let journal_path = root.join(".ivaldi/journal.jsonl");
            let journal = Journal::open(&journal_path).map_err(|e| ToolError::Execution(format!("Journal error: {}", e)))?;
            
            let response = Mutator::edit_files(&root, args, &journal).await;
            
            Ok(serde_json::to_value(response).unwrap())
        },
        "undo" => {
             let args: UndoArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
             
             let root = find_project_root(&args.path);
             
             let journal_path = root.join(".ivaldi/journal.jsonl");
             let journal = Journal::open(&journal_path).map_err(|e| ToolError::Execution(format!("Journal error: {}", e)))?;

             let response = Undoer::undo_last(&root, &journal);
             Ok(serde_json::to_value(response).unwrap())
        },
        // Session Tools
        "session_init" => {
            let args: SessionInitArgs = serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            let response = session::session_init(args, state)?;
            Ok(serde_json::to_value(response).unwrap())
        },
        "session_list" => {
            let args: SessionListArgs = serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            let response = session::session_list(args, state)?;
            Ok(serde_json::to_value(response).unwrap())
        },
        "session_get" => {
            let args: SessionGetArgs = serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            let response = session::session_get(args, state)?;
            Ok(serde_json::to_value(response).unwrap())
        },
        "session_update" => {
            let args: SessionUpdateArgs = serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            let response = session::session_update(args, state)?;
            Ok(serde_json::to_value(response).unwrap())
        },

        // Analysis Tools
        "analyze_dir" => {
            let args: AnalyzeDirArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            
            // Heuristic compatibility: apply gitignore default from config if needed
            // (AnalyzeDirArgs has ignore_patterns, but we might want to respect global config too)
            // For now, simple pass-through.
            
            let response = Analyzer::analyze_dir(args);
            Ok(serde_json::to_value(response).unwrap())
        },
        "analyze_file" => {
            let args: AnalyzeFileArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            
            let response = Analyzer::analyze_file(args);
            Ok(serde_json::to_value(response).unwrap())
        },
        "search_code" => {
            let args: SearchCodeArgs = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            
            let response = ivaldi_core::observe::search_code(args).await;
            Ok(serde_json::to_value(response).unwrap())
        },
        "git_read" => {
            let args: GitReadArgs = serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            let project_root = state.get_session().map(|s| s.root);
            Ok(serde_json::to_value(ivaldi_core::observe::git::git_read(args, project_root.as_ref()).await).unwrap())
        },
        "read_syslogs" => {
            let args: ReadSyslogsArgs = serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            Ok(serde_json::to_value(ivaldi_core::observe::syslogs::read_syslogs(args).await).unwrap())
        },

        _ => Err(ToolError::NotFound(name.into()))
    }
}