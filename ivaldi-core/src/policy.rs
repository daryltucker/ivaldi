use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use cedar_policy::{
    Authorizer, Context, Decision, Entities, EntityUid, Policy, PolicyId, PolicySet, Request,
};

use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Failed to parse policy: {0}")]
    ParseError(#[from] cedar_policy::ParseErrors),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Policy error: {0}")]
    PolicySetError(#[from] cedar_policy::PolicySetError),
    #[error("Invalid entity: {0}")]
    EntityError(String),
}

/// The PolicyEngine enforces permissions using Cedar.
pub struct PolicyEngine {
    policy_set: Arc<PolicySet>,
    authorizer: Authorizer,
}

impl PolicyEngine {
    /// Create a new PolicyEngine, loading all `.cedar` files from the given directory (if provided).
    pub fn new(policy_dir: Option<&Path>) -> Result<Self, PolicyError> {
        let mut policy_set = PolicySet::new();
        let mut loaded_any = false;

        if let Some(dir) = policy_dir {
            if dir.exists() {
                for entry in std::fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| ext == "cedar") {
                        let src = std::fs::read_to_string(&path)?;
                        let filename = path.file_stem().unwrap().to_string_lossy();
                        let policy_id = PolicyId::from_str(&filename).unwrap_or_else(|_| PolicyId::from_str("default").unwrap());
                        
                        let policy = Policy::parse(Some(policy_id), &src)?;
                        policy_set.add(policy)?;
                        info!("Loaded policy file: {:?}", path);
                        loaded_any = true;
                    }
                }
            }
        }

        if !loaded_any {
            tracing::debug!("No custom policies loaded. Using default ALLOW ALL policy.");
        }

        // Base Policy: ALLOW ALL. 
        // This ensures tools work by default, and users only need to add 'forbid' rules to restrict them.
        let src = r#"permit(principal, action, resource);"#;
        let policy_id = PolicyId::from_str("static_permissive").unwrap();
        let policy = Policy::parse(Some(policy_id), src).expect("Static permissive policy failed");
        policy_set.add(policy).expect("Failed to add permissive policy");

        Ok(Self {
            policy_set: Arc::new(policy_set),
            authorizer: Authorizer::new(),
        })
    }
    
    /// Create a permissive engine (useful for testing/fallback)
    pub fn permissive() -> Self {
        let mut policy_set = PolicySet::new();
        // A simple "permit everything" policy
        // ID: static_permissive
        let src = r#"permit(principal, action, resource);"#;
        let policy_id = PolicyId::from_str("static_permissive").unwrap();
        let policy = Policy::parse(Some(policy_id), src).expect("Static permissive policy failed");
        policy_set.add(policy).expect("Failed to add permissive policy");
        
        Self {
            policy_set: Arc::new(policy_set),
            authorizer: Authorizer::new(),
        }
    }

    /// Check if an action is authorized
    pub fn check(
        &self,
        principal_str: &str,
        action_str: &str,
        resource_str: &str,
    ) -> Result<bool, PolicyError> {
        let principal = EntityUid::from_str(principal_str)
            .map_err(|e| PolicyError::EntityError(format!("Invalid principal: {}", e)))?;
        let action = EntityUid::from_str(action_str)
            .map_err(|e| PolicyError::EntityError(format!("Invalid action: {}", e)))?;
        let resource = EntityUid::from_str(resource_str)
            .map_err(|e| PolicyError::EntityError(format!("Invalid resource: {}", e)))?;

        let request = Request::new(
            principal,
            action,
            resource,
            Context::empty(),
            None,
        ).map_err(|e| PolicyError::EntityError(format!("Request validation error: {}", e)))?;

        let entities = Entities::empty(); // No entity hierarchy for now
        let response = self.authorizer.is_authorized(&request, &self.policy_set, &entities);

        match response.decision() {
            Decision::Allow => Ok(true),
            Decision::Deny => {
                info!("Access denied by policy. Diagnosis: {:?}", response.diagnostics());
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_default_allow_all() {
        let dir = tempdir().unwrap();
        let engine = PolicyEngine::new(Some(dir.path())).unwrap();
        
        let allowed = engine.check(
            "User::\"daryl\"",
            "Action::\"read_file\"",
            "Resource::\"/etc/passwd\""
        ).unwrap();
        
        assert!(allowed, "Default policy should permit all actions");
    }

    #[test]
    fn test_explicit_forbid() {
        let dir = tempdir().unwrap();
        let policy_path = dir.path().join("secure.cedar");
        fs::write(&policy_path, r#"forbid(principal, action == Action::"delete_file", resource);"#).unwrap();
        
        let engine = PolicyEngine::new(Some(dir.path())).unwrap();
        
        // delete_file should be denied
        let allowed = engine.check(
            "User::\"daryl\"",
            "Action::\"delete_file\"",
            "Resource::\"/tags\""
        ).unwrap();
        assert!(!allowed, "Explicit forbid should deny access");
        
        // other actions should still be allowed by default permit
        let allowed = engine.check(
            "User::\"daryl\"",
            "Action::\"read_file\"",
            "Resource::\"/tags\""
        ).unwrap();
        assert!(allowed, "Other actions should still be allowed");
    }
}
