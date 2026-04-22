use async_trait::async_trait;

#[async_trait]
pub trait StoryLlm: Send + Sync {
    async fn generate(&self, prompt: &str, max_tokens: u32) -> Result<String, String>;
    fn provider_name(&self) -> &str;
    fn model_name(&self) -> &str;
}

pub struct AnthropicLlm {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicLlm {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string()),
        }
    }
}

#[async_trait]
impl StoryLlm for AnthropicLlm {
    async fn generate(&self, prompt: &str, max_tokens: u32) -> Result<String, String> {
        let resp = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": self.model,
                "max_tokens": max_tokens,
                "messages": [{"role": "user", "content": prompt}]
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        body["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| format!("Unexpected response: {body}"))
    }

    fn provider_name(&self) -> &str {
        "anthropic"
    }
    fn model_name(&self) -> &str {
        &self.model
    }
}

pub struct OpenAiLlm {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAiLlm {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com".to_string()),
        }
    }
}

#[async_trait]
impl StoryLlm for OpenAiLlm {
    async fn generate(&self, prompt: &str, max_tokens: u32) -> Result<String, String> {
        let resp = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": self.model,
                "max_tokens": max_tokens,
                "messages": [{"role": "user", "content": prompt}]
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        body["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| format!("Unexpected response: {body}"))
    }

    fn provider_name(&self) -> &str {
        "openai"
    }
    fn model_name(&self) -> &str {
        &self.model
    }
}

pub fn create_llm_from_params(
    provider: &str,
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
) -> Option<Box<dyn StoryLlm>> {
    let model = model.unwrap_or_else(|| match provider {
        "anthropic" => "claude-sonnet-4-20250514".to_string(),
        "openai" => "gpt-4o".to_string(),
        _ => "unknown".to_string(),
    });

    match provider {
        "anthropic" => Some(Box::new(AnthropicLlm::new(api_key, model, base_url))),
        "openai" => Some(Box::new(OpenAiLlm::new(api_key, model, base_url))),
        _ => {
            tracing::warn!("Unknown LLM provider: {provider}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn anthropic_success_sends_expected_request_and_parses_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "sk-test"))
            .and(header("anthropic-version", "2023-06-01"))
            .and(header("content-type", "application/json"))
            .and(body_partial_json(serde_json::json!({
                "model": "claude-sonnet-4",
                "max_tokens": 100,
                "messages": [{"role": "user", "content": "hi"}]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type": "text", "text": "hello from claude"}]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let llm = AnthropicLlm::new(
            "sk-test".into(),
            "claude-sonnet-4".into(),
            Some(server.uri()),
        );
        let out = llm.generate("hi", 100).await.unwrap();
        assert_eq!(out, "hello from claude");
    }

    #[tokio::test]
    async fn anthropic_non_2xx_returns_err_or_parse_err() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;

        let llm = AnthropicLlm::new("k".into(), "m".into(), Some(server.uri()));
        // Current implementation parses JSON even on non-2xx and then looks
        // for the text field; either a reqwest error or a "Unexpected
        // response" error is acceptable — we just need it to surface.
        let err = llm.generate("hi", 10).await.unwrap_err();
        assert!(!err.is_empty());
    }

    #[tokio::test]
    async fn openai_success_sends_bearer_auth_and_parses_choices() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer sk-openai"))
            .and(header("content-type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "hello from gpt"}}]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let llm = OpenAiLlm::new("sk-openai".into(), "gpt-4o".into(), Some(server.uri()));
        let out = llm.generate("hi", 50).await.unwrap();
        assert_eq!(out, "hello from gpt");
    }

    #[tokio::test]
    async fn openai_malformed_body_returns_err() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": []  // no message content
            })))
            .mount(&server)
            .await;

        let llm = OpenAiLlm::new("k".into(), "m".into(), Some(server.uri()));
        let err = llm.generate("hi", 10).await.unwrap_err();
        assert!(err.contains("Unexpected response"));
    }

    #[test]
    fn create_llm_unknown_provider_returns_none() {
        let r = create_llm_from_params("made-up", "k".into(), None, None);
        assert!(r.is_none());
    }

    #[test]
    fn create_llm_defaults_model_per_provider() {
        let a = create_llm_from_params("anthropic", "k".into(), None, None).unwrap();
        assert_eq!(a.provider_name(), "anthropic");
        assert!(a.model_name().starts_with("claude"));

        let o = create_llm_from_params("openai", "k".into(), None, None).unwrap();
        assert_eq!(o.provider_name(), "openai");
        assert_eq!(o.model_name(), "gpt-4o");
    }
}
