use std::path::Path;
use git2::Repository;
use serde_json::Value;
use crate::IvaldiResponse;

pub fn git_blame_sync(repo: &Repository, path: &Path, lines: Option<&[usize]>) -> IvaldiResponse<Value> {
    use crate::error::IvaldiError;
    let mut options = git2::BlameOptions::new();
    let blame = match repo.blame_file(path, Some(&mut options)) {
        Ok(b) => b,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Git(e)),
    };

    let mut results = Vec::new();
    for i in 0..blame.len() {
        if let Some(hunk) = blame.get_index(i) {
            let commit_id = hunk.final_commit_id();
            let commit = repo.find_commit(commit_id).ok();
            
            let start_line = hunk.final_start_line();
            let num_lines = hunk.lines_in_hunk();

            for line in start_line..(start_line + num_lines) {
                if let Some(targets) = lines && !targets.contains(&line) {
                    continue;
                }

                results.push(serde_json::json!({
                    "line": line,
                    "commit": commit_id.to_string(),
                    "author": commit.as_ref().and_then(|c| c.author().name().map(|n| n.to_string())),
                    "date": commit.as_ref().map(|c| c.time().seconds()), // UNIX timestamp
                    "message": commit.as_ref().and_then(|c| c.summary().map(|s| s.to_string())),
                }));
            }
        }
    }

    IvaldiResponse::success(serde_json::json!({
        "path": path,
        "blame": results
    }))
}

pub fn git_log_sync(repo: &Repository, path: Option<&Path>, limit: usize, since: Option<&str>) -> IvaldiResponse<Value> {
    use crate::error::IvaldiError;
    let mut revwalk = match repo.revwalk() {
        Ok(rw) => rw,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Git(e)),
    };

    if let Err(e) = revwalk.push_head() {
        return IvaldiResponse::from_error(IvaldiError::Git(e));
    }

    if let Some(since_ref) = since && let Ok(oid) = repo.revparse_single(since_ref).map(|obj| obj.id()) {
        // Check if we can hide this rev
        let _ = revwalk.hide(oid);
    }

    let mut commits = Vec::new();
    let mut count = 0;

    for id in revwalk {
        let id = match id {
            Ok(id) => id,
            Err(_) => continue,
        };

        let commit = match repo.find_commit(id) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Date filter
        if let Some(since_str) = since {
             let duration = match since_str {
                "1d" => chrono::Duration::days(1),
                "1w" => chrono::Duration::weeks(1),
                "1m" => chrono::Duration::days(30),
                _ => chrono::DateTime::parse_from_rfc3339(since_str).map(|dt| chrono::Utc::now().signed_duration_since(dt.with_timezone(&chrono::Utc))).unwrap_or(chrono::Duration::zero()),
             };
             let cutoff = (chrono::Utc::now() - duration).timestamp();
             if commit.time().seconds() < cutoff {
                 break; // Revwalk is reverse chronological, so we can stop
             }
        }

        // Path filter
        if let Some(p) = path {
            let mut changed = false;
            let tree = commit.tree().ok();
            let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
            
            if let Some(t) = tree {
                let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&t), None).ok();
                if let Some(d) = diff {
                    for delta in d.deltas() {
                        if delta.new_file().path() == Some(p) || delta.old_file().path() == Some(p) {
                            changed = true;
                            break;
                        }
                    }
                }
            }
            if !changed { continue; }
        }

        commits.push(serde_json::json!({
            "hash": id.to_string(),
            "author": commit.author().name().map(|n| n.to_string()),
            "date": commit.time().seconds(),
            "message": commit.summary().map(|s| s.to_string()),
        }));

        count += 1;
        if count >= limit {
            break;
        }
    }

    IvaldiResponse::success(serde_json::json!({
        "commits": commits
    }))
}

