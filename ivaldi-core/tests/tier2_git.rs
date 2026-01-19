use ivaldi_core::observe::git::{git_read, GitReadArgs, GitAction};
use git2::{Repository, Signature};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn setup_repo(path: &std::path::Path) -> Repository {
    let repo = Repository::init(path).unwrap();
    let mut index = repo.index().unwrap();
    let sig = Signature::now("Test User", "test@example.com").unwrap();
    
    // First commit
    let file1 = path.join("file1.txt");
    fs::write(&file1, "line 1\nline 2\n").unwrap();
    index.add_path(std::path::Path::new("file1.txt")).unwrap();
    index.write().unwrap();
    {
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[]).unwrap();
    }
    
    // Second commit
    fs::write(&file1, "line 1\nline 2\nline 3\n").unwrap();
    index.add_path(std::path::Path::new("file1.txt")).unwrap();
    index.write().unwrap();
    {
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Second commit", &tree, &[&parent]).unwrap();
    }
    
    repo
}

#[tokio::test]
async fn test_git_blame() {
    let dir = tempdir().unwrap();
    let _repo = setup_repo(dir.path());
    
    let args = GitReadArgs {
        action: GitAction::Blame {
            path: PathBuf::from("file1.txt"),
            lines: None,
        },
        project_root: Some(dir.path().to_path_buf()),
    };

    let response = git_read(args, None).await;
    assert!(response.error.is_none());
    
    let result = response.result.unwrap();
    let blame = result.get("blame").unwrap().as_array().unwrap();
    assert_eq!(blame.len(), 3);
    
    // Check third line is from Second commit
    let last_line = blame.get(2).unwrap();
    assert_eq!(last_line.get("line").unwrap(), 3);
    assert_eq!(last_line.get("message").unwrap(), "Second commit");
}

#[tokio::test]
async fn test_git_log() {
    let dir = tempdir().unwrap();
    let _repo = setup_repo(dir.path());
    
    let args = GitReadArgs {
        action: GitAction::Log {
            path: None,
            limit: 10,
            since: None,
        },
        project_root: Some(dir.path().to_path_buf()),
    };

    let response = git_read(args, None).await;
    assert!(response.error.is_none());
    
    let result = response.result.unwrap();
    let commits = result.get("commits").unwrap().as_array().unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].get("message").unwrap(), "Second commit");
    assert_eq!(commits[1].get("message").unwrap(), "Initial commit");
}

#[tokio::test]
async fn test_git_diff() {
    let dir = tempdir().unwrap();
    let _repo = setup_repo(dir.path());
    
    let args = GitReadArgs {
        action: GitAction::Diff {
            from: "HEAD~1".to_string(),
            to: "HEAD".to_string(),
            path: None,
            stat_only: false,
        },
        project_root: Some(dir.path().to_path_buf()),
    };

    let response = git_read(args, None).await;
    assert!(response.error.is_none());
    
    let result = response.result.unwrap();
    let files = result.get("files").unwrap().as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].get("diff").unwrap().as_str().unwrap().contains("+line 3"));
}

#[tokio::test]
async fn test_git_search() {
    let dir = tempdir().unwrap();
    let _repo = setup_repo(dir.path());
    
    let args = GitReadArgs {
        action: GitAction::Search {
            query: "line 3".to_string(),
            path: None,
            limit: 10,
        },
        project_root: Some(dir.path().to_path_buf()),
    };

    let response = git_read(args, None).await;
    assert!(response.error.is_none());
    
    let result = response.result.unwrap();
    let commits = result.get("commits").unwrap().as_array().unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].get("message").unwrap(), "Second commit");
}
