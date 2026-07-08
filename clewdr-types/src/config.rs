use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RequestParamRule {
    /// Model patterns this rule applies to. Supports exact match and `*` wildcards.
    #[serde(default)]
    pub models: Vec<String>,
    /// Top-level request body keys to remove.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Top-level request body parameters to add or override.
    #[serde(default)]
    pub params: Map<String, Value>,
}

fn default_prompt_cache_anchor_text() -> String {
    "[Start a new Chat]".to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConfigApi {
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub check_update: bool,
    #[serde(default)]
    pub auto_update: bool,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub admin_password: String,
    pub proxy: Option<String>,
    pub rproxy: Option<String>,
    #[serde(default)]
    pub max_retries: usize,
    #[serde(default)]
    pub preserve_chats: bool,
    #[serde(default)]
    pub web_search: bool,
    #[serde(default)]
    pub enable_web_count_tokens: bool,
    #[serde(default)]
    pub sanitize_messages: bool,
    #[serde(default)]
    pub request_param_rules: Vec<RequestParamRule>,
    #[serde(default = "default_request_body_archive_limit")]
    pub request_body_archive_limit: usize,
    #[serde(default)]
    pub prompt_cache_anchor_enabled: bool,
    #[serde(default)]
    pub prompt_cache_anchor_models: Vec<String>,
    #[serde(default = "default_prompt_cache_anchor_text")]
    pub prompt_cache_anchor_text: String,
    #[serde(default)]
    pub skip_first_warning: bool,
    #[serde(default)]
    pub skip_second_warning: bool,
    #[serde(default)]
    pub skip_restricted: bool,
    #[serde(default)]
    pub skip_non_pro: bool,
    #[serde(default)]
    pub skip_rate_limit: bool,
    #[serde(default)]
    pub skip_normal_pro: bool,
    #[serde(default)]
    pub use_real_roles: bool,
    pub custom_h: Option<String>,
    pub custom_a: Option<String>,
    #[serde(default)]
    pub custom_prompt: String,
    pub claude_code_client_id: Option<String>,
    pub custom_system: Option<String>,
}

fn default_request_body_archive_limit() -> usize {
    50
}
