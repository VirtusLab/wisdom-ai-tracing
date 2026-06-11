// Re-export trait definitions and types from core
pub use tracevault_core::extensions::*;

use async_trait::async_trait;
use std::sync::Arc;

pub use crate::llm::StoryLlm;

// -- Community implementations --

pub struct CommunityEncryptionProvider;

impl EncryptionProvider for CommunityEncryptionProvider {
    fn encrypt(&self, plaintext: &str) -> Result<(String, String), String> {
        Ok((plaintext.to_string(), String::new()))
    }
    fn decrypt(&self, ciphertext_b64: &str, _nonce_b64: &str) -> Result<String, String> {
        Ok(ciphertext_b64.to_string())
    }
    fn is_enabled(&self) -> bool {
        false
    }
}

pub struct CommunityStoryProvider;

#[async_trait]
impl StoryProvider for CommunityStoryProvider {
    async fn generate_story(&self, _prompt: &str, _max_tokens: u32) -> Result<String, String> {
        Err("Story generation is an enterprise feature".to_string())
    }
    fn is_available(&self) -> bool {
        false
    }
    fn provider_name(&self) -> &str {
        "none"
    }
    fn model_name(&self) -> &str {
        "none"
    }
}

pub struct CommunityPricingProvider;

impl PricingProvider for CommunityPricingProvider {
    fn cost_enabled(&self) -> bool {
        false
    }
    fn estimate_cost(&self, _: &str, _: i64, _: i64, _: i64, _: i64) -> f64 {
        0.0
    }
    fn estimate_cache_savings(&self, _: &str, _: i64) -> f64 {
        0.0
    }
    fn cost_from_model_usage(
        &self,
        _: Option<&serde_json::Value>,
        _: Option<&str>,
        _: i64,
        _: i64,
        _: i64,
        _: i64,
    ) -> f64 {
        0.0
    }
}

pub struct CommunityComplianceProvider;

impl ComplianceProvider for CommunityComplianceProvider {
    fn is_available(&self) -> bool {
        false
    }
    fn available_modes(&self) -> Vec<String> {
        vec![]
    }
}

pub struct CommunityPermissionsProvider;

impl PermissionsProvider for CommunityPermissionsProvider {
    fn valid_roles(&self) -> &[&str] {
        &["admin", "developer"]
    }

    fn role_permissions(
        &self,
        role: &str,
    ) -> std::collections::HashSet<crate::permissions::Permission> {
        use crate::permissions::Permission::*;
        match role {
            "owner" | "admin" => std::collections::HashSet::from([
                TracePush,
                TraceViewAll,
                TraceViewOwn,
                PolicyManage,
                CodeBrowse,
                UserManage,
                OrgSettingsManage,
                ChatUse,
            ]),
            "developer" => {
                std::collections::HashSet::from([TracePush, TraceViewOwn, CodeBrowse, ChatUse])
            }
            _ => std::collections::HashSet::new(),
        }
    }

    fn has_permission(&self, role: &str, perm: crate::permissions::Permission) -> bool {
        self.role_permissions(role).contains(&perm)
    }

    fn is_valid_role(&self, role: &str) -> bool {
        matches!(
            role,
            "admin" | "developer" | "owner" | "policy_admin" | "auditor"
        )
    }
}

pub struct CommunitySsoProvider;

#[async_trait]
impl SsoProvider for CommunitySsoProvider {
    fn is_enabled(&self) -> bool {
        false
    }

    async fn authorization_url(
        &self,
        _issuer_url: &str,
        _client_id: &str,
        _client_secret: &str,
        _redirect_uri: &str,
        _state: &str,
    ) -> Result<String, String> {
        Err("SSO is an enterprise feature".into())
    }

    async fn exchange_code(
        &self,
        _issuer_url: &str,
        _client_id: &str,
        _client_secret: &str,
        _redirect_uri: &str,
        _code: &str,
    ) -> Result<SsoUserInfo, String> {
        Err("SSO is an enterprise feature".into())
    }
}

// -- Adapter implementations (wrapping existing services) --

pub struct FullEncryptionProvider {
    key: String,
}

impl FullEncryptionProvider {
    pub fn new(key: String) -> Self {
        Self { key }
    }
}

impl EncryptionProvider for FullEncryptionProvider {
    fn encrypt(&self, plaintext: &str) -> Result<(String, String), String> {
        crate::encryption::encrypt(plaintext, &self.key)
    }
    fn decrypt(&self, ciphertext_b64: &str, nonce_b64: &str) -> Result<String, String> {
        crate::encryption::decrypt(ciphertext_b64, nonce_b64, &self.key)
    }
    fn is_enabled(&self) -> bool {
        true
    }
}

pub struct LlmStoryProvider {
    llm: Arc<dyn StoryLlm>,
}

impl LlmStoryProvider {
    pub fn new(llm: Arc<dyn StoryLlm>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl StoryProvider for LlmStoryProvider {
    async fn generate_story(&self, prompt: &str, max_tokens: u32) -> Result<String, String> {
        self.llm.generate(prompt, max_tokens).await
    }
    fn is_available(&self) -> bool {
        true
    }
    fn provider_name(&self) -> &str {
        self.llm.provider_name()
    }
    fn model_name(&self) -> &str {
        self.llm.model_name()
    }
}

pub struct FullPricingProvider;

impl PricingProvider for FullPricingProvider {
    fn estimate_cost(
        &self,
        model: &str,
        input_tokens: i64,
        output_tokens: i64,
        cache_read_tokens: i64,
        cache_write_tokens: i64,
    ) -> f64 {
        crate::pricing::estimate_cost(
            model,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_write_tokens,
        )
    }
    fn estimate_cache_savings(&self, model: &str, cache_read_tokens: i64) -> f64 {
        crate::pricing::estimate_cache_savings(model, cache_read_tokens)
    }
    fn cost_from_model_usage(
        &self,
        model_usage: Option<&serde_json::Value>,
        fallback_model: Option<&str>,
        fallback_input: i64,
        fallback_output: i64,
        fallback_cache_read: i64,
        fallback_cache_write: i64,
    ) -> f64 {
        crate::pricing::cost_from_model_usage(
            model_usage,
            fallback_model,
            fallback_input,
            fallback_output,
            fallback_cache_read,
            fallback_cache_write,
        )
    }
}

// -- Registry construction --

pub fn community_registry() -> ExtensionRegistry {
    ExtensionRegistry {
        features: FeatureFlags {
            edition: "community",
            compliance: false,
            audit_trail: false,
            sso: false,
            story_generation: false,
            advanced_analytics: false,
            multi_org: false,
            encryption_at_rest: false,
            full_policy_engine: false,
            advanced_redaction: false,
            chat_search: false,
        },
        encryption: Arc::new(CommunityEncryptionProvider),
        story: Arc::new(CommunityStoryProvider),
        pricing: Arc::new(CommunityPricingProvider),
        compliance: Arc::new(CommunityComplianceProvider),
        permissions: Arc::new(CommunityPermissionsProvider),
        sso: Arc::new(CommunitySsoProvider),
    }
}
