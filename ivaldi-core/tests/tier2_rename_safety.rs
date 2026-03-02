use ivaldi_core::mutate::{Mutator, RenameSymbolArgs};
use ivaldi_core::undo::Journal;
use tempfile::TempDir;
use std::fs;

#[tokio::test]
async fn test_rename_function_preserves_unrelated_code() {
    // 1. Setup
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join(".ivaldi/journal.jsonl")).unwrap();
    
    let path = root.join("test.rs");
    let content = r#"
fn unrelated_func() {
    // This "old_name" string should NOT be changed
    println!("unrelated old_name");
}

fn old_name() {
    println!("this is the target");
}

fn another_one() {
    let x = "old_name";
}
"#;
    fs::write(&path, content).unwrap();
    
    // 2. Rename specifically the function "old_name"
    let args = RenameSymbolArgs {
        path: "test.rs".to_string(),
        old_name: "old_name".to_string(),
        new_name: "new_name".to_string(),
        symbol_type: Some("function".to_string()),
        scope: None,
    };
    
    let result = Mutator::rename_symbol(root, args, &journal).await;
    
    // 3. Verify
    assert!(result.is_success(), "Rename operation failed: {:?}", result.error);
    
    let new_content = fs::read_to_string(&path).unwrap();
    
    // TARGET: Function definition should be renamed
    assert!(new_content.contains("fn new_name()"), "Function definition was not renamed");
    
    // SAFETY: Unrelated occurrences outside the node should be preserved
    assert!(new_content.contains("println!(\"unrelated old_name\")"), "Unrelated string literal was accidentally renamed!");
    assert!(new_content.contains("let x = \"old_name\""), "Unrelated variable assignment was accidentally renamed!");
    
    println!("Test passed: Surgical renaming isolated to target node.");
}

#[tokio::test]
async fn test_rename_symbol_not_found_returns_error() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join(".ivaldi/journal.jsonl")).unwrap();
    
    let path = root.join("test.rs");
    fs::write(&path, "fn existing() {}").unwrap();
    
    let args = RenameSymbolArgs {
        path: "test.rs".to_string(),
        old_name: "missing".to_string(),
        new_name: "new_name".to_string(),
        symbol_type: Some("function".to_string()),
        scope: None,
    };
    
    let result = Mutator::rename_symbol(root, args, &journal).await;
    assert!(result.is_error);
    assert!(result.error.unwrap().message.contains("not found"));
}

#[tokio::test]
async fn test_rename_symbol_fallback_to_hammer() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join(".ivaldi/journal.jsonl")).unwrap();
    
    let path = root.join("test.txt");
    let content = "The quick brown fox jumps over the old_name.";
    fs::write(&path, content).unwrap();
    
    let args = RenameSymbolArgs {
        path: "test.txt".to_string(),
        old_name: "old_name".to_string(),
        new_name: "new_name".to_string(),
        symbol_type: None,
        scope: None,
    };
    
    let result = Mutator::rename_symbol(root, args, &journal).await;
    
    assert!(result.is_success());
    let new_content = fs::read_to_string(&path).unwrap();
    assert!(new_content.contains("new_name"));
    
    // Verify Hammer fallback advisory
    let has_fallback_warn = result.advisory.iter().any(|a| {
        a.content.get("issue").map(|v| v == "hammer_fallback").unwrap_or(false)
    });
    assert!(has_fallback_warn, "Expected warning about hammer fallback. Got: {:?}", result.advisory);
}
