use ivaldi_core::mutate::{Mutator, WriteFileArgs};
use ivaldi_core::list::{FsLister, Lister, ListDirArgs};
use ivaldi_core::undo::{Journal, Undoer};
use ivaldi_core::lifecycle::project_root::find_project_root;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_simulation_scaffolding_workflow() {
    // 1. Start in empty directory
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let journal = Journal::open(root.join(".ivaldi/journal.jsonl")).unwrap(); // Explicit .ivaldi location
    
    // 2. Agent decides to create a Rust project
    // Create Cargo.toml
    let cargo_toml = root.join("Cargo.toml");
    let args_toml = WriteFileArgs {
        path: cargo_toml.clone(),
        content: "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n".to_string(),
        overwrite: false,
        append: false,
    };
    assert!(Mutator::write_file(root, args_toml, &journal).is_success());
    
    // Create src/main.rs (implicitly creates src dir?)
    // write_file logic: "fs::create_dir_all(parent)" -> Yes.
    let main_rs = root.join("src/main.rs");
    let args_main = WriteFileArgs {
        path: main_rs.clone(),
        content: "fn main() { println!(\"Hello\"); }".to_string(),
        overwrite: false,
        append: false,
    };
    assert!(Mutator::write_file(root, args_main, &journal).is_success());
    
    // 3. Verify Project Root Discovery
    // Logic: find_project_root(src_dir) should find root (because of Cargo.toml)
    let src_dir = root.join("src");
    let discovered_root = find_project_root(&src_dir);
    
    // Canonicalize paths for comparison to handle symlinks/dots
    let canonical_root = fs::canonicalize(root).unwrap();
    let canonical_discovered = fs::canonicalize(discovered_root).unwrap();
    
    assert_eq!(canonical_discovered, canonical_root, "Should discover project root from inside src");
    
    // 4. Verify Directory Listing
    let list_args = ListDirArgs {
        path: root.to_path_buf(),
        sort: true,
        show_hidden: false, // Default hides .ivaldi
        enable_gitignore: false,
        respect_aiignore: true,
        respect_agentignore: true,
    };
    let entries = FsLister::list_dir(list_args).content.unwrap();
    
    // Expect: Cargo.toml, src
    assert!(entries.iter().any(|e| e.name == "Cargo.toml"));
    assert!(entries.iter().any(|e| e.name == "src" && e.is_dir));
    assert!(!entries.iter().any(|e| e.name == ".ivaldi")); // Hidden
    
    // 5. Undo Scaffolding
    // Undo main.rs
    assert!(Undoer::undo_last(root, &journal).is_success());
    assert!(!main_rs.exists());
    assert!(src_dir.exists()); // Dir probably stays empty? logic doesn't rmdir parents.
    
    // Undo Cargo.toml
    assert!(Undoer::undo_last(root, &journal).is_success());
    assert!(!cargo_toml.exists());
}
