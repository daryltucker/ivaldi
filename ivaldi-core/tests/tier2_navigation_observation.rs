use ivaldi_core::navigate::{FsNavigator, Navigator, FindFilesArgs};
use ivaldi_core::observe::{FsObserver, Observer, ReadFileArgs};
use ivaldi_core::list::{FsLister, Lister, ListDirArgs};
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_find_files_respects_depth_and_pattern() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create structure:
    // root/
    //   file1.txt
    //   subdir/
    //     file2.rs
    //     nested/
    //       file3.txt
    
    std::fs::create_dir(root.join("subdir")).unwrap();
    std::fs::create_dir(root.join("subdir/nested")).unwrap();
    File::create(root.join("file1.txt")).unwrap();
    File::create(root.join("subdir/file2.rs")).unwrap();
    File::create(root.join("subdir/nested/file3.txt")).unwrap();

    // Test 1: Depth 1 (should find only file1.txt and subdir)
    // Test 1: Depth 1 (should find only file1.txt and subdir)
    let args = FindFilesArgs {
        path: root.to_path_buf(),
        pattern: "".to_string(),
        max_depth: 1,
        max_entries: 10,
        timeout_ms: 1000,
        enable_gitignore: false,
        respect_aiignore: false,
        respect_agentignore: false,
    };
    
    let result = FsNavigator::find_files(args);
    assert!(!result.is_error);
    let files = result.content.unwrap();
    // walkdir returns root? No, we filtered it in impl.
    // depth 1 from root usually includes direct children.
    // file1.txt, subdir.
    assert!(files.iter().any(|f| f.path.ends_with("file1.txt")));
    assert!(files.iter().any(|f| f.path.ends_with("subdir"))); // walkdir includes dirs
    assert!(!files.iter().any(|f| f.path.ends_with("file2.rs"))); // depth 2

    // Test 2: Pattern "*.rs" (recursive)
    // Test 2: Pattern "*.rs" (recursive)
    let args_glob = FindFilesArgs { 
        path: root.to_path_buf(),
        pattern: "*.rs".to_string(),
        max_depth: 5, 
        max_entries: 100,
        timeout_ms: 2000,
        enable_gitignore: false,
        respect_aiignore: true,
        respect_agentignore: true,
    };
    let result = FsNavigator::find_files(args_glob);
    let files = result.content.unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].path.ends_with("file2.rs"));
}

#[test]
fn test_read_file_safety() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    
    // 1. Binary File
    let bin_path = root.join("binary.bin");
    let mut f = File::create(&bin_path).unwrap();
    f.write_all(&[0, 1, 2, 0, 0, 255]).unwrap(); // Null bytes
    
    let args = ReadFileArgs {
        path: bin_path.clone(),
        from_line: None,
        to_line: None,
        force: false,
        ..Default::default()
    };
    let result = FsObserver::read_file(args);
    assert!(result.is_error);
    assert_eq!(result.error.unwrap().code, "binary_detected");

    // 2. Large File Truncation
    let large_path = root.join("large.txt");
    let mut f = File::create(&large_path).unwrap();
    for i in 0..1500 {
        writeln!(f, "Line {}", i).unwrap();
    }

    let args = ReadFileArgs {
        path: large_path.clone(),
        from_line: None,
        to_line: None,
        force: false,
        ..Default::default()
    };
    let result = FsObserver::read_file(args);
    assert!(!result.is_error);
    let content = result.content.unwrap();
    assert!(content.info.truncated);
    assert!(content.content.contains("... [ TRUNCATED ] ..."));
    assert!(content.content.contains("Line 0"));
    assert!(content.content.contains("Line 1499"));
    // Default trunc is 500 head + 500 tail = 1000 lines (+ marker)
    // 1500 > 1000, so it triggered.
}

#[test]
fn test_list_dir_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    
    File::create(root.join("a_file.txt")).unwrap();
    std::fs::create_dir(root.join("b_dir")).unwrap();
    
    let args = ListDirArgs {
        path: root.to_path_buf(),
        sort: true,
        show_hidden: true,
        enable_gitignore: false,
        respect_aiignore: true,
        respect_agentignore: true,
    };
    let result = FsLister::list_dir(args);
    assert!(!result.is_error);
    let entries = result.content.unwrap();
    
    assert_eq!(entries.len(), 2);
    // Sort default true
    assert_eq!(entries[0].name, "a_file.txt");
    assert!(!entries[0].is_dir);
    
    assert_eq!(entries[1].name, "b_dir");
    assert!(entries[1].is_dir);
}
