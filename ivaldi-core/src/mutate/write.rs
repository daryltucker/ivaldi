use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;
use crate::{IvaldiResponse, AdvisoryMessage};
use crate::error::IvaldiError;
use crate::undo::{Journal, types::{JournalEntry, ActionType}};
use super::types::WriteFileArgs;
use super::backup::{create_backup, sha256_digest};

pub fn write_file(
    root: &Path,
    args: WriteFileArgs,
    journal: &Journal
) -> IvaldiResponse<PathBuf> {
    // 0. Path Validation relative to root? (Assuming path is absolute/valid for now, or processed by caller)
    let path = args.path.to_path_buf();
    let content = &args.content;
    
    // Prepare content (may be modified if append mode)
    let final_content: String;
    
    // 1. Determine Target & Actions
    let target_path = path.clone();
    let mut backup_ref: Option<PathBuf> = None;
    let mut checksum_before: Option<String> = None;
    let mut action_type = ActionType::Create; // Default


    // 2. HEURISTICS: Git Awareness & Multi-Advisories
    let mut advisories = Vec::new();
    if let Some(adv) = crate::heuristics::GitAwareness::apply(&path) {
         advisories.push(adv);
    }

    if target_path.exists() {
        if !target_path.is_file() {
             return IvaldiResponse::error("write_error", "Target exists and is not a file");
        }

        // Smart Default: Append unless --force is specified
        let should_append = args.append || !args.overwrite;
        
        if should_append {
            // APPEND MODE: Read existing content, append new content
            action_type = ActionType::Update;
            
            let existing_content = match fs::read_to_string(&target_path) {
                Ok(c) => c,
                Err(e) => return IvaldiResponse::error("read_error", format!("Failed to read existing file for append: {}", e)),
            };
            
            // Count lines before append
            let lines_before = existing_content.lines().count();
            let bytes_before = existing_content.len();
            
            // Backup before modifying (using root)
            match create_backup(root, &target_path) {
                Ok((bp, checksum)) => {
                    backup_ref = Some(bp);
                    checksum_before = Some(checksum);
                },
                Err(e) => return IvaldiResponse::error("backup_failed", e.to_string()),
            }
            
            // Combine existing + new content
            final_content = format!("{}{}", existing_content, content);
            
            // Advisory: Show pre-append state
            if !args.append {
                // Implicit append (default behavior)
                advisories.push(AdvisoryMessage::tool_info(format!(
                    "File existed. Appended to end. Original: {} lines, {} bytes. Use --force to overwrite instead.",
                    lines_before, bytes_before
                )));
            } else {
                // Explicit append flag
                advisories.push(AdvisoryMessage::tool_info(format!(
                    "Appended to end. Original: {} lines, {} bytes.",
                    lines_before, bytes_before
                )));
            }
        } else {
            // OVERWRITE MODE: Backup -> Overwrite
            action_type = ActionType::Update;
            
            // BACKUP (using root)
            match create_backup(root, &target_path) {
                Ok((bp, checksum)) => {
                    backup_ref = Some(bp);
                    checksum_before = Some(checksum);
                },
                Err(e) => return IvaldiResponse::from_error(IvaldiError::Internal(format!("Backup failed: {}", e))),
            }
            
            final_content = content.to_string();
        }
    } else {
        // File doesn't exist, create new
        final_content = content.to_string();
    }

    // 2. Atomic Write

    // Create temp file in same directory (to ensure same mount for atomic rename)
    let parent = target_path.parent().unwrap_or(Path::new("."));
    // Ensure parent exists
    if let Err(e) = fs::create_dir_all(parent) {
         return IvaldiResponse::from_error(IvaldiError::Io(e));
    }


    let mut temp_file = match tempfile::NamedTempFile::new_in(parent) {
        Ok(f) => f,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Io(e)),
    };

    if let Err(e) = temp_file.write_all(final_content.as_bytes()) {
        return IvaldiResponse::from_error(IvaldiError::Io(e));
    }
    
    // Checksum After
    let checksum_after = sha256_digest(final_content.as_bytes());


    // Commit (Persist)
    if let Err(e) = temp_file.persist(&target_path) {
         // HEURISTICS: Permission Fixer
         // PersistError can be converted to io::Error
         let io_err = std::io::Error::other(e.to_string());
         if let Some(adv) = crate::heuristics::PermissionFixer::apply(&target_path, &io_err) {
             return IvaldiResponse::from_error(IvaldiError::Io(io_err)).with_advisory(adv);
         }
         return IvaldiResponse::from_error(IvaldiError::Io(io_err));
    }
    
    // HEURISTICS: Syntax Guard
    if let Some(adv) = crate::heuristics::SyntaxGuard::apply(&target_path) {
        advisories.push(adv);
    }
    // Permission Fix? (persist default perms are usually 600, might want 644?)
    // Ignoring for internal tool loop.

    // 3. Journal
    let mut entry = JournalEntry::new(action_type, target_path.clone());
    entry.checksum_before = checksum_before;
    entry.checksum_after = Some(checksum_after);
    entry.backup_ref = backup_ref;
    entry.actor = Some("agent-mcp".to_string()); // Hardcoded for now as author removed from args
    
    if let Err(e) = journal.append(&entry) {
        // Write succeeded but journal failed. Warn but don't fail operation?
        // "Silence is Deadly" - we should probably error or at least STRONG warn.
        // Since the file IS written, returning Error implies simple retry might duplicate sidecars.
        // Let's returns Success but with Critical Advisory?
        // Or just fail. Journal failure is critical for Undo.
         // Ideally we'd rollback. But let's error for now.
         return IvaldiResponse::from_error(IvaldiError::Journal(format!("Write succeeded but journal failed: {}", e)));
    }

    IvaldiResponse::success_with_advisory(target_path, advisories)
}
