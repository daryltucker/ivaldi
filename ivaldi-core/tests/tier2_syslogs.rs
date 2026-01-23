use ivaldi_core::observe::syslogs::{read_syslogs, ReadSyslogsArgs, LogLevel};

#[tokio::test]
async fn test_read_syslogs_smoke() {
    let args = ReadSyslogsArgs {
        service: None,
        level: None,
        since: Some("1m".to_string()),
        pattern: None,
        limit: 5,
        boot: None,
    };

    let response = read_syslogs(args).await;
    
    // We don't fail the test if the journal isn't accessible (e.g. in some CI)
    // but we verify it doesn't panic and returns a structured response.
    if let Some(err) = response.error {
        assert!(err.code == "syslog_error" || err.code == "io_error" || err.code == "permission_denied");
    } else {
        let result = response.content.unwrap();
        let logs = result.get("logs").unwrap().as_array().unwrap();
        assert!(logs.len() <= 5);
        if !logs.is_empty() {
            let first = &logs[0];
            assert!(first.get("timestamp").is_some());
            assert!(first.get("level").is_some());
            assert!(first.get("message").is_some());
        }
    }
}

#[tokio::test]
async fn test_read_syslogs_invalid_regex() {
    let args = ReadSyslogsArgs {
        service: None,
        level: None,
        since: None,
        pattern: Some("[[[[".to_string()), // Invalid regex
        limit: 10,
        boot: None,
    };

    let response = read_syslogs(args).await;
    assert!(response.error.is_some());
    assert_eq!(response.error.unwrap().code, "invalid_arg");
}

#[tokio::test]
async fn test_log_level_conversions() {
    // This is a unit test of the logic inside syslogs.rs (if it were public)
    // Since LogLevel is public, we can test it if it had methods. 
    // It doesn't have public methods for conversion yet, but we can verify it exists.
    let level = LogLevel::Info;
    assert_eq!(format!("{:?}", level), "Info");
}
