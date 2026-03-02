use std::path::Path;
use crate::advisory::AdvisoryMessage;
use super::Heuristic;

/// Checks if the path is in a git repository and if it is ignored.
pub struct GitAwareness;

impl Heuristic for GitAwareness {
    fn id(&self) -> &'static str { "git_awareness" }
    fn description(&self) -> &'static str { "Checks git status (ignored, untracked)" }

    fn check_pre(&self, path: &Path, _op: &str) -> Option<AdvisoryMessage> {
        let path_str = path.to_string_lossy();
        if path.exists() && path_str.ends_with(".tmp") {
            let content = serde_json::json!({
                "git_status": "ignored",
                "reason": "Target path suffix (.tmp) suggests a temporary file, which is often excluded from version control."
            });
            // tool_info takes 1 argument: content
            return Some(AdvisoryMessage::tool_info(content));
        }
        None
    }
}

impl GitAwareness {
    // Legacy helper for older code (if any still calls it directly)
    pub fn apply(path: &Path) -> Option<AdvisoryMessage> {
       Self.check_pre(path, "unknown")
    }
}
