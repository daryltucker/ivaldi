use std::sync::{Arc, Mutex, RwLock};
use ivaldi_core::session::{SessionManager, Session};
use ivaldi_core::config::GlobalConfig;
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
}

impl ServerState {
    pub fn new() -> Result<Self> {
        let session_manager = SessionManager::new()?;
        let config = GlobalConfig::default();

        Ok(Self {
            inner: Arc::new(StateInner {
                session_manager: Mutex::new(session_manager),
                current_session: RwLock::new(None),
                config,
            }),
        })
    }
    
    pub fn session_manager(&self) -> &Mutex<SessionManager> {
        &self.inner.session_manager
    }

    pub fn config(&self) -> &GlobalConfig {
        &self.inner.config
    }

    pub fn set_session(&self, session: Session) {
        let mut guard = self.inner.current_session.write().unwrap();
        *guard = Some(session);
    }
    
    pub fn get_session(&self) -> Option<Session> {
        self.inner.current_session.read().unwrap().clone()
    }
}
