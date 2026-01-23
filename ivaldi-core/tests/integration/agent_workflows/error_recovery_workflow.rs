use ivaldi_core::observe::{FsObserver, Observer, ReadFileArgs};
use tempfile::TempDir;
use std::fs::{self, File};
use std::io::Write;

#[test]
fn test_simulation_error_recovery() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    
    // Scenario 1: The Binary Wall
    // Agent encounters a binary file and is stopped, then overrides.
    let bin_path = root.join("firmware.bin");
    let mut f = File::create(&bin_path).unwrap();
    f.write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]).unwrap(); // Null byte!
    
    // Attempt 1: Naive Read
    let args_naive = ReadFileArgs {
        path: bin_path.clone(),
        from_line: None, to_line: None, force: false,
        ..Default::default()
    };
    let res_naive = FsObserver::read_file(args_naive);
    
    assert!(res_naive.is_error);
    assert!(res_naive.error.as_ref().unwrap().code.contains("binary"));
    
    // Attempt 2: Force Read (Agent decides they know better)
    let args_force = ReadFileArgs {
        path: bin_path.clone(),
        from_line: None, to_line: None, force: true,
        ..Default::default()
    };
    let res_force = FsObserver::read_file(args_force);
    
    assert!(!res_force.is_error);
    // Content is lossy string, but success.
    
    
    // Scenario 2: The Typo Stumble
    // Agent tries to read a file that doesn't exist, but a sibling does.
    let config_path = root.join("config.toml");
    fs::write(&config_path, "[main]\nkey=value").unwrap();
    
    // Attempt 1: Typo "configs.toml"
    let args_typo = ReadFileArgs {
        path: root.join("configs.toml"),
        from_line: None, to_line: None, force: false,
        ..Default::default()
    };
    let res_typo = FsObserver::read_file(args_typo);
    
    // Expect Error but with Advisory Hint in ACTION
    assert!(res_typo.is_error);
    assert!(res_typo.advisory.iter().any(|a| 
        a.action.as_ref().map(|s| s.contains("Did you mean")).unwrap_or(false)
    ));
    // AdvisoryMessage content is Varies.
    
    // Attempt 2: Correct Path (Simulating Agent using the hint)
    // We assume the agent parsed the advisory.
    let args_correct = ReadFileArgs {
        path: config_path.clone(),
        from_line: None, to_line: None, force: false,
        ..Default::default()
    };
    let res_correct = FsObserver::read_file(args_correct);
    assert!(!res_correct.is_error);
    assert!(res_correct.content.unwrap().content.contains("key=value"));
}
