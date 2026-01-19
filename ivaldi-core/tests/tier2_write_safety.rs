use ivaldi_core::ResponseStatus;
use ivaldi_core::undo::{Journal, types::ActionType};
use ivaldi_core::mutate::{Mutator, WriteFileArgs};
use tempfile::TempDir;
use std::fs;


#[test]
fn test_write_new_file() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join("journal.jsonl")).unwrap();
    
    let target = root.join("new.txt");
    let args = WriteFileArgs {
        path: target.clone(),
        content: "Hello".to_string(),
        overwrite: false,
        append: false,
    };
    let result = Mutator::write_file(root, args, &journal);
    
    assert_eq!(result.status, ResponseStatus::Success);
    assert_eq!(fs::read_to_string(&target).unwrap(), "Hello");
    
    let entries = journal.read_all().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].action, ActionType::Create);
    assert_eq!(entries[0].path, target);
}

#[test]
fn test_smart_append_behavior() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join("journal.jsonl")).unwrap();
    
    let target = root.join("exists.txt");
    fs::write(&target, "Old Content").unwrap();
    
    // Attempt write without overwrite flag -> Should Append
    let args = WriteFileArgs {
        path: target.clone(),
        content: "New Content".to_string(),
        overwrite: false,
        append: false,
    };
    let result = Mutator::write_file(root, args, &journal);
    
    assert_eq!(result.status, ResponseStatus::Success);
    
    // Check advisory
    assert!(!result.advisory.is_empty());
    let has_append = result.advisory.iter().any(|a| a.content.as_str().unwrap_or("").contains("Appended"));
    assert!(has_append, "Expected advisory about Appending. Got: {:?}", result.advisory);
    
    // Content should be appended
    assert_eq!(fs::read_to_string(&target).unwrap(), "Old ContentNew Content");
    
    // Journal should record Update (Append)
    let entries = journal.read_all().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].action, ActionType::Update);
    assert!(entries[0].backup_ref.is_some());
}

#[test]
fn test_overwrite_backup() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join("journal.jsonl")).unwrap();
    
    let target = root.join("overwrite.txt");
    fs::write(&target, "Original").unwrap();
    
    // Write with overwrite=true
    let args = WriteFileArgs {
        path: target.clone(),
        content: "Modified".to_string(),
        overwrite: true,
        append: false,
    };
    let result = Mutator::write_file(root, args, &journal);
    
    assert_eq!(result.status, ResponseStatus::Success);
    assert_eq!(fs::read_to_string(&target).unwrap(), "Modified");
    
    let entries = journal.read_all().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].action, ActionType::Update);
    assert!(entries[0].backup_ref.is_some());
    assert!(entries[0].checksum_before.is_some());
    
    // Verify backup exists
    let backup_path = entries[0].backup_ref.as_ref().unwrap();
    assert!(backup_path.exists());
    assert_eq!(fs::read_to_string(backup_path).unwrap(), "Original");
}
