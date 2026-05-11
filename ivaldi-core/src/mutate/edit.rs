use std::path::{Path, PathBuf};
use std::fs;
use vecdb_common::FileType;
use crate::IvaldiResponse;
use crate::error::IvaldiError;
use crate::undo::Journal;
use super::types::{EditFileArgs, EditFilesArgs, WriteFileArgs, EditPreview};
use super::write::write_file;

/// Edit a file surgically (The Scalpel).
/// 
/// If `preview: true` is set in args, returns EditPreview with diff without applying changes.
pub async fn edit_file(
    root: &Path,
    args: EditFileArgs,
    journal: &Journal,
) -> IvaldiResponse<serde_json::Value> {

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

    // 2. Perform Edit with Crime Scene error handling
    let file_type = FileType::from_path(&args.path);
    let outcome = match crate::ast_edit::edit_content(&content, file_type, selector, &args.replacement).await {
        Ok(o) => o,
        Err(rich_error) => {
            // Convert RichEditError to IvaldiResponse with Crime Scene advisory
            return match rich_error {
                crate::ast_edit::RichEditError::NoMatch(mut context) => {
                    // Set the actual file path
                    context.file_info.path = args.path.to_string_lossy().to_string();
                    let mut response = IvaldiResponse::from_error(IvaldiError::Query("No nodes matched query".into()));
                    response.advisory.push(context.to_advisory());
                    response
                },
                crate::ast_edit::RichEditError::Ambiguous(mut context) => {
                    // Set the actual file path
                    context.file_info.path = args.path.to_string_lossy().to_string();
                    let mut response = IvaldiResponse::from_error(IvaldiError::Query("Ambiguous edit: multiple nodes matched".into()));
                    response.advisory.push(context.to_advisory());
                    response
                },
                crate::ast_edit::RichEditError::InvalidLineRange(msg) => {
                    IvaldiResponse::from_error(IvaldiError::InvalidArgument(msg))
                },
                crate::ast_edit::RichEditError::GrepNoMatch(mut context) => {
                    // Set the actual file path
                    context.file_info.path = args.path.to_string_lossy().to_string();
                    let mut response = IvaldiResponse::from_error(IvaldiError::Query("No line matched grep pattern".into()));
                    response.advisory.push(context.to_advisory());
                    response
                },
                crate::ast_edit::RichEditError::GrepAmbiguous(mut context) => {
                    // Set the actual file path
                    context.file_info.path = args.path.to_string_lossy().to_string();
                    let mut response = IvaldiResponse::from_error(IvaldiError::Query("Ambiguous edit: multiple lines matched grep pattern".into()));
                    response.advisory.push(context.to_advisory());
                    response
                },
                crate::ast_edit::RichEditError::MissingLineStart => {
                    IvaldiResponse::from_error(IvaldiError::Internal("AST node missing line_start".into()))
                },
                crate::ast_edit::RichEditError::MissingLineEnd => {
                    IvaldiResponse::from_error(IvaldiError::Internal("AST node missing line_end".into()))
                },
                crate::ast_edit::RichEditError::IndentationMismatch(repl, target) => {
                    IvaldiResponse::from_error(IvaldiError::InvalidArgument(
                        format!("Indentation mismatch: replacement has {} spaces, target has {} spaces. Write at indent 0 (tool shifts) or at exact target indent ({} spaces).", repl, target, target)
                    ))
                },
                crate::ast_edit::RichEditError::Vecq(e) => {
                    IvaldiResponse::from_error(IvaldiError::Query(format!("Vecq error: {}", e)))
                },
                crate::ast_edit::RichEditError::Anyhow(e) => {
                    IvaldiResponse::from_error(IvaldiError::Internal(format!("Internal error: {}", e)))
                },
            };
        }
    };

    let mut advisories = Vec::new();
    for h in &outcome.heuristics_triggered {
        let msg: String = match h.as_str() {
            "indentation_healing" => "Surgical content was indented to match the target site's structural depth.".into(),
            "anchor_trimming_leading" => "Leading anchor line detected and removed from replacement string.".into(),
            "anchor_trimming_trailing" => "Trailing anchor line detected and removed from replacement string.".into(),
            "grep_multi_line_replacement" => "NOTE: Grep matched 1 line but replacement has multiple lines. Only the matched line was replaced. Use 'from_line'/'to_line' or an AST query to replace multi-line blocks.".into(),
            _ if h.starts_with("indentation_mismatch:") => {
                // Parse: "indentation_mismatch:repl=N:target=M"
                let details: String = h.chars().skip("indentation_mismatch:".len()).collect();
                format!("Replacement indentation ({} spaces) differs from target site ({} spaces). Replacement was NOT re-indented.", 
                    details.split(':').find_map(|p| p.strip_prefix("repl=")).unwrap_or("?"),
                    details.split(':').find_map(|p| p.strip_prefix("target=")).unwrap_or("?"))
            },
            _ => "Surgery heuristic applied during edit.".into(),
        };
        advisories.push(crate::AdvisoryMessage::tool_info(msg));
    }

    let path_display = args.path.display().to_string();
    let final_content = outcome.content.clone();
    
    // === PREVIEW MODE ===
    if args.preview {
        let diff = similar::TextDiff::from_lines(&content, &final_content);
        let unified_diff = diff.unified_diff()
            .context_radius(3)
            .header(&path_display, &path_display)
            .to_string();
        
        // Count lines changed (simplified - count diff lines that are adds/removes)
        let lines_changed = unified_diff.lines()
            .filter(|l| l.starts_with('+') || l.starts_with('-'))
            .count();
        
        let preview = EditPreview {
            path: args.path.clone(),
            diff: unified_diff.clone(),
            original_preview: content.chars().take(500).collect(),
            modified_preview: final_content.chars().take(500).collect(),
            lines_changed,
            heuristics_triggered: outcome.heuristics_triggered.clone(),
        };
        
        let mut response: IvaldiResponse<EditPreview> = IvaldiResponse::success(preview);
        response.advisory.push(crate::AdvisoryMessage::tool_info(
            "PREVIEW MODE: No changes written. Set preview:false to apply."
        ));
        response.advisory.extend(advisories);
        
        if !unified_diff.is_empty() {
            response.ui_diffs.push(format!("```diff\n{}\n```", unified_diff));
        }
        
        // Convert to generic Response for return
        let generic_response = IvaldiResponse {
            is_error: response.is_error,
            content: response.content.map(|c| serde_json::to_value(c).unwrap_or(serde_json::Value::Null)),
            ui_diffs: response.ui_diffs,
            error: response.error,
            advisory: response.advisory,
        };
        
        return generic_response;
    }

    // === ACTUAL EDIT MODE ===
    
    // Construct WriteFileArgs
    let write_args = WriteFileArgs {
        path: args.path,
        content: outcome.content,
        overwrite: true, // ALWAYS overwrite here, as edit_file logic handles the merge
        append: false,
    };

    let mut response = write_file(root, write_args, journal);
    response.advisory.extend(advisories);
    
    // Generate Visual UI Diff (only if write_file didn't already produce one)
    // write_file generates its own diff; we add one only if write_file's path didn't produce a diff
    if response.ui_diffs.is_empty() {
        let diff = similar::TextDiff::from_lines(&content, &final_content);
        let unified_diff = diff.unified_diff().context_radius(3).header(&path_display, &path_display).to_string();
        if !unified_diff.is_empty() {
            response.ui_diffs.push(format!("```diff\n{}\n```", unified_diff));
        }
    }
    
    // Convert to generic Response for return
    IvaldiResponse {
        is_error: response.is_error,
        content: response.content.map(|c| serde_json::to_value(c).unwrap_or(serde_json::Value::Null)),
        ui_diffs: response.ui_diffs,
        error: response.error,
        advisory: response.advisory,
    }
}

