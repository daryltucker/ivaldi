//! Tier 2 Tests: Multi-Turn Sequential Edit Workflows
//!
//! **TRUTH**: Agents don't call `edit_files` (transaction). They call `edit_file` multiple times
//! in SEPARATE turns. Each call reads the CURRENT state of the file and modifies it.
//! This tests the REAL agent workflow.
//!
//! **BUG PATTERN**: Agent does edit → edit → edit → undo → files corrupted

use ivaldi_core::mutate::{Mutator, WriteFileArgs, EditFileArgs, EditFilesArgs};
use ivaldi_core::undo::{Journal, Undoer};
use tempfile::TempDir;
use std::fs;

/// Test: Three sequential edit_file calls (the REAL agent pattern)
#[tokio::test]
async fn test_sequential_edit_file_calls_x3() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join("journal.jsonl")).unwrap();
    let file = root.join("test.rs");
    
    // 1. Create file with known content
    let original = "fn old() {}\nfn test() {}\n";
    let res = Mutator::write_file(root, WriteFileArgs {
        path: file.clone(),
        content: original.to_string(),
        overwrite: true,
        append: false,
    }, &journal);
    assert!(res.is_success());
    
    // 2. First edit (change line 1)
    let result1 = Mutator::edit_file(root, EditFileArgs {
        path: file.clone(),
        grep: Some("fn old".to_string()),
        replacement: "fn new() {}".to_string(),
        query: None,
        from_line: None,
        to_line: None,
        preview: false,
    }, &journal).await;
    assert!(result1.is_success(), "First edit failed: {:?}", result1.error);
    
    // 3. Second edit (change line 2) - reads CURRENT file state
    let result2 = Mutator::edit_file(root, EditFileArgs {
        path: file.clone(),
        grep: Some("fn test".to_string()),
        replacement: "fn modified() {}".to_string(),
        query: None,
        from_line: None,
        to_line: None,
        preview: false,
    }, &journal).await;
    assert!(result2.is_success(), "Second edit failed: {:?}", result2.error);
    
    // 4. Third edit - add a new function
    let result3 = Mutator::edit_file(root, EditFileArgs {
        path: file.clone(),
        grep: Some("fn modified".to_string()),
        replacement: "fn modified() {}\n\nfn added() {}".to_string(),
        query: None,
        from_line: None,
        to_line: None,
        preview: false,
    }, &journal).await;
    assert!(result3.is_success(), "Third edit failed: {:?}", result3.error);
    
    // Verify final content
    let content = fs::read_to_string(&file).unwrap();
    assert!(content.contains("fn new()"), "fn new() not in result");
    assert!(content.contains("fn modified()"), "fn modified() not in result");
    assert!(content.contains("fn added()"), "fn added() not in result");
    
    // Verify NO duplication (this is what agents were experiencing!)
    let new_count = content.matches("fn new").count();
    let modified_count = content.matches("fn modified").count();
    let added_count = content.matches("fn added").count();
    
    assert_eq!(new_count, 1, "fn new duplicated {} times", new_count);
    assert_eq!(modified_count, 1, "fn modified duplicated {} times", modified_count);
    assert_eq!(added_count, 1, "fn added duplicated {} times", added_count);
}

/// Test: Sequential edits → undo twice → verify correct restoration
#[tokio::test]
async fn test_sequential_edits_then_undo_x2() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join("journal.jsonl")).unwrap();
    let file = root.join("test.txt");
    
    // 1. Create file
    let original = "Line 1\nLine 2\nLine 3\n";
    let res = Mutator::write_file(root, WriteFileArgs {
        path: file.clone(),
        content: original.to_string(),
        overwrite: true,
        append: false,
    }, &journal);
    assert!(res.is_success());
    
    // 2. Edit 1: change Line 2
    Mutator::edit_file(root, EditFileArgs {
        path: file.clone(),
        grep: Some("Line 2".to_string()),
        replacement: "MODIFIED 2".to_string(),
        query: None,
        from_line: None,
        to_line: None,
        preview: false,
    }, &journal).await;
    
    // 3. Edit 2: change Line 3
    Mutator::edit_file(root, EditFileArgs {
        path: file.clone(),
        grep: Some("Line 3".to_string()),
        replacement: "MODIFIED 3".to_string(),
        query: None,
        from_line: None,
        to_line: None,
        preview: false,
    }, &journal).await;
    
    // 4. Undo once - should restore Edit 1 result
    let undo1 = Undoer::undo_last(root, &journal);
    assert!(undo1.is_success(), "First undo failed: {:?}", undo1.error);
    
    let content_after_undo1 = fs::read_to_string(&file).unwrap();
    assert!(content_after_undo1.contains("MODIFIED 2"), "After undo1: MODIFIED 2 missing");
    assert!(content_after_undo1.contains("Line 3"), "After undo1: Line 3 not restored");
    
    // 5. Undo twice - should restore original
    let undo2 = Undoer::undo_last(root, &journal);
    assert!(undo2.is_success(), "Second undo failed: {:?}", undo2.error);
    
    let content_after_undo2 = fs::read_to_string(&file).unwrap();
    assert_eq!(content_after_undo2, original, "After undo2: should be original content");
}

