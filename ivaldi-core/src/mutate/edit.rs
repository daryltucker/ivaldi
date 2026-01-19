use std::path::{Path, PathBuf};
use std::fs;
use vecdb_common::FileType;
use crate::IvaldiResponse;
use crate::error::IvaldiError;
use crate::undo::Journal;
use super::types::{EditFileArgs, EditFilesArgs, WriteFileArgs};
use super::write::write_file;

/// Edit a file surgically (The Scalpel).
pub async fn edit_file(
    root: &Path,
    args: EditFileArgs,
    journal: &Journal,
) -> IvaldiResponse<PathBuf> {

    // 1. Read current content
    let content = match fs::read_to_string(&args.path) {
        Ok(c) => c,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Io(e)),
    };

    // Determine selector
    let selector = if let Some(q) = &args.query {
            crate::ast_edit::EditSelector::Node(q.to_string())
    } else if let Some(g) = &args.grep {
            crate::ast_edit::EditSelector::Grep(g.to_string())
    } else if let (Some(f), Some(t)) = (args.from_line, args.to_line) {
            crate::ast_edit::EditSelector::Lines(f, t)
    } else {
            return IvaldiResponse::from_error(IvaldiError::InvalidArgument("Edit requires query, grep, or from_line/to_line".into()));
    };

    // 2. Perform Edit
    let file_type = FileType::from_path(&args.path);
    let new_content = match crate::ast_edit::edit_content(&content, file_type, selector, &args.replacement).await {
        Ok(c) => c,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Internal(format!("Edit failed: {}", e))),
    };

    // 3. Write via write_file
    // Construct WriteFileArgs
    let write_args = WriteFileArgs {
        path: args.path,
        content: new_content,
        overwrite: args.overwrite,
        append: false,
    };

    write_file(root, write_args, journal)
}

/// Transactional multi-file edit.
/// Either all edits apply, or none (via rollback).
pub async fn edit_files(
    root: &Path,
    args: EditFilesArgs,
    journal: &Journal,
) -> IvaldiResponse<Vec<PathBuf>> {
    // PHASE 1: PREPARE (Read & Calculate New Content)
    let mut prepared_writes = Vec::new();
    
    for edit_arg in args.edits {
        // Read
        let content = match fs::read_to_string(&edit_arg.path) {
            Ok(c) => c,
            Err(e) => return IvaldiResponse::error("read_error", format!("Failed to read {}: {}", edit_arg.path.display(), e)),
        };
        
        // Selector logic (duplicated from edit_file, maybe extract?)
        let selector = if let Some(q) = &edit_arg.query {
                crate::ast_edit::EditSelector::Node(q.to_string())
        } else if let Some(g) = &edit_arg.grep {
                crate::ast_edit::EditSelector::Grep(g.to_string())
        } else if let (Some(f), Some(t)) = (edit_arg.from_line, edit_arg.to_line) {
                crate::ast_edit::EditSelector::Lines(f, t)
        } else {
                return IvaldiResponse::error("invalid_args", format!("Invalid args for {}: Selector required", edit_arg.path.display()));
        };
        
        // Edit
        let file_type = FileType::from_path(&edit_arg.path);
        let new_content = match crate::ast_edit::edit_content(&content, file_type, selector, &edit_arg.replacement).await {
            Ok(c) => c,
            Err(e) => return IvaldiResponse::error("edit_error", format!("Failed to edit {}: {}", edit_arg.path.display(), e)),
        };
        
        // Store for Phase 2
        prepared_writes.push(WriteFileArgs {
            path: edit_arg.path,
            content: new_content,
            overwrite: edit_arg.overwrite,
            append: false,
        });
    }
    
    // PHASE 2: COMMIT (Write with Rollback)
    let mut success_paths = Vec::new();
    let mut undo_count = 0;
    let mut validation_error = None;
    
    for write_arg in prepared_writes {
        let resp = write_file(root, write_arg, journal);
        
        if resp.status == crate::response::ResponseStatus::Error {
            // Write failed
            validation_error = Some(format!("Write failed: {:?}", resp.error));
            break;
        }
        // Unwrap panic safe if status is not Error (result should be Some)
        if let Some(path) = resp.result {
            success_paths.push(path);
            undo_count += 1;
        } else {
                validation_error = Some("Write succeeded but returned no path".to_string());
                break;
        }
    }
    
    // PHASE 3: ROLLBACK (If needed)
    if let Some(err) = validation_error {
            // Rollback `undo_count` times
            for _ in 0..undo_count {
                let _ = crate::undo::Undoer::undo_last(root, journal);
            }
            return IvaldiResponse::error("transaction_failed", format!("Transaction aborted. Rolled back {} changes. Cause: {}", undo_count, err));
    }
    
    IvaldiResponse::success(success_paths)
}
