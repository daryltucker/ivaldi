use std::path::Path;
use crate::advisory::AdvisoryMessage;
use serde_json::Value;
use super::Heuristic;

/// Provides complete permission/ownership snapshot on EACCES.
/// Follows the "Crime Scene" principle: report facts, not solutions.
pub struct PermissionFixer;

impl Heuristic for PermissionFixer {
    fn id(&self) -> &'static str { "permission_fixer" }
    fn description(&self) -> &'static str { "Provides permission/ownership state snapshot on EACCES" }

    fn check_post(&self, path: &Path, _op: &str, error: Option<&crate::response::ErrorDetail>) -> Option<AdvisoryMessage> {
        if let Some(err) = error {
            // Check if it's a permission error (EACCES / 13)
            let is_permission_error = err.code == "13" 
                || err.code == "EACCES"
                || err.message.contains("Permission denied")
                || err.message.contains("OS Error 13");

            if is_permission_error {
                let mut context = serde_json::Map::new();
                context.insert("error_code".to_string(), "EACCES".into());
                context.insert("path".to_string(), path.to_string_lossy().into());

                // Process Context (who am I?)
                let mut process_info = serde_json::Map::new();
                #[cfg(unix)]
                {
                    unsafe {
                        process_info.insert("uid".to_string(), libc::getuid().into());
                        process_info.insert("gid".to_string(), libc::getgid().into());
                        process_info.insert("euid".to_string(), libc::geteuid().into());
                        process_info.insert("egid".to_string(), libc::getegid().into());
                    }
                }
                context.insert("process".to_string(), Value::Object(process_info));

                // Target Context (the file itself)
                let mut target_info = serde_json::Map::new();
                if path.exists() {
                    if let Ok(meta) = std::fs::metadata(path) {
                        target_info.insert("exists".to_string(), true.into());
                        target_info.insert("is_file".to_string(), meta.is_file().into());
                        target_info.insert("is_dir".to_string(), meta.is_dir().into());
                        target_info.insert("size_bytes".to_string(), meta.len().into());
                        
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            use std::os::unix::fs::MetadataExt;
                            let mode = meta.permissions().mode();
                            target_info.insert("mode".to_string(), format!("{:o}", mode).into());
                            target_info.insert("mode_octal".to_string(), format!("0{:o}", mode & 0o7777).into());
                            target_info.insert("mode_symbolic".to_string(), format_mode_symbolic(mode).into());
                            target_info.insert("uid".to_string(), meta.uid().into());
                            target_info.insert("gid".to_string(), meta.gid().into());
                            let current_uid = unsafe { libc::getuid() };
                            let file_uid = meta.uid();
                            target_info.insert("readable".to_string(), ((mode & 0o400) != 0 && current_uid == file_uid).into());
                            target_info.insert("writable".to_string(), ((mode & 0o200) != 0 && current_uid == file_uid).into());
                            target_info.insert("executable".to_string(), ((mode & 0o100) != 0 && current_uid == file_uid).into());
                        }
                        #[cfg(not(unix))]
                        {
                            target_info.insert("readonly".to_string(), meta.permissions().readonly().into());
                        }
                    } else {
                        target_info.insert("exists".to_string(), true.into());
                        target_info.insert("metadata_error".to_string(), "Could not read metadata".into());
                    }
                } else {
                    target_info.insert("exists".to_string(), false.into());
                }
                context.insert("target".to_string(), Value::Object(target_info));

                // Parent Context (why can't I create/modify?)
                if let Some(parent) = path.parent() {
                    let mut parent_info = serde_json::Map::new();
                    parent_info.insert("path".to_string(), parent.to_string_lossy().into());
                    
                    if parent.exists() {
                        if let Ok(meta) = std::fs::metadata(parent) {
                            parent_info.insert("exists".to_string(), true.into());
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                use std::os::unix::fs::MetadataExt;
                                let mode = meta.permissions().mode();
                                parent_info.insert("mode".to_string(), format!("{:o}", mode).into());
                                parent_info.insert("mode_octal".to_string(), format!("0{:o}", mode & 0o7777).into());
                                parent_info.insert("mode_symbolic".to_string(), format_mode_symbolic(mode).into());
                                parent_info.insert("uid".to_string(), meta.uid().into());
                                parent_info.insert("gid".to_string(), meta.gid().into());
                                let current_uid = unsafe { libc::getuid() };
                                let parent_uid = meta.uid();
                                parent_info.insert("writable".to_string(), ((mode & 0o200) != 0 && current_uid == parent_uid).into());
                            }
                            #[cfg(not(unix))]
                            {
                                parent_info.insert("readonly".to_string(), meta.permissions().readonly().into());
                            }
                        } else {
                            parent_info.insert("exists".to_string(), true.into());
                            parent_info.insert("metadata_error".to_string(), "Could not read metadata".into());
                        }
                    } else {
                        parent_info.insert("exists".to_string(), false.into());
                    }
                    context.insert("parent".to_string(), Value::Object(parent_info));
                }

                return Some(AdvisoryMessage::tool_warn(Value::Object(context)));
            }
        }
        None
    }
}

impl PermissionFixer {
    pub fn apply(path: &Path, error: &std::io::Error) -> Option<AdvisoryMessage> {
       let detail = crate::response::ErrorDetail {
           code: error.raw_os_error().map(|i| i.to_string()).unwrap_or_else(|| "unknown".to_string()),
           message: error.to_string(),
           hint: None,
           context: None,
       };
       Self.check_post(path, "unknown", Some(&detail))
    }
}

// Helper function to format mode as symbolic (e.g., "rwxr-xr-x")
#[cfg(unix)]
fn format_mode_symbolic(mode: u32) -> String {
    let file_type = if (mode & 0o170000) == 0o040000 { 'd' } else { '-' };
    let user = format!(
        "{}{}{}",
        if mode & 0o400 != 0 { 'r' } else { '-' },
        if mode & 0o200 != 0 { 'w' } else { '-' },
        if mode & 0o100 != 0 { 'x' } else { '-' }
    );
    let group = format!(
        "{}{}{}",
        if mode & 0o040 != 0 { 'r' } else { '-' },
        if mode & 0o020 != 0 { 'w' } else { '-' },
        if mode & 0o010 != 0 { 'x' } else { '-' }
    );
    let others = format!(
        "{}{}{}",
        if mode & 0o004 != 0 { 'r' } else { '-' },
        if mode & 0o002 != 0 { 'w' } else { '-' },
        if mode & 0o001 != 0 { 'x' } else { '-' }
    );
    format!("{}{}{}{}", file_type, user, group, others)
}
