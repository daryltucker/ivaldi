use ivaldi_core::observe::search::{search_code, SearchCodeArgs};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_search_code_rust_functions() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("main.rs");
    fs::write(&file_path, r#"
fn hello() { println!("hello"); }
fn world() { println!("world"); }
"#).unwrap();

    let args = SearchCodeArgs {
        path: dir.path().to_path_buf(),
        query: Some(".[] | .functions[] | .name".to_string()),
        category: None,
        name_pattern: None,
        language: None,
        depth: 1,
        pattern: None,
        respect_agentignore: true,
        limit: 0,
        offset: 0,
    };

    let response = search_code(args).await;
    assert!(response.error.is_none());
    
    let result = response.content.unwrap();
    let names: Vec<String> = serde_json::from_value(result).unwrap();
    assert!(names.contains(&"hello".to_string()));
    assert!(names.contains(&"world".to_string()));
}

#[tokio::test]
async fn test_search_code_friendly_category() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("main.py");
    fs::write(&file_path, r#"
def foo():
    pass

class Bar:
    def method(self):
        pass
"#).unwrap();

    // Search for functions
    let args = SearchCodeArgs {
        path: dir.path().to_path_buf(),
        query: None, // Use friendly mode
        category: Some("function".to_string()),
        name_pattern: None,
        language: None,
        depth: 1,
        pattern: None,
        respect_agentignore: true,
        limit: 0,
        offset: 0,
    };

    let response = search_code(args).await;
    assert!(response.error.is_none());
    
    let result = response.content.unwrap();
    let matches = result.as_array().unwrap();
    // vecq friendly output is usually the full nodes
    let names: Vec<&str> = matches.iter()
        .filter_map(|m| m.get("name").and_then(|v| v.as_str()))
        .collect();
    
    assert!(names.contains(&"foo"));
    // Depending on vecq, it might or might not include methods in "function" category
    // But it should definitely find "foo"
}

#[tokio::test]
async fn test_search_code_respects_gitignore() {
    let dir = tempdir().unwrap();
    // Initialize dummy git repo structure
    fs::create_dir(dir.path().join(".git")).unwrap();
    fs::write(dir.path().join(".gitignore"), "ignored.rs").unwrap();
    
    fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
    fs::write(dir.path().join("ignored.rs"), "fn secret() {}").unwrap();

    let args = SearchCodeArgs {
        path: dir.path().to_path_buf(),
        query: Some(".[] | .functions[] | .name".to_string()),
        category: None,
        name_pattern: None,
        language: None,
        depth: 2,
        pattern: None,
        respect_agentignore: true,
        limit: 0,
        offset: 0,
    };

    let response = search_code(args).await;
    let result = response.content.unwrap();
    let names: Vec<String> = serde_json::from_value(result).unwrap();
    
    assert!(names.contains(&"main".to_string()));
    assert!(!names.contains(&"secret".to_string()));
}
