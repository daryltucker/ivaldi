use std::path::Path;
use crate::advisory::AdvisoryMessage;

/// A Heuristic is an "opinionated" module that can offer advice
/// based on the state of the world or the result of an operation.
pub trait Heuristic {
    /// Unique identifier for this heuristic (e.g. "git_awareness")
    fn id(&self) -> &'static str;
    
    /// User-friendly description of what this heuristic checks
    fn description(&self) -> &'static str;

    /// Pre-execution check.
    /// Returns `Some(AdvisoryMessage)` if a warning/blocker is detected *before* the operation.
    fn check_pre(&self, _path: &Path, _op: &str) -> Option<AdvisoryMessage> {
        None
    }

    /// Post-execution check.
    /// Returns `Some(AdvisoryMessage)` if an issue is detected *after* the operation (success or failure).
    /// `result` is the outcome of the operation (Ok or Err).
    fn check_post(&self, _path: &Path, _op: &str, _error: Option<&crate::response::ErrorDetail>) -> Option<AdvisoryMessage> {
        None
    }
}

// Module declarations
pub mod git;
pub mod syntax;
pub mod permissions;
pub mod typos;

// Re-exports for convenience
pub use git::GitAwareness;
pub use syntax::SyntaxGuard;
pub use permissions::PermissionFixer;
pub use typos::SiblingTyposHint;

/// ParentDirectoryHint (Stump implementation, maybe move to own file later if expanded)
pub struct ParentDirectoryHint;
impl ParentDirectoryHint {
    pub fn apply(_path: &Path, _error: &std::io::Error) -> Option<AdvisoryMessage> {
        None 
    }
}