/// Transactional multi-file edit.
/// Either all edits apply, or none (via rollback).
pub async fn edit_files(
    root: &Path,
    args: EditFilesArgs,
    journal: &Journal,
) -> IvaldiResponse<Vec<PathBuf>> {
    // PHASE 1: PREPARE (Read & Calculate New Content)
    // We use a local cache to track the "working state" of files during the transaction.
    // This allows multiple edits to the same file to build upon each other instead
    // of overwriting each other.
    let mut file_states: std::collections::HashMap<PathBuf, String> = std::collections::HashMap::new();
    let mut original_states: std::collections::HashMap<PathBuf, String> = std::collections::HashMap::new();
    let mut unique_paths = Vec::new();

    for edit_arg in args.edits {
        // Get the latest content (from cache if already touched, otherwise disk)
        let content = if let Some(cached_content) = file_states.get(&edit_arg.path) {
            cached_content.clone()
        } else {
            let disk_content = match fs::read_to_string(&edit_arg.path) {
                Ok(c) => c,
                Err(e) => return IvaldiResponse::error("read_error", format!("Failed to read {}: {}", edit_arg.path.display(), e)),
            };
            unique_paths.push(edit_arg.path.clone());
            original_states.insert(edit_arg.path.clone(), disk_content.clone());
            disk_content
        };
        
        // Selector logic
        let selector = if let Some(q) = &edit_arg.query {
                crate::ast_edit::EditSelector::Node(q.to_string())
        } else if let Some(g) = &edit_arg.grep {
                crate::ast_edit::EditSelector::Grep(g.to_string())
        } else if let (Some(f), Some(t)) = (edit_arg.from_line, edit_arg.to_line) {
                crate::ast_edit::EditSelector::Lines(f, t)
        } else {
                return IvaldiResponse::error("invalid_args", format!("Invalid args for {}: Selector required", edit_arg.path.display()));
        };
        
        // Apply Edit to the CURRENT state (could be already modified in this turn)
        let file_type = FileType::from_path(&edit_arg.path);
        let outcome = match crate::ast_edit::edit_content(&content, file_type, selector, &edit_arg.replacement).await {
            Ok(o) => o,
            Err(e) => return IvaldiResponse::error("edit_error", format!("Failed to edit {}: {}", edit_arg.path.display(), e)),
        };
        
        // Update cache for next possible edit on this path
        file_states.insert(edit_arg.path.clone(), outcome.content);
    }
    
    // Convert cached final states to WriteFileArgs
    let mut prepared_writes = Vec::new();
    for path in &unique_paths {
        if let Some(content) = file_states.get(path) {
            prepared_writes.push(WriteFileArgs {
                path: path.clone(),
                content: content.clone(),
                overwrite: true,
                append: false,
            });
        }
    }
    
    // PHASE 2: COMMIT (Write with Rollback)
    let mut success_paths = Vec::new();
    let mut undo_count = 0;
    let mut validation_error = None;
    
    for write_arg in prepared_writes {
        let resp = write_file(root, write_arg, journal);
        
        if resp.is_error {
            // Write failed
            validation_error = Some(format!("Write failed: {:?}", resp.error));
            break;
        }
        // Unwrap panic safe if status is not Error (content should be Some)
        if let Some(path) = resp.content {
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
    
    // Generate UI Diffs for the transaction
    let mut response = IvaldiResponse::success(success_paths);
    for path in &unique_paths {
        if let (Some(orig), Some(new)) = (original_states.get(path), file_states.get(path)) {
            let diff = similar::TextDiff::from_lines(orig, new);
            let unified_diff = diff.unified_diff().context_radius(3).header(&path.display().to_string(), &path.display().to_string()).to_string();
            if !unified_diff.is_empty() {
                response.ui_diffs.push(format!("```diff\n{}\n```", unified_diff));
            }
        }
    }
    
    response
}
