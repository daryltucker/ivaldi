use std::sync::{Arc, Mutex, RwLock};
use std::path::Path;
use ivaldi_core::session::{SessionManager, Session};
use ivaldi_core::config::GlobalConfig;
use ivaldi_core::policy::PolicyEngine;
use anyhow::Result;

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
}

impl ServerState {
    pub fn new(config: GlobalConfig) -> Result<Self> {
        let session_manager = SessionManager::new()?;
        // config is now passed in

        // Initialize Policy Engine (defaulting to .ivaldi/policies)
        // In the future, this path should be configurable via Args or ENV
        let policy_path = Path::new(".ivaldi/policies");
        let policy_engine = PolicyEngine::new(policy_path).unwrap_or_else(|e| {
            tracing::warn!("Failed to load policies from {:?}: {}. Defaulting to empty (DENY ALL).", policy_path, e);
            // Create a safe default engine (empty = deny all)
            PolicyEngine::new(Path::new("/non/existent")).unwrap() 
        });

        Ok(Self {
            inner: Arc::new(StateInner {
                session_manager: Mutex::new(session_manager),
                current_session: RwLock::new(None),
                config,
                policy_engine: Arc::new(policy_engine),
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
}
