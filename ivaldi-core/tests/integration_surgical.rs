use std::fs;
use tempfile::tempdir;
use ivaldi_core::mutate::{Mutator, WriteFileArgs, EditFileArgs};
use ivaldi_core::undo::Journal;


#[tokio::test]
async fn test_full_scalpel_workflow() {
    let dir = tempdir().unwrap();
    let journal_path = dir.path().join("journal.jsonl");
    let journal = Journal::open(&journal_path).unwrap();
    
    let file_path = dir.path().join("main.rs");
    let content = "fn main() {\n    println!(\"old\");\n}\n\nfn extra() {\n    println!(\"stay\");\n}";
    
    // 1. Initial Write
    let args = WriteFileArgs {
        path: file_path.clone(),
        content: content.to_string(),
        overwrite: false,
        append: false,
    };
    let resp = Mutator::write_file(dir.path(), args, &journal);
    assert!(resp.content.is_some());
    
    // 2. Surgical Edit (AST)
    let replacement = "fn main() {\n    println!(\"surgical\");\n}";
    
    let edit_args = EditFileArgs {
        path: file_path.clone(),
        replacement: replacement.to_string(),
        query: Some(".functions[] | select(.name == \"main\")".to_string()),
        grep: None,
        from_line: None,
        to_line: None,
        preview: false,
    };

    let edit_resp = Mutator::edit_file(dir.path(), edit_args, &journal).await;
    
    assert!(edit_resp.content.is_some());
    let final_content = fs::read_to_string(&file_path).unwrap();
    
    assert!(final_content.contains("surgical"));
    assert!(final_content.contains("stay"));
    assert!(!final_content.contains("old"));
    
    // 3. Verify Journal
    let entries = journal.read_all().unwrap();
    assert_eq!(entries.len(), 2); // Write + Edit
}