/// Test: edit_file → undo → edit again (the "recovery loop" agents fall into)
#[tokio::test]
async fn test_edit_undo_recovery_loop() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join("journal.jsonl")).unwrap();
    let file = root.join("test.txt");
    
    // Create file
    let res = Mutator::write_file(root, WriteFileArgs {
        path: file.clone(),
        content: "original\n".to_string(),
        overwrite: true,
        append: false,
    }, &journal);
    assert!(res.is_success());
    
    // Edit (this should work)
    Mutator::edit_file(root, EditFileArgs {
        path: file.clone(),
        grep: Some("original".to_string()),
        replacement: "changed".to_string(),
        query: None,
        from_line: None,
        to_line: None,
        preview: false,
    }, &journal).await;
    
    // Undo
    Undoer::undo_last(root, &journal);
    
    // Edit AGAIN (this is where bugs often manifest)
    let result = Mutator::edit_file(root, EditFileArgs {
        path: file.clone(),
        grep: Some("original".to_string()),
        replacement: "changed again".to_string(),
        query: None,
        from_line: None,
        to_line: None,
        preview: false,
    }, &journal).await;
    
    assert!(result.is_success(), "Edit after undo failed: {:?}", result.error);
    assert!(fs::read_to_string(&file).unwrap().contains("changed again"));
}

/// Test: Verify journal entry counts match operations
#[tokio::test]
async fn test_journal_entries_match_operations() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join("journal.jsonl")).unwrap();
    let file = root.join("test.txt");
    
    // Create file
    Mutator::write_file(root, WriteFileArgs {
        path: file.clone(),
        content: "A\nB\nC\n".to_string(),
        overwrite: true,
        append: false,
    }, &journal);
    
    // 3 edits
    Mutator::edit_file(root, EditFileArgs {
        path: file.clone(),
        grep: Some("A".to_string()),
        replacement: "A'".to_string(),
        query: None,
        from_line: None,
        to_line: None,
        preview: false,
    }, &journal).await;
    Mutator::edit_file(root, EditFileArgs {
        path: file.clone(),
        grep: Some("B".to_string()),
        replacement: "B'".to_string(),
        query: None,
        from_line: None,
        to_line: None,
        preview: false,
    }, &journal).await;
    Mutator::edit_file(root, EditFileArgs {
        path: file.clone(),
        grep: Some("C".to_string()),
        replacement: "C'".to_string(),
        query: None,
        from_line: None,
        to_line: None,
        preview: false,
    }, &journal).await;
    
    // Journal should have 4 entries (1 write + 3 edits)
    let entries = journal.read_all().unwrap();
    assert_eq!(entries.len(), 4, "Expected 4 journal entries, got {}", entries.len());
    
    // Last entry should be the most recent edit
    let last_entry = entries.last().unwrap();
    assert_eq!(last_entry.action, ivaldi_core::undo::types::ActionType::Update);
}

/// Test: edit_files (transaction) vs sequential edit_file calls produce same result
#[tokio::test]
async fn test_transaction_vs_sequential_equivalence() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    
    // File 1: Sequential calls
    let file1 = root.join("sequential.txt");
    let journal1 = Journal::open(root.join("journal1.jsonl")).unwrap();
    
    let res1 = Mutator::write_file(root, WriteFileArgs {
        path: file1.clone(),
        content: "A\nB\nC\n".to_string(),
        overwrite: true,
        append: false,
    }, &journal1);
    assert!(res1.is_success());
    
    Mutator::edit_file(root, EditFileArgs {
        path: file1.clone(),
        grep: Some("A".to_string()),
        replacement: "A1".to_string(),
        query: None,
        from_line: None,
        to_line: None,
        preview: false,
    }, &journal1).await;
    Mutator::edit_file(root, EditFileArgs {
        path: file1.clone(),
        grep: Some("B".to_string()),
        replacement: "B1".to_string(),
        query: None,
        from_line: None,
        to_line: None,
        preview: false,
    }, &journal1).await;
    
    // File 2: Transaction (same edits)
    let file2 = root.join("transaction.txt");
    let journal2 = Journal::open(root.join("journal2.jsonl")).unwrap();
    
    let res2 = Mutator::write_file(root, WriteFileArgs {
        path: file2.clone(),
        content: "A\nB\nC\n".to_string(),
        overwrite: true,
        append: false,
    }, &journal2);
    assert!(res2.is_success());
    
    Mutator::edit_files(root, EditFilesArgs {
        edits: vec![
            EditFileArgs {
                path: file2.clone(),
                grep: Some("A".to_string()),
                replacement: "A1".to_string(),
                query: None,
                from_line: None,
                to_line: None,
                preview: false,
            },
            EditFileArgs {
                path: file2.clone(),
                grep: Some("B".to_string()),
                replacement: "B1".to_string(),
                query: None,
                from_line: None,
                to_line: None,
                preview: false,
            },
        ],
    }, &journal2).await;
    
    // Both should produce the same content
    let content1 = fs::read_to_string(&file1).unwrap();
    let content2 = fs::read_to_string(&file2).unwrap();
    assert_eq!(content1, content2, "Sequential and transaction should produce same result");
}