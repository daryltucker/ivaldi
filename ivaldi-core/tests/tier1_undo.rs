//! Tier 1 Tests: Undo (The Time Machine)
//!
//! Purpose: Verify that Undoer correctly reverts actions.

use ivaldi_core::mutate::{Mutator, WriteFileArgs};
use ivaldi_core::undo::{Journal, Undoer};
use tempfile::TempDir;
use std::fs;

#[test]
fn test_undo_create() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join("journal.jsonl")).unwrap();
    
    let target = root.join("to_delete.txt");
    
    // 1. Create file
    // 1. Create file
    let args = WriteFileArgs {
        path: target.clone(),
        content: "Data".to_string(),
        overwrite: false,
        append: false,
    };
    let res = Mutator::write_file(root, args, &journal);
    assert!(res.is_success());
    assert!(target.exists());
    
    // 2. Undo
    let undo_res = Undoer::undo_last(root, &journal);
    assert!(undo_res.is_success());
    
    // 3. Verify file gone
    assert!(!target.exists());
}

#[test]
fn test_undo_update_append() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join("journal.jsonl")).unwrap();
    
    let target = root.join("log.txt");
    fs::write(&target, "Line 1\n").unwrap();
    
    // 1. Append (Update)
    // 1. Append (Update)
    let args = WriteFileArgs {
        path: target.clone(),
        content: "Line 2\n".to_string(),
        overwrite: false,
        append: true,
    };
    let res = Mutator::write_file(root, args, &journal);
    assert!(res.is_success());
    assert_eq!(fs::read_to_string(&target).unwrap(), "Line 1\nLine 2\n");
    
    // 2. Undo
    let undo_res = Undoer::undo_last(root, &journal);
    assert!(undo_res.is_success());
    
    // 3. Verify content reverted (Backup restore)
    assert_eq!(fs::read_to_string(&target).unwrap(), "Line 1\n");
}

#[test]
fn test_undo_overwrite() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join("journal.jsonl")).unwrap();
    
    let target = root.join("config.txt");
    fs::write(&target, "v1").unwrap();
    
    // 1. Overwrite (Update)
    // 1. Overwrite (Update)
    let args = WriteFileArgs {
        path: target.clone(),
        content: "v2".to_string(),
        overwrite: true,
        append: false,
    };
    let res = Mutator::write_file(root, args, &journal);
    assert!(res.is_success());
    assert_eq!(fs::read_to_string(&target).unwrap(), "v2");
    
    // 2. Undo
    let undo_res = Undoer::undo_last(root, &journal);
    assert!(undo_res.is_success());
    
    // 3. Verify content reverted
    assert_eq!(fs::read_to_string(&target).unwrap(), "v1");
}

#[test]
fn test_undo_safety_checksum_failure() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join("journal.jsonl")).unwrap();
    
    let target = root.join("tampered.txt");
    
    // 1. Create
    // 1. Create
    let args = WriteFileArgs {
        path: target.clone(),
        content: "Original".to_string(),
        overwrite: false,
        append: false,
    };
    Mutator::write_file(root, args, &journal);
    
    // 2. Tamper external
    fs::write(&target, "Tampered").unwrap();
    
    // 3. Undo should fail
    let undo_res = Undoer::undo_last(root, &journal);
    assert!(!undo_res.is_success());
    assert!(undo_res.error.unwrap().message.contains("changed since last operation"));
    
    // 4. Verify content keeps tamper (safe)
    assert_eq!(fs::read_to_string(&target).unwrap(), "Tampered");
}
