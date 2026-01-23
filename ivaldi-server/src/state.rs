use anyhow::Result;
use ivaldi_core::config::GlobalConfig;
use ivaldi_core::policy::PolicyEngine;
use ivaldi_core::session::{Session, SessionManager};
use ivaldi_server::cli;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

/// Shared application state
///
/// Uses Arc internally to allow cloning for use in multiple async tasks.
#[derive(Clone)]
pub struct ServerState {
    inner: Arc<StateInner>,
}

struct StateInner {
    session_manager: Mutex<SessionManager>,
    current_session: RwLock<Option<Session>>,
    config: GlobalConfig,
    policy_engine: Arc<PolicyEngine>,
    tool_namespace: Option<String>,
    response_mode: cli::ResponseMode,
}

impl ServerState {
    pub fn new(
        config: GlobalConfig,
        tool_namespace: Option<String>,
        response_mode: cli::ResponseMode,
    ) -> Result<Self> {
        let session_manager = SessionManager::new()?;
        // config is now passed in

        // Initialize Policy Engine using search hierarchy:
        // 1. Local (./.ivaldi/policies)
        // 2. Global (~/.config/ivaldi/policies)
        // 3. Fallback: SILENT ALLOW ALL
        let local_path = Path::new(".ivaldi/policies");
        let policy_path = if local_path.exists() {
            Some(local_path.to_path_buf())
        } else {
            // Check global config directory
            let xdg_config = std::env::var("XDG_CONFIG_HOME")
                .ok()
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    std::env::var("HOME")
                        .ok()
                        .map(|h| std::path::PathBuf::from(h).join(".config"))
                });

            xdg_config
                .map(|p| p.join("ivaldi").join("policies"))
                .filter(|p| p.exists())
        };

        let _ = if let Some(ref path) = policy_path {
            tracing::info!(path = ?path, "Policy: loading from file");
        } else {
            tracing::info!("Policy: no file found, using silent default (ALLOW ALL)");
        };

        let policy_engine = PolicyEngine::new(policy_path.as_deref()).unwrap_or_else(|e| {
            tracing::error!(
                "Failed to initialize policy engine: {}. Defaulting to silent ALLOW ALL.",
                e
            );
            PolicyEngine::permissive()
        });

        Ok(Self {
            inner: Arc::new(StateInner {
                session_manager: Mutex::new(session_manager),
                current_session: RwLock::new(None),
                config,
                policy_engine: Arc::new(policy_engine),
                tool_namespace,
                response_mode,
            }),
        })
    }

    pub fn session_manager(&self) -> &Mutex<SessionManager> {
        &self.inner.session_manager
    }

    pub fn config(&self) -> &GlobalConfig {
        &self.inner.config
    }

    pub fn policy_engine(&self) -> &Arc<PolicyEngine> {
        &self.inner.policy_engine
    }

    pub fn set_session(&self, session: Session) {
        let mut guard = self.inner.current_session.write().unwrap();
        *guard = Some(session);
    }

    pub fn get_session(&self) -> Option<Session> {
        self.inner.current_session.read().unwrap().clone()
    }

    pub fn tool_namespace(&self) -> &Option<String> {
        &self.inner.tool_namespace
    }

    pub fn response_mode(&self) -> &cli::ResponseMode {
        &self.inner.response_mode
    }
}
