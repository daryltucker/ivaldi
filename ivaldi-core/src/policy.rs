use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use cedar_policy::{
    Authorizer, Context, Decision, Entities, EntityUid, Policy, PolicyId, PolicySet, Request,
};

use thiserror::Error;
use tracing::{info, warn};

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
    /// Create a new PolicyEngine, loading all `.cedar` files from the given directory.
    pub fn new(policy_dir: &Path) -> Result<Self, PolicyError> {
        let mut policy_set = PolicySet::new();

        if policy_dir.exists() {
            for entry in std::fs::read_dir(policy_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "cedar") {
                    let src = std::fs::read_to_string(&path)?;
                    let filename = path.file_stem().unwrap().to_string_lossy();
                    let policy_id = PolicyId::from_str(&filename).unwrap_or_else(|_| PolicyId::from_str("default").unwrap());
                    
                    let policy = Policy::parse(Some(policy_id), &src)?;
                    policy_set.add(policy)?;
                    info!("Loaded policy file: {:?}", path);
                }
            }
        } else {
            // Default: Permissive for beta (or restrictive? Plan said default deny for safe execution)
            // Ideally we default deny. If no policies exist, nothing is permitted.
            warn!("Policy directory {:?} not found. Defaulting to empty policy set (DENY ALL).", policy_dir);
        }

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
