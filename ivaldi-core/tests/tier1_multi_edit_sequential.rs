//! Tier 1 Tests: Sequential Multi-Editing
//!
//! **TRUTH**: edit_files must handle multiple edits to the same file sequentially,
//! ensuring each subsequent edit builds upon the previous one's result.

use ivaldi_core::mutate::{Mutator, WriteFileArgs, EditFileArgs, EditFilesArgs};
use ivaldi_core::undo::Journal;

use tempfile::tempdir;
use std::fs;

#[tokio::test]
async fn test_edit_files_sequential_on_same_path() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("sequential.txt");
    let journal = Journal::open(dir.path().join("journal.jsonl")).unwrap();

    // 1. Setup original file
    let original = "line 1\nline 2\nline 3\n";
    let args = WriteFileArgs {
        path: file.clone(),
        content: original.to_string(),
        overwrite: true,
        append: false,
    };
    Mutator::write_file(dir.path(), args, &journal);

    // 2. Prepare two edits for the SAME file
    // Edit 1: change line 1
    let edit1 = EditFileArgs {
        path: file.clone(),
        replacement: "REPLACED 1".to_string(),
        query: None,
        grep: Some("line 1".to_string()),
        from_line: None,
        to_line: None,
    };
    // Edit 2: change line 2
    let edit2 = EditFileArgs {
        path: file.clone(),
        replacement: "REPLACED 2".to_string(),
        query: None,
        grep: Some("line 2".to_string()),
        from_line: None,
        to_line: None,
    };

    let multi_args = EditFilesArgs {
        edits: vec![edit1, edit2],
    };

    // 3. Execute Transaction
    let result = Mutator::edit_files(dir.path(), multi_args, &journal).await;

    // 4. VERIFY
    assert!(result.is_success(), "Transaction failed: {:?}", result.error);
    let final_content = fs::read_to_string(&file).unwrap();
    
    // Both lines should be replaced
    let expected = "REPLACED 1\nREPLACED 2\nline 3\n";
    assert_eq!(final_content, expected, "Edits were not applied sequentially!");
}

#[tokio::test]
async fn test_edit_files_rollback_preserves_initial_state() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("rollback.txt");
    let journal = Journal::open(dir.path().join("journal.jsonl")).unwrap();

    // 1. Setup original file
    let original = "initial\n";
    Mutator::write_file(dir.path(), WriteFileArgs {
        path: file.clone(),
        content: original.to_string(),
        overwrite: true,
        append: false,
    }, &journal);

    // 2. Edit 1: valid
    let edit1 = EditFileArgs {
        path: file.clone(),
        replacement: "modified\n".to_string(),
        query: None,
        grep: Some("initial".to_string()),
        from_line: None,
        to_line: None,
    };
    
    // Edit 2: fails (path doesn't exist)
    let edit2 = EditFileArgs {
        path: dir.path().join("nonexistent.txt"),
        replacement: "fail".to_string(),
        query: None,
        grep: Some("any".to_string()),
        from_line: None,
        to_line: None,
    };

    let result = Mutator::edit_files(dir.path(), EditFilesArgs { edits: vec![edit1, edit2] }, &journal).await;

    // 3. VERIFY
    assert!(!result.is_success(), "Transaction should have failed");
    let content = fs::read_to_string(&file).unwrap();
    assert_eq!(content, original, "File should have been rolled back to initial state");
}
