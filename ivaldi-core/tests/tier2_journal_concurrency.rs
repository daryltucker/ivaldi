use ivaldi_core::undo::{Journal, types::{JournalEntry, ActionType}};
use tempfile::TempDir;
use std::path::PathBuf;

use std::thread;
use std::sync::{Arc, Barrier};


#[test]
fn test_journal_concurrency() {
    let temp_dir = TempDir::new().unwrap();
    let journal_path = temp_dir.path().join("journal.jsonl");
    let path_clone = journal_path.clone();

    // Setup: Create journal
    let journal = Journal::open(&journal_path).unwrap();

    let thread_count = 10;
    let barrier = Arc::new(Barrier::new(thread_count));
    let mut handles = vec![];

    // Spawn 10 threads, each appending 10 entries
    for i in 0..thread_count {
        let p = path_clone.clone();
        let b = barrier.clone();
        handles.push(thread::spawn(move || {
            let j = Journal::open(&p).unwrap();
            b.wait(); // Synchonize start
            
            for k in 0..10 {
                let entry = JournalEntry::new(
                    ActionType::Create, 
                    PathBuf::from(format!("file_{}_{}.txt", i, k))
                );
                j.append(&entry).unwrap();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Verify
    let entries = journal.read_all().unwrap();
    assert_eq!(entries.len(), 100, "Should have 100 entries (10 threads * 10 writes)");
    
    // Integrity check (parse check is implicit in read_all unwrapping)
}
