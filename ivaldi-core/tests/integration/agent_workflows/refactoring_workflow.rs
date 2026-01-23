use ivaldi_core::mutate::{Mutator, WriteFileArgs, EditFileArgs, RenameSymbolArgs};

use ivaldi_core::undo::{Journal, Undoer};
use tempfile::TempDir;
use std::fs;
use anyhow::Result;

#[tokio::test]
async fn test_simulation_refactoring_workflow() {
    // 1. Setup Mock Project
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join("journal.jsonl")).unwrap();
    
    // Create 3 interconnected files
    let file_a = root.join("mod_a.rs");
    let file_b = root.join("mod_b.rs");
    let file_main = root.join("main.rs");
    
    // Initial content
    let content_a = "pub struct OldName { id: u32 }";
    let content_b = "use crate::mod_a::OldName;\nfn process(o: OldName) {}";
    let content_main = "mod mod_a;\nmod mod_b;\nuse mod_a::OldName;\nfn main() { let _ = OldName { id: 1 }; }";
    
    // Helper to write
    let write = |path: &std::path::PathBuf, content: &str| {
        let args = WriteFileArgs {
            path: path.clone(),
            content: content.to_string(),
            overwrite: true, // Force create/overwrite
            append: false,
        };
        let res = Mutator::write_file(root, args, &journal);
        assert!(res.is_success(), "Failed to setup file {:?}", path);
    };
    
    write(&file_a, content_a);
    write(&file_b, content_b);
    write(&file_main, content_main);
    
    // 2. The Refactoring (OldName -> NewName)
    // Agent reads files (skipped here, assume agent has context)
    
    // Edit A
    let edit_a = EditFileArgs {
        path: file_a.clone(),
        replacement: "pub struct NewName { id: u32 }".to_string(),
        query: None,
        grep: Some("pub struct OldName".to_string()),
        from_line: None, to_line: None, overwrite: true,
    };
    assert!(Mutator::edit_file(root, edit_a, &journal).await.is_success());
    
    // Edit B
    let _edit_b = EditFileArgs {
        path: file_b.clone(),
        replacement: "use crate::mod_a::NewName;\nfn process(o: NewName) {}".to_string(),
        // Simulating a full file rewrite for simplicity in this step, or surgical edits?
        // Let's do surgical to be cool.
        // Actually, grep replace of "OldName" might be dangerous globally, but fine here.
        query: None, 
        grep: Some("OldName".to_string()), // Matches line 1
        from_line: None, to_line: None, overwrite: true,
    };
    // Wait, grep only replaces ONE line. file_b has 2 occurrences? 
    // "use ... OldName" and "fn ... OldName".
    // ivaldi edit matches *one* line. Multiline grep not supported yet?
    // Let's use string replacement on full content for B to simulate "smart agent" doing whole file.
    let content_b_new = content_b.replace("OldName", "NewName");
    let write_b = WriteFileArgs {
        path: file_b.clone(),
        content: content_b_new,
        overwrite: true,
        append: false,
    };
    assert!(Mutator::write_file(root, write_b, &journal).is_success());
    
    // Edit Main
    let content_main_new = content_main.replace("OldName", "NewName");
    let write_main = WriteFileArgs {
        path: file_main.clone(),
        content: content_main_new,
        overwrite: true,
        append: false,
    };
    assert!(Mutator::write_file(root, write_main, &journal).is_success());
    
    // 3. Verify Consistency
    assert!(fs::read_to_string(&file_a).unwrap().contains("NewName"));
    assert!(fs::read_to_string(&file_b).unwrap().contains("NewName"));
    assert!(fs::read_to_string(&file_main).unwrap().contains("NewName"));
    
    assert!(!fs::read_to_string(&file_a).unwrap().contains("OldName"));
    
    // 4. UNDO STACK (The real test)
    // We did: Write A, Write B, Write Main (after setup). 3 ops.
    // Undo 3 times.
    
    assert!(Undoer::undo_last(root, &journal).is_success()); // Undo Main
    assert!(Undoer::undo_last(root, &journal).is_success()); // Undo B
    assert!(Undoer::undo_last(root, &journal).is_success()); // Undo A (Edit)
    
    // Verify Restoration
    assert_eq!(fs::read_to_string(&file_a).unwrap(), content_a);
    assert_eq!(fs::read_to_string(&file_b).unwrap(), content_b);
    assert_eq!(fs::read_to_string(&file_main).unwrap(), content_main);
}

#[tokio::test]
async fn test_rename_symbol_basic() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // Create a test file with a function to rename
    let test_file = root.join("test.rs");
    let content = r#"
fn old_function_name() {
    println!("Hello world");
}

fn main() {
    old_function_name();
}
"#;
    fs::write(&test_file, content)?;

    // Test renaming the function
    let args = RenameSymbolArgs {
        path: test_file.to_string_lossy().to_string(),
        old_name: "old_function_name".to_string(),
        new_name: "new_function_name".to_string(),
        symbol_type: Some("function".to_string()),
        scope: Some("file".to_string()),
    };

    let journal = Journal::open(root.join("journal.jsonl"))?;
    let response = Mutator::rename_symbol(root, args, &journal).await;

    // Verify the response is successful
    assert!(response.is_success());
    let result = response.content.unwrap();

    // Verify the result
    assert_eq!(result.files_modified, 1);
    assert_eq!(result.symbols_renamed, 1);

    // Verify the file content was updated
    let updated_content = fs::read_to_string(&test_file)?;
    assert!(updated_content.contains("new_function_name"));
    assert!(!updated_content.contains("old_function_name"));

    Ok(())
}
