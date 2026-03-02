use serde::{Deserialize, Serialize};
use crate::IvaldiResponse;
use schemars::JsonSchema;
use std::collections::HashMap;

/// Log levels for filtering
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
    Alert,
    Emergency,
}

impl LogLevel {
    #[cfg(target_os = "linux")]
    fn to_priority(self) -> i32 {
        match self {
            LogLevel::Emergency => 0,
            LogLevel::Alert => 1,
            LogLevel::Critical => 2,
            LogLevel::Error => 3,
            LogLevel::Warning => 4,
            LogLevel::Notice => 5,
            LogLevel::Info => 6,
            LogLevel::Debug => 7,
        }
    }

    #[cfg(target_os = "linux")]
    fn from_priority(p: i32) -> Self {
        match p {
            0 => LogLevel::Emergency,
            1 => LogLevel::Alert,
            2 => LogLevel::Critical,
            3 => LogLevel::Error,
            4 => LogLevel::Warning,
            5 => LogLevel::Notice,
            6 => LogLevel::Info,
            _ => LogLevel::Debug,
        }
    }
}

/// Arguments for the read_syslogs tool
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub struct ReadSyslogsArgs {
    /// systemd unit name (e.g. "ivaldi-server")
    pub service: Option<String>,
    
    /// Minimum log level to include
    pub level: Option<LogLevel>,
    
    /// Time window to query (e.g. "10m", "1h", "1d" or ISO timestamp)
    pub since: Option<String>,
    
    /// Regex pattern to search for in log messages
    pub pattern: Option<String>,
    
    /// Maximum number of entries to return (default: 100)
    #[serde(default = "default_limit")]
    pub limit: usize,
    
    /// Boot to query (e.g. "current", "previous")
    pub boot: Option<String>,
}

fn default_limit() -> usize { 100 }

/// Structured log entry
#[derive(Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
    pub service: Option<String>,
    pub pid: Option<u32>,
    pub fields: HashMap<String, String>,
}

/// Perform syslog read operation (Journald)
pub async fn read_syslogs(args: ReadSyslogsArgs) -> IvaldiResponse<serde_json::Value> {
    #[cfg(target_os = "linux")]
    {
        // Offload to blocking thread
        let res = tokio::task::spawn_blocking(move || {
            read_syslogs_sync(args)
        }).await;

        match res {
            Ok(response) => response,
            Err(e) => IvaldiResponse::from_error(crate::error::IvaldiError::Internal(format!("Blocking task failed: {}", e))),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = args;
        IvaldiResponse::from_error(crate::error::IvaldiError::Internal("Systemd logs are only available on Linux".to_string()))
    }
}

#[cfg(target_os = "linux")]
fn read_syslogs_sync(args: ReadSyslogsArgs) -> IvaldiResponse<serde_json::Value> {
    use crate::error::IvaldiError;
    use systemd::journal;

    let mut journal: journal::Journal = match journal::OpenOptions::default()
        .system(true)
        .open() 
    {
        Ok(j) => j,
        Err(e) => return IvaldiResponse::from_error(IvaldiError::Systemd(e.to_string())),
    };

    // Since filter
    if let Some(since_str) = &args.since {
        let duration = match since_str.as_str() {
            "5m" => Some(chrono::Duration::minutes(5)),
            "10m" => Some(chrono::Duration::minutes(10)),
            "30m" => Some(chrono::Duration::minutes(30)),
            "1h" => Some(chrono::Duration::hours(1)),
            "1d" => Some(chrono::Duration::days(1)),
            _ => {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(since_str) {
                    Some(chrono::Utc::now().signed_duration_since(dt.with_timezone(&chrono::Utc)))
                } else {
                    None
                }
            }
        };

        if let Some(d) = duration {
            let start = (chrono::Utc::now() - d).timestamp_micros() as u64;
            if let Err(e) = journal.seek_realtime_usec(start) {
                return IvaldiResponse::from_error(IvaldiError::Systemd(e.to_string()));
            }
        }
    } else {
        journal.seek_tail().ok();
    }

    let mut logs = Vec::new();
    let mut count = 0;
    
    let regex = if let Some(p) = &args.pattern {
        match regex::Regex::new(p) {
            Ok(r) => Some(r),
            Err(e) => return IvaldiResponse::from_error(IvaldiError::InvalidArgument(format!("Invalid regex: {}", e))),
        }
    } else {
        None
    };

    while count < args.limit {
        match journal.previous() {
            Ok(0) => break, // EOF
            Ok(_) => {
                let service_val = if let Ok(Some(data)) = journal.get_data("_SYSTEMD_UNIT") {
                    data.value().map(|b| String::from_utf8_lossy(b).into_owned())
                } else {
                    None
                };
                
                if let Some(s) = &args.service {
                    if service_val.as_ref() != Some(s) {
                        continue;
                    }
                }

                let priority_val = if let Ok(Some(data)) = journal.get_data("PRIORITY") {
                    data.value().and_then(|b| String::from_utf8_lossy(b).parse::<i32>().ok())
                } else {
                    None
                }.unwrap_or(6); // Info default

                if let Some(l) = args.level {
                    if priority_val > l.to_priority() {
                        continue;
                    }
                }

                let message = if let Ok(Some(data)) = journal.get_data("MESSAGE") {
                    data.value().map(|b| String::from_utf8_lossy(b).into_owned())
                } else {
                    None
                }.unwrap_or_default();

                if let Some(re) = &regex {
                    if !re.is_match(&message) {
                        continue;
                    }
                }

                let timestamp = if let Ok(Some(data)) = journal.get_data("__REALTIME_TIMESTAMP") {
                    data.value().and_then(|b| String::from_utf8_lossy(b).parse::<i64>().ok())
                } else {
                    None
                }.map(|ts| {
                    let naive = chrono::DateTime::from_timestamp(ts / 1_000_000, (ts % 1_000_000) as u32 * 1000).map(|dt| dt.naive_utc()).unwrap_or_default();
                    chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive, chrono::Utc).to_rfc3339()
                }).unwrap_or_else(|| "unknown".to_string());

                let pid = if let Ok(Some(data)) = journal.get_data("_PID") {
                    data.value().and_then(|b| String::from_utf8_lossy(b).parse::<u32>().ok())
                } else {
                    None
                };

                logs.push(LogEntry {
                    timestamp,
                    level: LogLevel::from_priority(priority_val),
                    message,
                    service: service_val,
                    pid,
                    fields: HashMap::new(),
                });
                count += 1;
            }
            Err(_) => break,
        }
    }

    IvaldiResponse::success(serde_json::json!({
        "logs": logs
    }))
}
