use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Global configuration for ivaldi tools and server.
/// 
/// These settings can be controlled via CLI flags or environment variables.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct GlobalConfig {
    /// Whether to evaluate and restrict operations based on .gitignore.
    /// 
    /// CLI: --enable-gitignore
    /// ENV: IVALDI_ENABLE_GITIGNORE
    #[serde(default)]
    pub enable_gitignore: bool,

    /// API Key for authenticated services (e.g. VecDB Cloud)
    /// 
    /// CLI: --api-key
    /// ENV: IVALDI_API_KEY
    #[serde(default)]
    pub api_key: Option<String>,

    /// Path to a custom configuration file
    /// 
    /// CLI: --config
    /// ENV: IVALDI_CONFIG
    #[serde(default)]
    pub config_path: Option<String>,
}
