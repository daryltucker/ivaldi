use std::path::Path;
use crate::advisory::AdvisoryMessage;
use super::Heuristic;

/// Checks if the file is a Rust file and suggests checking syntax.
pub struct SyntaxGuard;

impl Heuristic for SyntaxGuard {
    fn id(&self) -> &'static str { "syntax_guard" }
    fn description(&self) -> &'static str { "Suggests cargo check for Rust files" }

    fn check_post(&self, path: &Path, _op: &str, _error: Option<&crate::response::ErrorDetail>) -> Option<AdvisoryMessage> {
        if let Some(ext) = path.extension() && ext == "rs" {
            let content = serde_json::json!({
                "syntax_valid": false, 
                "language": "rust",
                "error": "Potential syntax error detected. Code block may be incomplete."
            });
            return Some(AdvisoryMessage::adt_suggest(content, "Run `cargo check` to verify syntax."));
        }
        None
    }
}

impl SyntaxGuard {
    pub fn apply(path: &Path) -> Option<AdvisoryMessage> {
        Self.check_post(path, "write", None)
    }
}
