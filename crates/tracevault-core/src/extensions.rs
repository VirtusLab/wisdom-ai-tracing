use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;

use crate::permissions::Permission;

/// Configuration passed to enterprise extension registration.
pub struct EnterpriseConfig {
    pub encryption_key: Option<String>,
}

/// Describes which features are available in this edition.
#[derive(Debug, Clone, Serialize)]
pub struct FeatureFlags {
    pub edition: &'static str,
    pub compliance: bool,
    pub audit_trail: bool,
    pub sso: bool,
    pub story_generation: bool,
    pub advanced_analytics: bool,
    pub multi_org: bool,
    pub encryption_at_rest: bool,
    pub full_policy_engine: bool,
    pub advanced_redaction: bool,
    pub chat_search: bool,
}

/// The central registry of pluggable features.
pub struct ExtensionRegistry {
    pub features: FeatureFlags,
    pub encryption: Arc<dyn EncryptionProvider>,
    pub story: Arc<dyn StoryProvider>,
    pub pricing: Arc<dyn PricingProvider>,
    pub compliance: Arc<dyn ComplianceProvider>,
    pub permissions: Arc<dyn PermissionsProvider>,
    pub sso: Arc<dyn SsoProvider>,
}

impl Clone for ExtensionRegistry {
    fn clone(&self) -> Self {
        Self {
            features: self.features.clone(),
            encryption: Arc::clone(&self.encryption),
            story: Arc::clone(&self.story),
            pricing: Arc::clone(&self.pricing),
            compliance: Arc::clone(&self.compliance),
            permissions: Arc::clone(&self.permissions),
            sso: Arc::clone(&self.sso),
        }
    }
}

// -- Encryption --

pub trait EncryptionProvider: Send + Sync {
    fn encrypt(&self, plaintext: &str) -> Result<(String, String), String>;
    fn decrypt(&self, ciphertext_b64: &str, nonce_b64: &str) -> Result<String, String>;
    fn is_enabled(&self) -> bool;
}

// -- Story (LLM) --

#[async_trait]
pub trait StoryProvider: Send + Sync {
    async fn generate_story(&self, prompt: &str, max_tokens: u32) -> Result<String, String>;
    fn is_available(&self) -> bool;
    fn provider_name(&self) -> &str;
    fn model_name(&self) -> &str;
}

// -- Pricing --

pub trait PricingProvider: Send + Sync {
    fn estimate_cost(
        &self,
        model: &str,
        input_tokens: i64,
        output_tokens: i64,
        cache_read_tokens: i64,
        cache_write_tokens: i64,
    ) -> f64;

    fn estimate_cache_savings(&self, model: &str, cache_read_tokens: i64) -> f64;

    fn cost_from_model_usage(
        &self,
        model_usage: Option<&serde_json::Value>,
        fallback_model: Option<&str>,
        fallback_input: i64,
        fallback_output: i64,
        fallback_cache_read: i64,
        fallback_cache_write: i64,
    ) -> f64;
}

// -- Compliance --

pub trait ComplianceProvider: Send + Sync {
    fn is_available(&self) -> bool;
    fn available_modes(&self) -> Vec<String>;
}

// -- Permissions --

pub trait PermissionsProvider: Send + Sync {
    fn valid_roles(&self) -> &[&str];
    fn role_permissions(&self, role: &str) -> std::collections::HashSet<Permission>;
    fn has_permission(&self, role: &str, perm: Permission) -> bool;
    fn is_valid_role(&self, role: &str) -> bool;
}

// -- SSO --

#[derive(Debug, Clone)]
pub struct SsoUserInfo {
    pub subject: String,
    pub email: String,
    pub name: Option<String>,
}

#[async_trait]
pub trait SsoProvider: Send + Sync {
    fn is_enabled(&self) -> bool;

    /// Build the OIDC authorization URL that the user's browser should be redirected to.
    async fn authorization_url(
        &self,
        issuer_url: &str,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
        state: &str,
    ) -> Result<String, String>;

    /// Exchange an authorization code for user identity claims.
    async fn exchange_code(
        &self,
        issuer_url: &str,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
        code: &str,
    ) -> Result<SsoUserInfo, String>;
}
