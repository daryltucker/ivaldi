use ivaldi_core::observe::{FsObserver, Observer, ReadFileArgs};
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_read_with_query_ast() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    std::fs::create_dir(root.join("src")).unwrap();
    let file_path = root.join("src/lib.rs");
    
    // Create a Rust file with known structure
    let mut f = File::create(&file_path).unwrap();
    f.write_all(r#"
fn main() {
    println!("Hello");
}

fn helper() {
    let x = 1;
}
"#.as_bytes()).unwrap();

    // Query for all functions
    let args = ReadFileArgs {
        path: file_path.clone(),
        query: Some(".functions[]".to_string()),
        ..Default::default()
    };

    let result = FsObserver::read_file(args);
    let content = result.content.unwrap().content;

    assert!(content.contains("fn main"));
    assert!(content.contains("fn helper"));
}

#[test]
fn test_read_with_grep_no_context() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let file_path = root.join("notes.txt");
    
    let mut f = File::create(&file_path).unwrap();
    f.write_all(r#"Line 1
TODO: Fix bug
Line 3
FIXME: Another bug
Line 5
"#.as_bytes()).unwrap();

    // Grep for TODO/FIXME with ZERO context to test pure filtering
    let args = ReadFileArgs {
        path: file_path,
        grep: Some("^(TODO|FIXME)".to_string()),
        context_lines: Some(0), // Explicitly zero
        ..Default::default()
    };

    let result = FsObserver::read_file(args);
    let content = result.content.unwrap().content;

    assert!(content.contains("TODO: Fix bug"));
    assert!(content.contains("FIXME: Another bug"));
    assert!(!content.contains("Line 1")); // Should NOT be present with 0 context
}

#[test]
fn test_read_with_grep_and_context() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let file_path = root.join("context.txt");
    
    let mut f = File::create(&file_path).unwrap();
    f.write_all(r#"ALPHA
BETA
TARGET
GAMMA
DELTA
"#.as_bytes()).unwrap();

    // Grep for TARGET with 1 line context
    let args = ReadFileArgs {
        path: file_path,
        grep: Some("TARGET".to_string()),
        context_lines: Some(1),
        ..Default::default()
    };

    let result = FsObserver::read_file(args);
    let content = result.content.unwrap().content;
    
    if content.contains("ALPHA") {
        println!("DEBUG CONTENT:\n{}", content);
    }

    assert!(content.contains("TARGET"));
    assert!(content.contains("BETA")); // Context before
    assert!(content.contains("GAMMA")); // Context after
    assert!(!content.contains("ALPHA")); // Too far
    assert!(!content.contains("DELTA")); // Too far
}
