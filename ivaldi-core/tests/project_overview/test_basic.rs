use ivaldi_core::observe::{Analyzer, ProjectOverviewArgs};
use ivaldi_core::IvaldiResponse;
use std::fs;
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_project_overview_basic() {
    // Create a temporary directory with test files
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let dir_path = temp_dir.path();
    
    // Create a simple Rust file
    let rust_file_path = dir_path.join("lib.rs");
    let mut rust_file = fs::File::create(&rust_file_path).expect("Failed to create file");
    writeln!(rust_file, "pub fn main() {{}}").expect("Failed to write to file");
    writeln!(rust_file, "pub struct MyStruct {{}}").expect("Failed to write to file");
    
    // Create a simple Python file
    let py_file_path = dir_path.join("module.py");
    let mut py_file = fs::File::create(&py_file_path).expect("Failed to create file");
    writeln!(py_file, "def hello():").expect("Failed to write to file");
    writeln!(py_file, "    pass").expect("Failed to write to file");
    
    // Create args for project_overview
    let args = ProjectOverviewArgs {
        path: dir_path.to_path_buf(),
        max_depth: 5,
        ignore_patterns: Vec::new(),
    };
    
    // Call project_overview
    let response: IvaldiResponse<super::ProjectOverview> = Analyzer::project_overview(args);
    
    // Assertions
    assert!(!response.is_error, "Response should not be an error: {:?}", response.error);
    let overview = response.content.expect("Response should contain ProjectOverview");
    
    // Check project root
    assert_eq!(overview.project_root, dir_path);
    
    // Should have processed both files
    assert!(overview.files.len() >= 2, "Should have processed at least 2 files");
    
    // Check for Rust file info
    let rust_file_info = overview.files.iter().find(|f| f.file_name() == "lib.rs");
    assert!(rust_file_info.is_some(), "Should have processed lib.rs");
    let rust_info = rust_file_info.unwrap();
    assert_eq!(rust_info.file_type, "FileType::Rust");
    assert!(rust_info.symbols.iter().any(|s| s.contains("pub fn main")));
    assert!(rust_info.symbols.iter().any(|s| s.contains("pub struct MyStruct")));
    
    // Check for Python file info
    let py_file_info = overview.files.iter().find(|f| f.file_name() == "module.py");
    assert!(py_file_info.is_some(), "Should have processed module.py");
    let py_info = py_file_info.unwrap();
    assert_eq!(py_info.file_type, "FileType::Python");
    assert!(py_info.symbols.iter().any(|s| s.contains("def hello")));
    
    // Check dependencies (imports)
    // In this simple test, we might not have imports, but the structure should be correct
    
    // Check that mermaid diagram was generated
    assert!(!overview.mermaid.is_empty(), "Mermaid diagram should not be empty");
    assert!(overview.mermaid.contains("graph TD"), "Should be a valid Mermaid diagram");
}

#[test]
fn test_project_overview_nonexistent_dir() {
    let args = ProjectOverviewArgs {
        path: std::path::PathBuf::from("/nonexistent/path"),
        max_depth: 5,
        ignore_patterns: Vec::new(),
    };
    
    let response: IvaldiResponse<super::ProjectOverview> = Analyzer::project_overview(args);
    assert!(response.is_error, "Should return error for nonexistent directory");
}

#[test]
fn test_project_overview_file_instead_of_dir() {
    // Create a temporary file
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test.txt");
    let mut file = fs::File::create(&file_path).expect("Failed to create file");
    writeln!(file, "test content").expect("Failed to write to file");
    
    let args = ProjectOverviewArgs {
        path: file_path,
        max_depth: 5,
        ignore_patterns: Vec::new(),
    };
    
    let response: IvaldiResponse<super::ProjectOverview> = Analyzer::project_overview(args);
    assert!(response.is_error, "Should return error when path is a file, not directory");
}
