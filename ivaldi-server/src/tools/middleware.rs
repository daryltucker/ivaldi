use std::path::PathBuf;
use serde_json::Value; // Added generic Value
use ivaldi_core::response::{IvaldiResponse, ErrorDetail};
use ivaldi_core::heuristics::{Heuristic, GitAwareness, SyntaxGuard, PermissionFixer};
use sha2::{Sha256, Digest}; // Added SHA2
use tracing::warn;

use crate::adt::AdtClient;

/// Middleware to intercept tool execution, running pre- and post-flight heuristics.
///
/// Use `intercept_and_execute` to wrap any tool closure.
pub struct Middleware {
    heuristics: Vec<Box<dyn Heuristic + Send + Sync>>,
    adt_client: Option<AdtClient>,
}

impl Middleware {
    pub fn new(adt_client: Option<AdtClient>) -> Self {
        // Register known heuristics here
        let heuristics: Vec<Box<dyn Heuristic + Send + Sync>> = vec![
            Box::new(GitAwareness),
            Box::new(SyntaxGuard),
            Box::new(PermissionFixer)
        ];
        
        Self { heuristics, adt_client }
    }

    /// Run all Pre-flight checks. 
    pub async fn run_pre(&self, path: &std::path::Path, op: &str, args_hash: &str) -> Vec<ivaldi_core::advisory::AdvisoryMessage> {
        let mut advisories = Vec::new();
        
        // 1. Local Heuristics
        for h in &self.heuristics {
            if let Some(adv) = h.check_pre(path, op) {
                advisories.push(adv);
            }
        }
        
        // 2. Prophetic Checks (ADT) -> PropheticError Heuristic
        if let Some(adt) = &self.adt_client {
            // Prophecy: Have we failed exactly this before?
            match adt.query_prophecy(op, args_hash).await {
                Ok(entries) => {
                    // Filter for failures
                    for entry in entries {
                        if let ivaldi_core::wisdom::Outcome::Failure { message, .. } = entry.outcome {
                            // Found a past failure! Warn the agent.
                            let content = serde_json::json!({
                                "prophecy": "This operation has failed previously.",
                                "past_error": message,
                                "timestamp": entry.timestamp,
                                "agent_version": entry.agent_version
                            });
                            advisories.push(ivaldi_core::advisory::AdvisoryMessage::tool_warn(content));
                            // Only warn once per prophecy query to avoid spam
                            break;
                        }
                    }
                },
                Err(e) => {
                    // Fail silent on prophecy error, but log it?
                    warn!(error = %e, "Prophecy query failed");
                }
            }
        }
        
        advisories
    }

    /// Run all Post-flight checks.
    pub async fn run_post(&self, path: &std::path::Path, op: &str, error: Option<&ErrorDetail>) -> Vec<ivaldi_core::advisory::AdvisoryMessage> {
        let mut advisories = Vec::new();
        
        // 1. Local Heuristics
        for h in &self.heuristics {
            if let Some(adv) = h.check_post(path, op, error) {
                advisories.push(adv);
            }
        }
        
        // 2. Wisdom Logging (ADT) is handled in intercept_and_execute where duration is known.
        
        advisories
    }
}

/// Helper to execute with middleware.
pub async fn intercept_and_execute<F, Fut, T>(
    middleware: &Middleware,
    tool_name: &str,
    path: PathBuf,
    args: &Value, // Now accepts arguments for hashing
    func: F
) -> IvaldiResponse<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = IvaldiResponse<T>>,
    T: serde::Serialize + Send, 
{
    // 0. Compute Context Hash (SHA256 of canonical args)
    let args_str = args.to_string();
    let mut hasher = Sha256::new();
    hasher.update(args_str.as_bytes());
    let args_hash = format!("{:x}", hasher.finalize());

    // 1. Pre-flight
    let pre_advisories = middleware.run_pre(&path, tool_name, &args_hash).await;

    // 2. Execute Tool
    // Start timing
    let start = std::time::Instant::now();
    let mut response = func().await;
    let duration = start.elapsed();

    // 3. Post-flight
    let post_advisories = middleware.run_post(&path, tool_name, response.error.as_ref()).await;

    // 4. ADT Logging (WisdomCollector Heuristic)
    if let Some(adt) = &middleware.adt_client {
        use ivaldi_core::wisdom::{WisdomEntry, Outcome};
        
        let outcome = if response.error.is_some() {
            let err = response.error.as_ref().unwrap();
            Outcome::Failure { 
                code: Some(err.code.clone()), 
                message: err.message.clone() 
            }
        } else {
            Outcome::Success
        };
        
        let entry = WisdomEntry::new(tool_name, &args_hash, outcome, duration.as_millis() as u64);
        
        // Fire and forget (await but don't crash on error)
        if let Err(e) = adt.log_operation(entry).await {
            warn!(error = %e, "Failed to log wisdom");
        }
    }

    // 5. Merge Advisories
    let mut all_advisories = Vec::with_capacity(pre_advisories.len() + post_advisories.len() + response.advisory.len());
    all_advisories.extend(response.advisory); 
    all_advisories.extend(pre_advisories);
    all_advisories.extend(post_advisories);

    response.advisory = all_advisories;

    response
}
