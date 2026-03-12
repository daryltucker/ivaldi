use std::path::Path;
use crate::advisory::AdvisoryMessage;
use super::Heuristic;

/// Checks if the file is a Rust file and suggests checking syntax.
pub struct SyntaxGuard;

impl Heuristic for SyntaxGuard {
    fn id(&self) -> &'static str { "syntax_guard" }
    fn description(&self) -> &'static str { "Validates source file syntax using vecq parser" }

    fn check_post(&self, path: &Path, _op: &str, _error: Option<&crate::response::ErrorDetail>) -> Option<AdvisoryMessage> {
        // Detect language supported by vecq
        let ftype = vecq::FileType::from_path(path);
        
        // If vecq doesn't support it or it's a documentation block, stay silent
        if !ftype.is_supported() || matches!(ftype, vecq::FileType::Text | vecq::FileType::Markdown) {
            return None;
        }

        // Try reading the newly written file
        if let Ok(content) = std::fs::read_to_string(path) {
            // vecq parse_file is async, so we need to enter the runtime
            // since edit/write are executed within tokio but this heuristic is sync.
            // Using a new thread ensures we don't conflict with single-threaded test runtimes or spawn_blocking panics.
            let parse_result = std::thread::scope(|s| {
                s.spawn(|| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    rt.block_on(async { vecq::parse_file(&content, ftype).await })
                }).join().unwrap()
            });

            // Format Language Label cleanly
            let lang_label = format!("{:?}", ftype).to_lowercase();
            // Use standard run commands for coaching tips based on language
            let helper_cmd = match ftype {
                vecq::FileType::Rust => "`cargo check`",
                vecq::FileType::Python => "`python -m py_compile <file>`",
                vecq::FileType::Go => "`go build`",
                _ => "an appropriate compiler or linter",
            };

            match parse_result {
                Ok(_) => {
                    let json_payload = serde_json::json!({
                        "syntax_valid": true,
                        "language": lang_label,
                        "instrumentation": format!("Target file type: {}. AST structural validation passed successfully.", lang_label)
                    });
                    return Some(AdvisoryMessage::tool_info(json_payload));
                },
                Err(e) => {
                    let json_payload = serde_json::json!({
                        "syntax_valid": false,
                        "language": lang_label,
                        "instrumentation": format!("Target file type: {}. AST validation failed: {}. Review lines near the error or use {}.", lang_label, e, helper_cmd)
                    });
                    return Some(AdvisoryMessage::tool_info(json_payload));
                }
            }
        }
        
        None
    }
}

impl SyntaxGuard {
    pub fn apply(path: &Path) -> Option<AdvisoryMessage> {
        Self.check_post(path, "write", None)
    }
}
