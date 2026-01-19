use anyhow::Result;
use ivaldi_core::wisdom::{WisdomEntry, Outcome};
use serde_json::{json, Value};
use tracing::{info, warn};
use std::time::Duration;

/// Client for the Abstract Decision Tree (Vector Database) via HTTP API.
/// 
/// This client handles:
/// 1. Logging "Wisdom" (tool execution traces) to the `ivaldi_ops` collection.
/// 2. Querying "Wisdom" to provide prophetic warnings.
/// 
/// **Resilience**: All operations are "best effort". If the ADT server is unreachable,
/// we simply log a warning and return success/empty results. The "Hand" must work
/// even if the "Brain" is sleeping.
#[derive(Clone)]
pub struct AdtClient {
    client: reqwest::Client,
    base_url: String,
    collection: String,
}

impl AdtClient {
    /// Create a new ADT client connecting to the given URL (default: http://localhost:8080).
    pub fn new(url_override: Option<String>) -> Self {
        let base = url_override.unwrap_or_else(|| "http://localhost:8080".to_string());
        // Strip trailing slash
        let base_url = if base.ends_with('/') {
            base[..base.len()-1].to_string()
        } else {
            base
        };

        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_millis(500)) // Fast timeout, don't block the agent
                .build()
                .unwrap_or_default(),
            base_url,
            collection: "ivaldi_ops".to_string(), // TODO: Configurable?
        }
    }

    /// Log a tool operation to the ADT.
    /// 
    /// This should be called *after* the operation concludes (check_post).
    /// It constructs a text representation for embedding (the "Lesson")
    /// and stores the structured WisdomEntry as metadata.
    pub async fn log_operation(&self, entry: WisdomEntry) -> Result<()> {
        // 1. Construct the "Lesson" text for embedding.
        // Format: "Tool [NAME] [OUTCOME]: [ERROR/CONTEXT]"
        let outcome_str = match &entry.outcome {
            Outcome::Success => "succeeded".to_string(),
            Outcome::Failure { code, message } => {
                format!("failed with {} ({})", code.as_deref().unwrap_or("error"), message)
            }
        };
        
        let text = format!("Tool {} {}. ArgsHash: {}", entry.tool_name, outcome_str, entry.args_hash);
        
        // 2. Prepare Metadata
        let mut metadata = serde_json::Map::new();
        metadata.insert("tool".to_string(), Value::String(entry.tool_name.clone()));
        metadata.insert("outcome".to_string(), Value::String(match entry.outcome {
            Outcome::Success => "success".to_string(),
            Outcome::Failure { .. } => "failure".to_string(),
        }));
        metadata.insert("args_hash".to_string(), Value::String(entry.args_hash.clone()));
        
        if let Outcome::Failure { code: Some(c), .. } = &entry.outcome {
             metadata.insert("error_code".to_string(), Value::String(c.clone()));
        }
        
        if let Ok(json_str) = serde_json::to_string(&entry) {
            metadata.insert("full_entry".to_string(), Value::String(json_str));
        }

        // 3. Send to API
        let payload = json!({
            "content": text,
            "metadata": metadata,
            "collection": self.collection,
            "wait": false // Fire and forget on the server side
        });

        let url = format!("{}/v1/ingest", self.base_url);
        
        // We spawn this off to not block the return? No, async is fine, but we swallow errors.
        let result = self.client.post(&url)
            .json(&payload)
            .send()
            .await;

        match result {
            Ok(res) => {
                if !res.status().is_success() {
                    warn!("ADT Log Failed: {} - {}", res.status(), res.text().await.unwrap_or_default());
                }
            },
            Err(e) => {
                // This is expected if server is down. Don't spam error logs unless Debug.
                info!("ADT Offline (Logging skipped): {}", e);
            }
        }

        Ok(())
    }

    /// Query for prophetic warnings before an operation.
    /// 
    /// Returns any `WisdomEntry`s that match the current context and represented failures.
    pub async fn query_prophecy(&self, tool_name: &str, _args_hash: &str) -> Result<Vec<WisdomEntry>> {
        // Query: "Tool [NAME] failed"
        let query = format!("Tool {} failed", tool_name);
        
        // Filter: tool = NAME, outcome = failure
        let filter = json!({
            "must": [
                { "key": "tool", "match": { "value": tool_name } },
                { "key": "outcome", "match": { "value": "failure" } }
            ]
        });

        let payload = json!({
            "query": query,
            "collection": self.collection,
            "limit": 5,
            "filter": filter
        });

        let url = format!("{}/v1/search", self.base_url);
        
        let result = self.client.post(&url)
            .json(&payload)
            .send()
            .await;

        match result {
            Ok(res) => {
                if res.status().is_success() {
                    let body: Value = res.json().await.unwrap_or(json!([]));
                    
                    // Parse "result" or "results" array
                    let items = if let Some(arr) = body.get("results").and_then(|v| v.as_array()) {
                        arr
                    } else if let Some(arr) = body.as_array() {
                        arr
                    } else {
                        return Ok(Vec::new());
                    };

                    let mut wisdoms = Vec::new();
                    for item in items {
                         if let Some(metadata) = item.get("metadata") && let Some(entry_val) = metadata.get("full_entry") {
                                 // Metadata values might come back as strings if the DB un-nested them
                                 let entry_str = if let Some(s) = entry_val.as_str() {
                                     s
                                 } else {
                                     // It might be a raw JSON object already
                                     &entry_val.to_string()
                                 };

                                 if let Ok(entry) = serde_json::from_str::<WisdomEntry>(entry_str) {
                                     wisdoms.push(entry);
                                 }
                         }
                    }
                    return Ok(wisdoms);
                } else {
                    warn!("ADT Search Failed: {}", res.status());
                }
            },
            Err(e) => {
                info!("ADT Offline (Prophecy skipped): {}", e);
            }
        }
        
        Ok(Vec::new())
    }
}
