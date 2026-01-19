//! Tier 1 Tests: Surgical Editing Fundamentals
//!
//! PHILOSOPHY: Test basic assumptions. Prove everything from ground up.
//! No fancy roundtrip validation - just direct file content verification.

use ivaldi_core::mutate::{Mutator, WriteFileArgs, EditFileArgs};
use ivaldi_core::undo::Journal;

use tempfile::tempdir;
use std::fs;

/// **TRUTH**: AST-based editing can replace a Rust function by name
#[tokio::test]
async fn test_ast_selector_replaces_rust_function() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("test.rs");
    let journal = Journal::open(dir.path().join("journal.jsonl")).unwrap();

    // Write original
    let original = "fn old() { 1 }\nfn keep() { 2 }\n";
    let args = WriteFileArgs {
        path: file.clone(),
        content: original.to_string(),
        overwrite: true,
        append: false,
    };
    Mutator::write_file(dir.path(), args, &journal);

    // Edit via AST selector
    let selector = ".functions[] | select(.name == \"old\")".to_string(); // AST selector string directly
    let replacement = "fn new() { 99 }".to_string();
    
    let edit_args = EditFileArgs {
        path: file.clone(),
        replacement,
        query: Some(selector),
        grep: None,
        from_line: None,
        to_line: None,
        overwrite: true,
    };
    let result = Mutator::edit_file(dir.path(), edit_args, &journal).await;

    // VERIFY: old replaced, keep preserved
    assert!(result.is_success(), "Edit failed: {:?}", result.error);
    let final_content = fs::read_to_string(&file).unwrap();
    assert!(final_content.contains("fn new() { 99 }"), "Replacement not found");
    assert!(final_content.contains("fn keep() { 2 }"), "Existing function lost");
    assert!(!final_content.contains("fn old()"), "Old function still present");
}

/// **TRUTH**: Grep-based editing can replace a single matching line
#[tokio::test]
async fn test_grep_selector_replaces_exact_line() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("test.txt");
    let journal = Journal::open(dir.path().join("journal.jsonl")).unwrap();

    let original = "line 1\ntarget line\nline 3\n";
    let args = WriteFileArgs {
        path: file.clone(),
        content: original.to_string(),
        overwrite: true,
        append: false,
    };
    Mutator::write_file(dir.path(), args, &journal);

    let edit_args = EditFileArgs {
        path: file.clone(),
        replacement: "REPLACED".to_string(),
        query: None,
        grep: Some("target line".to_string()),
        from_line: None,
        to_line: None,
        overwrite: true,
    };
    let result = Mutator::edit_file(dir.path(), edit_args, &journal).await;

    assert!(result.is_success());
    let final_content = fs::read_to_string(&file).unwrap();
    assert_eq!(final_content, "line 1\nREPLACED\nline 3\n");
}


/// **TRUTH**: Line-based editing can replace a specific range
#[tokio::test]
async fn test_line_range_selector_replaces_range() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("test.txt");
    let journal = Journal::open(dir.path().join("journal.jsonl")).unwrap();

    let original = "L1\nL2\nL3\nL4\n";
    let args = WriteFileArgs {
        path: file.clone(),
        content: original.to_string(),
        overwrite: true,
        append: false,
    };
    Mutator::write_file(dir.path(), args, &journal);

    let edit_args = EditFileArgs {
        path: file.clone(),
        replacement: "REPLACED_2_3".to_string(),
        query: None,
        grep: None,
        from_line: Some(2),
        to_line: Some(3),
        overwrite: true,
    };
    let result = Mutator::edit_file(dir.path(), edit_args, &journal).await;

    assert!(result.is_success());
    let final_content = fs::read_to_string(&file).unwrap();
    assert_eq!(final_content, "L1\nREPLACED_2_3\nL4\n");
}

/// **TRUTH**: AST edit fails gracefully when selector finds nothing
#[tokio::test]
async fn test_ast_selector_not_found_returns_error() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("test.rs");
    let journal = Journal::open(dir.path().join("journal.jsonl")).unwrap();

    let original = "fn existing() {}\n";
    let args = WriteFileArgs {
        path: file.clone(),
        content: original.to_string(),
        overwrite: true,
        append: false,
    };
    Mutator::write_file(dir.path(), args, &journal);

    let edit_args = EditFileArgs {
        path: file.clone(),
        replacement: "fn new() {}".to_string(),
        query: Some(".functions[] | select(.name == \"nonexistent\")".to_string()),
        grep: None,
        from_line: None,
        to_line: None,
        overwrite: true,
    };
    let result = Mutator::edit_file(dir.path(), edit_args, &journal).await;

    assert!(!result.is_success(), "Should fail when selector finds nothing");
    assert!(result.error.is_some());
    
    // VERIFY: Original file unchanged
    let final_content = fs::read_to_string(&file).unwrap();
    assert_eq!(final_content, original);
}