pub fn git_diff_sync(repo: &Repository, from: &str, to: &str, path: Option<&Path>, stat_only: Option<bool>) -> IvaldiResponse<Value> {
    use crate::error::IvaldiError;
    let from_obj = match repo.revparse_single(from) {
        Ok(obj) => obj,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::InvalidArgument(format!("Failed to resolve 'from' ref {}: {}", from, e))),
    };

    let to_obj = match repo.revparse_single(to) {
        Ok(obj) => obj,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::InvalidArgument(format!("Failed to resolve 'to' ref {}: {}", to, e))),
    };

    let from_tree = from_obj.peel_to_tree().ok();
    let to_tree = to_obj.peel_to_tree().ok();

    let mut opts = git2::DiffOptions::new();
    if let Some(p) = path {
        opts.pathspec(p);
    }

    let diff = match repo.diff_tree_to_tree(from_tree.as_ref(), to_tree.as_ref(), Some(&mut opts)) {
        Ok(d) => d,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Git(e)),
    };

    if stat_only.unwrap_or(false) {
        let stats = match diff.stats() {
            Ok(s) => s,
            Err(e) => return IvaldiResponse::from_error(IvaldiError::Git(e)),
        };
        
        return IvaldiResponse::success(serde_json::json!({
            "from": from,
            "to": to,
            "files_changed": stats.files_changed(),
            "insertions": stats.insertions(),
            "deletions": stats.deletions(),
        }));
    }

    let mut files = Vec::new();
    let mut current_file = None;
    let mut current_diff = String::new();

    diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
        let path = delta.new_file().path().or(delta.old_file().path());
        if let Some(p) = path {
            let p_str = p.to_string_lossy().to_string();
            if current_file.as_ref() != Some(&p_str) {
                if let Some(prev_p) = current_file.take() {
                    files.push(serde_json::json!({
                        "path": prev_p,
                        "diff": current_diff.clone()
                    }));
                }
                current_file = Some(p_str);
                current_diff.clear();
            }
        }
        
        match line.origin() {
            '+' | '-' | ' ' | 'H' | 'F' => {
                current_diff.push(line.origin());
                current_diff.push_str(&String::from_utf8_lossy(line.content()));
            }
            _ => {}
        }
        true
    }).ok();

    if let Some(prev_p) = current_file {
        files.push(serde_json::json!({
            "path": prev_p,
            "diff": current_diff
        }));
    }

    IvaldiResponse::success(serde_json::json!({
        "from": from,
        "to": to,
        "files": files
    }))
}

pub fn git_search_sync(repo: &Repository, query: &str, path: Option<&Path>, limit: usize) -> IvaldiResponse<Value> {
    let mut revwalk = match repo.revwalk() {
        Ok(rw) => rw,
        Err(e) => return IvaldiResponse::error("git_error", format!("Failed to create revwalk: {}", e)),
    };
    revwalk.push_head().ok();

    let mut results = Vec::new();
    let mut count = 0;

    let regex = match regex::Regex::new(query) {
        Ok(r) => r,
        Err(e) => return IvaldiResponse::error("invalid_arg", format!("Invalid regex: {}", e)),
    };

    for id in revwalk {
        let id = match id { Ok(id) => id, Err(_) => continue };
        let commit = match repo.find_commit(id) { Ok(c) => c, Err(_) => continue };
        
        let tree = commit.tree().ok();
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
        
        let mut options = git2::DiffOptions::new();
        if let Some(p) = path {
            options.pathspec(p);
        }

        let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), tree.as_ref(), Some(&mut options)).ok();
        let mut found = false;
        let mut context = String::new();

        if let Some(d) = diff {
            d.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
                if line.origin() == '+' || line.origin() == '-' {
                    let content = String::from_utf8_lossy(line.content());
                    if regex.is_match(&content) {
                        found = true;
                        context = content.trim().to_string();
                        return false; // Stop printing
                    }
                }
                true
            }).ok();
        }

        if found {
            results.push(serde_json::json!({
                "hash": id.to_string(),
                "date": commit.time().seconds(),
                "message": commit.summary().map(|s| s.to_string()),
                "match_context": context
            }));
            count += 1;
        }

        if count >= limit {
            break;
        }
    }

    IvaldiResponse::success(serde_json::json!({
        "commits": results
    }))
}

pub fn git_raw_sync(args: Vec<String>) -> IvaldiResponse<Value> {
    use std::process::{Command, Stdio};
    use crate::util::process::ProcessGuard;
    use crate::error::IvaldiError;
    
    let mut cmd = Command::new("git");
    cmd.args(&args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let pg = match ProcessGuard::spawn(&mut cmd) {
        Ok(p) => p,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Io(e)),
    };

    let output = match pg.wait_with_output() {
        Ok(o) => o,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Io(e)),
    };

    IvaldiResponse::success(serde_json::json!({
        "status": "completed",
        "exit_code": output.status.code(),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr)
    }))
}
