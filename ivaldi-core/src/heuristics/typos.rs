use std::path::Path;
use crate::advisory::AdvisoryMessage;
use super::Heuristic;

/// SiblingTyposHint (Restored)
pub struct SiblingTyposHint;
impl Heuristic for SiblingTyposHint {
    fn id(&self) -> &'static str { "sibling_typos" }
    fn description(&self) -> &'static str { "Suggests correct filename based on siblings" }
    // We only use the static helper `apply` for now in the main code, 
    // but implementing the trait is good for future registry usage.
}

impl SiblingTyposHint {
    pub fn apply(path: &Path, error: &std::io::Error) -> Option<AdvisoryMessage> {
        // Only trigger on NotFound
        if error.kind() != std::io::ErrorKind::NotFound {
            return None;
        }

        let file_name = path.file_name()?.to_string_lossy();
        let parent = path.parent()?;
        
        if !parent.exists() {
            return None; // Parent doesn't exist, can't list
        }

        let mut suggestions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                 let name = entry.file_name().to_string_lossy().to_string();
                 if name == file_name { continue; } // Exact match should have been found unless race condition

                 // Use strsim for levenshtein
                 let distance = strsim::levenshtein(&file_name, &name);
                 
                 // Heuristic threshold:
                 // 1 for short strings (< 5 chars)
                 // 2 for medium strings
                 // 3 for long strings (> 10 chars)
                 let threshold = if file_name.len() < 5 { 1 } else if file_name.len() < 10 { 2 } else { 3 };
                 
                 if distance > 0 && distance <= threshold {
                     suggestions.push(name);
                 }
            }
        }

        if !suggestions.is_empty() {
             let content = serde_json::json!({
                 "error_type": "not_found",
                 "alternatives": suggestions
             });
             
             let action = format!("Did you mean: {}?", suggestions.join(", "));
             return Some(AdvisoryMessage::tool_info(content).with_action(action));
        }

        None 
    }
}
