use ivaldi_core::session::{SessionManager};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn setup_env() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path().join("config");
    fs::create_dir_all(&config_dir).unwrap();
    
    // No more env var setting!
    
    (temp_dir, config_dir.join("sessions.toml"))
}

#[test]
fn test_session_create_saves_to_disk() {
    let (_temp, session_file) = setup_env();
    
    let mut manager = SessionManager::new_with_path(session_file.clone()).unwrap();
    let root = session_file.parent().unwrap().join("my-project");
    fs::create_dir_all(&root).unwrap();
    
    // Create session
    let session = manager.load_or_create("test-project", Some(root.clone())).unwrap();
    
    assert_eq!(session.id, "test-project");
    assert_eq!(session.root, root);
    
    // Check file existence
    assert!(session_file.exists(), "sessions.toml should be created");
    
    let content = fs::read_to_string(session_file).unwrap();
    assert!(content.contains("test-project"));
}

#[test]
fn test_session_load_existing() {
    let (_temp, session_file) = setup_env();
    
    // 1. Create and save
    let mut manager = SessionManager::new_with_path(session_file.clone()).unwrap();
    let _ = manager.load_or_create("persistent", None).unwrap();
    
    // 2. New manager instance (simulate restart) pointing to SAME file
    let manager2 = SessionManager::new_with_path(session_file).unwrap();
    
    // 3. Should already exist in store
    let sessions = manager2.list();
    assert!(sessions.iter().any(|s| s.id == "persistent"));
}

#[test]
fn test_resolve_path_hierarchy() {
    let (_temp, session_file) = setup_env();
    let root = session_file.parent().unwrap().to_path_buf();
    
    let mut manager = SessionManager::new_with_path(session_file).unwrap();
    
    // Create dummy project structure
    // root/
    //   .git/ (dir)
    //   src/
    //     main.rs
    //   README.md
    
    let project_root = root.join("my-rust-app");
    fs::create_dir_all(project_root.join(".git")).unwrap();
    fs::create_dir_all(project_root.join("src")).unwrap();
    fs::write(project_root.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(project_root.join("README.md"), "# Hello").unwrap();
    
    // Session starts in src/
    let session_root = project_root.join("src");
    let session = manager.load_or_create("rust-app", Some(session_root.clone())).unwrap();
    
    // 1. Resolve relative path inside session root
    let p1 = manager.resolve_path(&session, Path::new("main.rs"));
    assert_eq!(p1, session_root.join("main.rs"));
    
    // 2. Resolve relative path in PROJECT root (parent of session root via .git discovery)
    // Note: Our manager implementation tries project_root first if file exists there
    let p2 = manager.resolve_path(&session, Path::new("README.md"));
    assert_eq!(p2, project_root.join("README.md"));
    
    // 3. Resolve absolute path
    let abs = PathBuf::from("/etc/hosts");
    let p3 = manager.resolve_path(&session, &abs);
    assert_eq!(p3, abs);
    
    // 4. Resolve missing file (defaults to session root join)
    let p4 = manager.resolve_path(&session, Path::new("ghost.txt"));
    assert_eq!(p4, session_root.join("ghost.txt"));
}

#[test]
fn test_project_root_discovery() {
    let (_temp, session_file) = setup_env();
    let root = session_file.parent().unwrap().to_path_buf();
    
    let mut manager = SessionManager::new_with_path(session_file).unwrap();
    
    let deep_path = root.join("a/b/c/d");
    fs::create_dir_all(&deep_path).unwrap();
    
    // Plant a marker at 'a'
    fs::create_dir(root.join("a/.ivaldi")).unwrap();
    
    let session = manager.load_or_create("discovery-test", Some(deep_path)).unwrap();
    
    assert!(session.project_root.is_some());
    assert_eq!(session.project_root.unwrap(), root.join("a"));
}

#[test]
#[ignore]
fn test_smart_label_generation() {
    let (_temp, session_file) = setup_env();
    let root = session_file.parent().unwrap().to_path_buf();
    
    let mut manager = SessionManager::new_with_path(session_file).unwrap();
    
    // Create a directory "cool-app"
    let app_dir = root.join("cool-app");
    fs::create_dir_all(&app_dir).unwrap();
    
    let session = manager.load_or_create("generated-label-test", Some(app_dir)).unwrap();
    
    // Expect label to be "cool-app"
    assert_eq!(session.metadata.label, Some("cool-app".to_string()));
    
    // Test with project root discovery
    // root/parent-proj.git  <- Needs .git inside
    let parent = root.join("parent-project");
    fs::create_dir_all(parent.join(".git")).unwrap();
    let sub = parent.join("sub/dir");
    fs::create_dir_all(&sub).unwrap();
    
    let session2 = manager.load_or_create("deep-nested", Some(sub)).unwrap();
    
    // Should discover "parent-project" as project root and use that for label
    assert_eq!(session2.metadata.label, Some("parent-project".to_string()));
}