/// **TRUTH**: Grep edit fails when pattern matches multiple lines
#[tokio::test]
async fn test_grep_multiple_matches_returns_error() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("test.txt");
    let journal = Journal::open(dir.path().join("journal.jsonl")).unwrap();

    let original = "match\nother\nmatch\n";

    let args = WriteFileArgs {
        path: file.clone(),
        content: original.to_string(),
        overwrite: true,
        append: false,
    };
    Mutator::write_file(dir.path(), args, &journal);

    let edit_args = EditFileArgs {
        path: file.clone(),
        replacement: "REPLACED".to_string(),
        query: None,
        grep: Some("match".to_string()),
        from_line: None,
        to_line: None,
        overwrite: true,
    };
    let result = Mutator::edit_file(dir.path(), edit_args, &journal).await;

    assert!(!result.is_success(), "Should fail on ambiguous grep");
    
    // VERIFY: Original file unchanged
    let final_content = fs::read_to_string(&file).unwrap();
    assert_eq!(final_content, original);
}

/// **TRUTH**: Line range edit fails when range is invalid
#[tokio::test]
async fn test_line_range_invalid_returns_error() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("test.txt");
    let journal = Journal::open(dir.path().join("journal.jsonl")).unwrap();

    let original = "L1\nL2\n";
    let args = WriteFileArgs {
        path: file.clone(),
        content: original.to_string(),
        overwrite: true,
        append: false,
    };
    Mutator::write_file(dir.path(), args, &journal);

    // Try to edit lines 2-5 when file only has 2 lines
    let edit_args = EditFileArgs {
        path: file.clone(),
        replacement: "REPLACED".to_string(),
        query: None,
        grep: None,
        from_line: Some(2),
        to_line: Some(5),
        overwrite: true,
    };
    let result = Mutator::edit_file(dir.path(), edit_args, &journal).await;

    assert!(!result.is_success(), "Should fail when range exceeds file");
    
    // VERIFY: Original file unchanged
    let final_content = fs::read_to_string(&file).unwrap();
    assert_eq!(final_content, original);
}

/// **TRUTH**: Smart append default adds to existing file without --force
#[test]
fn test_write_default_appends_to_existing() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("test.txt");
    let journal = Journal::open(dir.path().join("journal.jsonl")).unwrap();

    // Write original
    let original = "Line 1\nLine 2\n";
    let args = WriteFileArgs {
        path: file.clone(),
        content: original.to_string(),
        overwrite: true,
        append: false,
    };
    let result1 = Mutator::write_file(dir.path(), args, &journal);
    assert!(result1.is_success());

    // Write again with default options (should append)
    let additional = "Line 3\n";
    let args_append = WriteFileArgs {
        path: file.clone(),
        content: additional.to_string(),
        overwrite: false,
        append: false,
    };
    let result2 = Mutator::write_file(dir.path(), args_append, &journal);
    assert!(result2.is_success());
    
    // Check for "Appended" advisory among all advisories
    let has_append_advisory = result2.advisory.iter().any(|a| {
        let msg = a.content.as_str().unwrap_or("");
        msg.contains("Appended") && msg.contains("2 lines")
    });
    assert!(has_append_advisory, "Should have advisory about append with line count. Advisories: {:?}", result2.advisory);

    // VERIFY: Content was appended
    let final_content = fs::read_to_string(&file).unwrap();
    assert_eq!(final_content, "Line 1\nLine 2\nLine 3\n");
}

#[test]
fn test_write_force_overwrites_existing() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("test.txt");
    let journal = Journal::open(dir.path().join("journal.jsonl")).unwrap();

    // Write original
    let original = "Old content\n";
    let args = WriteFileArgs {
        path: file.clone(),
        content: original.to_string(),
        overwrite: true,
        append: false,
    };
    Mutator::write_file(dir.path(), args, &journal);

    // Overwrite with force
    let new_content = "New content\n";
    let args_force = WriteFileArgs {
        path: file.clone(),
        content: new_content.to_string(),
        overwrite: true,
        append: false,
    };
    let result = Mutator::write_file(dir.path(), args_force, &journal);
    assert!(result.is_success());

    // VERIFY: Old content completely replaced
    let final_content = fs::read_to_string(&file).unwrap();
    assert_eq!(final_content, "New content\n");
    assert!(!final_content.contains("Old content"));
}