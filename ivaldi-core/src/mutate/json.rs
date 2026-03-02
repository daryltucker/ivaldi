//! # JSON Editing Operations
//!
//! ## PURPOSE
//! Semantic editing of JSON documents using JSONPath selectors.
//!
//! ## PHILOSOPHY
//! - **Structure-Aware**: Understand JSON hierarchy for precise targeting
//! - **Key-Value Operations**: Replace, add, delete keys with full context
//! - **Preservation**: Maintain formatting and comments where possible

use super::write::write_file;
use super::WriteFileArgs;
use crate::error::IvaldiError;
use crate::undo::Journal;
use crate::IvaldiResponse;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Arguments for JSON editing operations
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EditJsonArgs {
    /// Path to the JSON file
    pub path: PathBuf,
    /// JSONPath selector (e.g., "$.provider", "$.provider.openrouter.models")
    pub selector: String,
    /// Operation to perform
    pub operation: JsonOperation,
}

/// Types of JSON editing operations
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonOperation {
    /// Replace the value at the selector path
    Replace { value: Value },
    /// Add a new key-value pair at the selector path
    Add { key: String, value: Value },
    /// Delete the key at the selector path
    Delete { key: String },
    /// Merge an object into the object at the selector path
    Merge { object: Value },
}

/// Edit JSON files with semantic operations
pub fn edit_json(root: &Path, args: EditJsonArgs, journal: &Journal) -> IvaldiResponse<PathBuf> {
    // 1. Read and parse existing JSON
    let content = match std::fs::read_to_string(&args.path) {
        Ok(c) => c,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Io(e)),
    };

    let mut json: Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Serialization(e)),
    };

    // 2. Apply the JSON operation
    match apply_json_operation(&mut json, &args.selector, &args.operation) {
        Ok(changed) => {
            if !changed {
                // No change needed, but still return success
                return IvaldiResponse::success(args.path);
            }
        }
        Err(e) => {
            return IvaldiResponse::from_error(IvaldiError::Internal(format!(
                "JSON operation failed: {}",
                e
            )))
        }
    }

    // 3. Serialize back to JSON with nice formatting
    let new_content = match serde_json::to_string_pretty(&json) {
        Ok(c) => c,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Serialization(e)),
    };

    // 4. Write the updated JSON
    let write_args = WriteFileArgs {
        path: args.path,
        content: new_content,
        overwrite: true, // ALWAYS overwrite here as we have semantically merged the state
        append: false,
    };

    write_file(root, write_args, journal)
}

/// Apply a JSON operation to the value at the given selector path
fn apply_json_operation(
    json: &mut Value,
    selector: &str,
    operation: &JsonOperation,
) -> Result<bool, String> {
    // Parse JSONPath selector (simplified version for common cases)
    let path_parts = parse_json_path(selector)?;

    // Navigate to the target location
    let target = navigate_json_path(json, &path_parts)?;

    match operation {
        JsonOperation::Replace { value } => {
            *target = value.clone();
            Ok(true)
        }
        JsonOperation::Add { key, value } => {
            if let Value::Object(obj) = target {
                obj.insert(key.clone(), value.clone());
                Ok(true)
            } else {
                Err("Add operation requires object target".to_string())
            }
        }
        JsonOperation::Delete { key } => {
            if let Value::Object(obj) = target {
                Ok(obj.remove(key).is_some())
            } else {
                Err("Delete operation requires object target".to_string())
            }
        }
        JsonOperation::Merge { object } => {
            if let (Value::Object(target_obj), Value::Object(merge_obj)) = (target, object) {
                for (k, v) in merge_obj {
                    target_obj.insert(k.clone(), v.clone());
                }
                Ok(true)
            } else {
                Err("Merge operation requires object targets".to_string())
            }
        }
    }
}

/// Parse a simplified JSONPath selector into path parts
/// Supports: $ (root), $.key, $.key.subkey, etc.
fn parse_json_path(selector: &str) -> Result<Vec<String>, String> {
    if !selector.starts_with('$') {
        return Err("JSONPath must start with $".to_string());
    }

    if selector == "$" {
        return Ok(vec![]);
    }

    if !selector.starts_with("$.") {
        return Err("JSONPath must start with $.".to_string());
    }

    let parts: Vec<String> = selector[2..].split('.').map(|s| s.to_string()).collect();

    Ok(parts)
}

/// Navigate to the value at the JSON path, returning a mutable reference
fn navigate_json_path<'a>(
    json: &'a mut Value,
    path_parts: &[String],
) -> Result<&'a mut Value, String> {
    let mut current = json;

    for part in path_parts {
        match current {
            Value::Object(obj) => {
                current = obj
                    .get_mut(part)
                    .ok_or_else(|| format!("Key '{}' not found", part))?;
            }
            _ => return Err(format!("Cannot navigate into non-object at '{}'", part)),
        }
    }

    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_json_path() {
        assert_eq!(parse_json_path("$").unwrap(), Vec::<String>::new());
        assert_eq!(
            parse_json_path("$.provider").unwrap(),
            vec!["provider".to_string()]
        );
        assert_eq!(
            parse_json_path("$.provider.openrouter").unwrap(),
            vec!["provider".to_string(), "openrouter".to_string()]
        );
    }

    #[test]
    fn test_apply_json_operation_replace() {
        let mut json = json!({"provider": {"old": "value"}});
        let operation = JsonOperation::Replace {
            value: json!({"new": "value"}),
        };

        let changed = apply_json_operation(&mut json, "$.provider", &operation).unwrap();
        assert!(changed);
        assert_eq!(json["provider"], json!({"new": "value"}));
    }

    #[test]
    fn test_apply_json_operation_add() {
        let mut json = json!({"provider": {}});
        let operation = JsonOperation::Add {
            key: "openrouter".to_string(),
            value: json!({"models": {}}),
        };

        let changed = apply_json_operation(&mut json, "$.provider", &operation).unwrap();
        assert!(changed);
        assert_eq!(json["provider"]["openrouter"], json!({"models": {}}));
    }

    #[test]
    fn test_provider_replacement_preserves_other_keys() {
        // Test the exact scenario from the user's config update
        let mut json = json!({
            "$schema": "https://opencode.ai/config.json",
            "provider": {
                "old": "value"
            },
            "other_key": "preserved"
        });

        let new_provider = json!({
            "openrouter": {
                "models": {
                    "gpt-3.5-turbo": {"name": "GPT-3.5 Turbo"}
                }
            }
        });

        let operation = JsonOperation::Replace {
            value: new_provider,
        };

        let changed = apply_json_operation(&mut json, "$.provider", &operation).unwrap();
        assert!(changed);

        // Verify the provider was replaced
        assert_eq!(
            json["provider"]["openrouter"]["models"]["gpt-3.5-turbo"]["name"],
            "GPT-3.5 Turbo"
        );

        // Verify other keys were preserved
        assert_eq!(json["$schema"], "https://opencode.ai/config.json");
        assert_eq!(json["other_key"], "preserved");

        // Verify old provider is gone
        assert!(!json["provider"].get("old").is_some());
    }
}
