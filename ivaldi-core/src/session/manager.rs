use super::types::{Session, SessionMetadata, SessionStore};
use super::conversation::{ConversationContext, ConversationMode};
use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use std::fs;
use std::env;
use chrono::Utc;
use std::sync::{Arc, RwLock};

pub struct SessionManager {
    config_path: PathBuf,
    store: Arc<RwLock<SessionStore>>,
}

impl SessionManager {
    /// Create a new SessionManager, loading from the default config path
    pub fn new() -> Result<Self> {
        let config_entry = env::var("IVALDI_CONFIG").map(PathBuf::from).ok();
        
        let config_dir = if let Some(path) = config_entry {
            if path.is_file() || (path.exists() && !path.is_dir()) {
                path.parent().unwrap_or(&path).to_path_buf()
            } else {
                path
            }
        } else {
             env::var("HOME").map(|h| PathBuf::from(h).join(".config/ivaldi"))
                .context("Could not determine config directory")?
        };

        let config_path = config_dir.join("sessions.toml");
        Self::new_with_path(config_path)
    }

    /// Create with explicit config path (for testing)
    pub fn new_with_path(config_path: PathBuf) -> Result<Self> {
        let mut manager = Self {
            config_path,
            store: Arc::new(RwLock::new(SessionStore::default())),
        };
        
        // Attempt to load existing sessions, ignore if file missing
        let _ = manager.load();
        
        Ok(manager)
    }

    /// Load sessions from disk
    pub fn load(&mut self) -> Result<()> {
        if !self.config_path.exists() {
            return Ok(());
        }
        
        let content = fs::read_to_string(&self.config_path)
            .context("Failed to read sessions.toml")?;
            
        let store: SessionStore = toml::from_str(&content)
            .context("Failed to parse sessions.toml")?;
            
        self.store = Arc::new(RwLock::new(store));
        Ok(())
    }

    /// Save sessions to disk
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let store = self.store.read().unwrap();
        let content = toml::to_string_pretty(&*store)
            .context("Failed to serialize sessions")?;
            
        // Atomic write
        let tmp_path = self.config_path.with_extension("tmp");
        fs::write(&tmp_path, content)?;
        fs::rename(&tmp_path, &self.config_path)?;
        
        Ok(())
    }

    /// Create or retrieve a session
    pub fn load_or_create(&mut self, id: &str, root: Option<PathBuf>) -> Result<Session> {
        self.load_or_create_with_root(id, root, None)
    }
    
    /// Create or retrieve a session with explicit project_root override
    /// This is the canonical implementation used by MCP initialize
    pub fn load_or_create_with_root(
        &mut self,
        id: &str,
        root: Option<PathBuf>,
        explicit_project_root: Option<PathBuf>,
    ) -> Result<Session> {
        // Scope for read lock
        {
            let store = self.store.read().unwrap();
            if let Some(session) = store.sessions.get(id) {
                // Update last_active
                let mut session = session.clone();
                session.last_active = Utc::now();
                
                // Drop read lock to acquire write lock for update
                drop(store);
                self.update_session(session.clone())?;
                return Ok(session);
            }
        }
        
        // Create new
        let root = root.unwrap_or_else(|| {
             env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
        });
        
        // Priority: explicit_project_root > discovery
        let project_root = explicit_project_root
            .or_else(|| self.discover_project_root(&root));
        
        // Smart Label Generation
        let smart_label = if let Some(pr) = &project_root {
            pr.file_name()
              .map(|n| n.to_string_lossy().to_string())
        } else {
            root.file_name()
                .map(|n| n.to_string_lossy().to_string())
        };
        
        let mut metadata = SessionMetadata::default();
        if let Some(label) = smart_label {
            metadata.label = Some(label);
        } else {
             // Absolute fallback
             metadata.label = Some(id.to_string());
        }
        
        let session = Session {
            id: id.to_string(),
            project_root,
            root, 
            created_at: Utc::now(),
            last_active: Utc::now(),
            conversations: std::collections::HashMap::new(),
            metadata,
        };
        
        self.update_session(session.clone())?;
        Ok(session)
    }
    
    /// Update an existing session (or insert new) and save
    pub fn update_session(&mut self, session: Session) -> Result<()> {
        let mut store = self.store.write().unwrap();
        store.sessions.insert(session.id.clone(), session);
        drop(store); // release lock before save
        self.save()
    }

    /// List all sessions
    pub fn list(&self) -> Vec<Session> {
        let store = self.store.read().unwrap();
        store.sessions.values().cloned().collect()
    }

    /// Track a conversation in a session
    /// Creates or updates the conversation context, touching last_active
    pub fn track_conversation(&mut self, session_id: &str, conversation_id: &str, mode: Option<ConversationMode>) -> Result<()> {
        let mut store = self.store.write().unwrap();
        
        if let Some(session) = store.sessions.get_mut(session_id) {
            session.conversations
                .entry(conversation_id.to_string())
                .and_modify(|conv| conv.touch())
                .or_insert_with(|| {
                    if let Some(m) = mode {
                        if m == ConversationMode::Incognito {
                            ConversationContext::new_incognito(conversation_id)
                        } else {
                            ConversationContext::new(conversation_id)
                        }
                    } else {
                        ConversationContext::new(conversation_id)
                    }
                });
            
            session.last_active = Utc::now();
        }
        
        drop(store);
        self.save()
    }

    /// Get a conversation context from a session
    pub fn get_conversation(&self, session_id: &str, conversation_id: &str) -> Option<ConversationContext> {
        let store = self.store.read().unwrap();
        store.sessions.get(session_id)
            .and_then(|s| s.conversations.get(conversation_id))
            .cloned()
    }

    /// Discover project root by scanning upward for markers
    fn discover_project_root(&self, start: &Path) -> Option<PathBuf> {
        let markers = [".ivaldi", ".git", "Cargo.toml", "package.json", "pyproject.toml"];
        
        for ancestor in start.ancestors() {
            for marker in markers {
                if ancestor.join(marker).exists() {
                    return Some(ancestor.to_path_buf());
                }
            }
            // Don't traverse up past home directory to avoid scanning whole system
            if let Ok(home) = env::var("HOME") && ancestor == Path::new(&home) {
                break;
            }
        }
        None
    }

    /// Resolve a path relative to the session
    /// Hierarchy:
    /// 1. Project Root (if exists)
    /// 2. Session Root
    /// 3. IVALDI_ROOT (env)
    /// 4. Home Directory
    pub fn resolve_path(&self, session: &Session, path: &Path) -> PathBuf {
        if path.is_absolute() {
            return path.to_path_buf();
        }

        // 1. Try project root
        if let Some(project_root) = &session.project_root {
            let candidate = project_root.join(path);
            if candidate.exists() {
                return candidate;
            }
        }

        // 2. Try session root
        let candidate = session.root.join(path);
        if candidate.exists() {
            return candidate;
        }

        // 3. Try IVALDI_ROOT
        if let Ok(ivaldi_root) = env::var("IVALDI_ROOT") {
            let candidate = PathBuf::from(ivaldi_root).join(path);
            if candidate.exists() {
                return candidate;
            }
        }

        // 4. Try HOME (or return absolute join on session root if all else fails)
        if let Ok(home) = env::var("HOME") {
            let candidate = PathBuf::from(home).join(path);
             if candidate.exists() {
                return candidate;
            }
        }
        
        // Default: Relative to session root (even if doesn't exist, e.g. for creating files)
        session.root.join(path)
    }
}
