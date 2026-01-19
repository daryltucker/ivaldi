//! Git Read Operations
//!
//! Read-only git history access via git2 crate.
//! Supports blame, log, diff, and search operations.

pub mod types;
pub mod sync;

pub use types::{GitReadArgs, GitAction};

use std::path::PathBuf;
use crate::IvaldiResponse;
use serde_json::Value;
use git2::Repository;

/// Perform git read operation
pub async fn git_read(args: GitReadArgs, session_project_root: Option<&PathBuf>) -> IvaldiResponse<Value> {
    use crate::error::IvaldiError;
    
    // Determine repository root
    let repo_root = match args.project_root.as_ref().or(session_project_root) {
        Some(root) => root.clone(),
        None => return IvaldiResponse::from_error(IvaldiError::InvalidArgument("No project_root available. Cannot locate git repository.".into())),
    };
    
    // We use spawn_blocking because git2::Repository is not Sync and its operations are CPU-bound C calls.
    let res = tokio::task::spawn_blocking(move || {
        // Discover repo inside the thread
        let repo = match Repository::discover(&repo_root) {
            Ok(r) => r,
            Err(e) => return IvaldiResponse::from_error(IvaldiError::Git(e)),
        };
        
        match args.action {
            GitAction::Blame { path, lines } => sync::git_blame_sync(&repo, &path, lines.as_deref()),
            GitAction::Log { path, limit, since } => sync::git_log_sync(&repo, path.as_deref(), limit, since.as_deref()),
            GitAction::Diff { from, to, path, stat_only } => sync::git_diff_sync(&repo, &from, &to, path.as_deref(), Some(stat_only)),
            GitAction::Search { query, path, limit } => sync::git_search_sync(&repo, &query, path.as_deref(), limit),
            GitAction::Raw { args } => sync::git_raw_sync(args),
        }
    }).await;

    match res {
        Ok(response) => response,
        Err(e) => IvaldiResponse::from_error(IvaldiError::Internal(format!("Git operation panicked: {}", e))),
    }
}
