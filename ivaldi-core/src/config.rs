use crate::execution::SafetyConfig;
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

    /// Execution safety configuration
    #[serde(default)]
    pub safety: SafetyConfig,
}

impl GlobalConfig {
    /// Load configuration from a specific path, or return defaults
    pub fn load(path: Option<&std::path::Path>) -> anyhow::Result<Self> {
        if let Some(p) = path {
            if p.exists() {
                let contents = std::fs::read_to_string(p)?;
                // Try TOML first (standard for rust configs)
                let config: Self = toml::from_str(&contents)
                    .or_else(|_| serde_json::from_str(&contents))
                    .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))?;
                return Ok(config);
            }
        }
        Ok(Self::default())
    }
}
