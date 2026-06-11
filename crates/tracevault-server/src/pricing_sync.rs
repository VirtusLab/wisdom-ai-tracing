use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;

const LITELLM_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

#[derive(Debug, Deserialize)]
struct LiteLLMEntry {
    #[serde(default)]
    litellm_provider: Option<String>,
    #[serde(default)]
    input_cost_per_token: Option<f64>,
    #[serde(default)]
    output_cost_per_token: Option<f64>,
    #[serde(default)]
    cache_creation_input_token_cost: Option<f64>,
    #[serde(default)]
    cache_read_input_token_cost: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPricing {
    pub model: String,
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_write_per_mtok: f64,
    pub cache_read_per_mtok: f64,
}

#[derive(Debug, Clone)]
struct LiteLLMPricing {
    input_per_mtok: f64,
    output_per_mtok: f64,
    cache_write_per_mtok: f64,
    cache_read_per_mtok: f64,
}

fn extract_date_suffix(key: &str) -> &str {
    if key.len() >= 8 {
        let suffix = &key[key.len() - 8..];
        if suffix.chars().all(|c| c.is_ascii_digit()) {
            return suffix;
        }
    }
    ""
}

fn strip_provider_prefix(key: &str) -> &str {
    // LiteLLM keys may have prefixes like "anthropic/", "bedrock/", etc.
    key.rsplit_once('/').map(|(_, rest)| rest).unwrap_or(key)
}

/// Parse ALL entries for the given provider from LiteLLM JSON.
/// Returns a HashMap keyed by LiteLLM model key.
fn parse_litellm_entries(
    data: &[u8],
    provider: &str,
) -> Result<HashMap<String, LiteLLMPricing>, serde_json::Error> {
    let entries: HashMap<String, LiteLLMEntry> = serde_json::from_slice(data)?;
    let mut result = HashMap::new();

    for (key, entry) in entries {
        let is_provider = entry
            .litellm_provider
            .as_deref()
            .map(|p| p == provider)
            .unwrap_or(false);
        if !is_provider {
            continue;
        }
        if entry.input_cost_per_token.is_none() || entry.output_cost_per_token.is_none() {
            continue;
        }

        let to_mtok = |v: Option<f64>| v.unwrap_or(0.0) * 1_000_000.0;
        result.insert(
            key,
            LiteLLMPricing {
                input_per_mtok: to_mtok(entry.input_cost_per_token),
                output_per_mtok: to_mtok(entry.output_cost_per_token),
                cache_write_per_mtok: to_mtok(entry.cache_creation_input_token_cost),
                cache_read_per_mtok: to_mtok(entry.cache_read_input_token_cost),
            },
        );
    }

    Ok(result)
}

/// Find the best matching LiteLLM entry for a session model name.
/// e.g., "claude-sonnet-4-6" matches "claude-sonnet-4-6-20260514" or "anthropic/claude-sonnet-4-6-20260514"
fn match_litellm_entry<'a>(
    session_model: &str,
    entries: &'a HashMap<String, LiteLLMPricing>,
) -> Option<&'a LiteLLMPricing> {
    let mut best_key: Option<&str> = None;
    let mut best_entry: Option<&LiteLLMPricing> = None;

    for (key, entry) in entries {
        let stripped = strip_provider_prefix(key);
        // Check if the LiteLLM key (stripped of prefix) starts with the session model name
        if stripped.starts_with(session_model) {
            let replace = match best_key {
                None => true,
                Some(existing) => extract_date_suffix(key) > extract_date_suffix(existing),
            };
            if replace {
                best_key = Some(key);
                best_entry = Some(entry);
            }
        }
    }

    best_entry
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncResult {
    pub models_updated: Vec<String>,
    pub last_synced_at: DateTime<Utc>,
}

pub async fn sync_pricing(
    pool: &PgPool,
    http_client: &reqwest::Client,
    cost_enabled: bool,
) -> Result<SyncResult, String> {
    let bytes = http_client
        .get(LITELLM_URL)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch LiteLLM pricing: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("Failed to read LiteLLM response: {e}"))?;

    let litellm_entries = parse_litellm_entries(&bytes, "anthropic")
        .map_err(|e| format!("Failed to parse LiteLLM JSON: {e}"))?;

    if litellm_entries.is_empty() {
        tracing::warn!("No Anthropic models found in LiteLLM pricing data");
        log_sync(pool, &[], None).await;
        return Ok(SyncResult {
            models_updated: vec![],
            last_synced_at: Utc::now(),
        });
    }

    // Get distinct model names actually used in sessions
    let session_models = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT model FROM sessions WHERE model IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to query session models: {e}"))?;

    let mut updated_models = Vec::new();

    for session_model in &session_models {
        if let Some(litellm_pricing) = match_litellm_entry(session_model, &litellm_entries) {
            let entry = ParsedPricing {
                model: session_model.clone(),
                input_per_mtok: litellm_pricing.input_per_mtok,
                output_per_mtok: litellm_pricing.output_per_mtok,
                cache_write_per_mtok: litellm_pricing.cache_write_per_mtok,
                cache_read_per_mtok: litellm_pricing.cache_read_per_mtok,
            };

            let changed = diff_and_update(pool, &entry)
                .await
                .map_err(|e| format!("Failed to update pricing for {session_model}: {e}"))?;

            if changed {
                updated_models.push(session_model.clone());
            }
        } else {
            tracing::debug!("No LiteLLM pricing found for session model: {session_model}");
        }
    }

    // Recalculate affected sessions
    for model in &updated_models {
        let old_from = sqlx::query_scalar::<_, DateTime<Utc>>(
            "SELECT effective_from FROM model_pricing
             WHERE model = $1 AND source = 'litellm_sync' AND effective_until IS NOT NULL
             ORDER BY effective_until DESC LIMIT 1",
        )
        .bind(model)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        // Recalculating stored session costs is enterprise-only (cost analytics).
        if let (true, Some(from)) = (cost_enabled, old_from) {
            let pricing = crate::pricing::fetch_pricing_for_model(pool, model, None).await;
            if let Err(e) = crate::pricing::recalculate_sessions_for_pricing(
                pool, model, &pricing, from, None, None, None,
            )
            .await
            {
                tracing::error!("Failed to recalculate sessions for {model}: {e}");
            }
        }
    }

    log_sync(pool, &updated_models, None).await;

    Ok(SyncResult {
        models_updated: updated_models,
        last_synced_at: Utc::now(),
    })
}

async fn diff_and_update(pool: &PgPool, entry: &ParsedPricing) -> Result<bool, sqlx::Error> {
    let current = sqlx::query_as::<_, (uuid::Uuid, f64, f64, f64, f64)>(
        "SELECT id, input_per_mtok, output_per_mtok, cache_read_per_mtok, cache_write_per_mtok
         FROM model_pricing
         WHERE model = $1 AND source = 'litellm_sync' AND effective_until IS NULL
         ORDER BY effective_from DESC LIMIT 1",
    )
    .bind(&entry.model)
    .fetch_optional(pool)
    .await?;

    let needs_update = match &current {
        None => true,
        Some((_id, input, output, cache_read, cache_write)) => {
            let eps = 0.001;
            (entry.input_per_mtok - input).abs() > eps
                || (entry.output_per_mtok - output).abs() > eps
                || (entry.cache_read_per_mtok - cache_read).abs() > eps
                || (entry.cache_write_per_mtok - cache_write).abs() > eps
        }
    };

    if !needs_update {
        return Ok(false);
    }

    let now = Utc::now();

    if let Some((old_id, _, _, _, _)) = current {
        sqlx::query("UPDATE model_pricing SET effective_until = $1 WHERE id = $2")
            .bind(now)
            .bind(old_id)
            .execute(pool)
            .await?;
    }

    sqlx::query(
        "INSERT INTO model_pricing (model, input_per_mtok, output_per_mtok, cache_read_per_mtok, cache_write_per_mtok, effective_from, source)
         VALUES ($1, $2, $3, $4, $5, $6, 'litellm_sync')",
    )
    .bind(&entry.model)
    .bind(entry.input_per_mtok)
    .bind(entry.output_per_mtok)
    .bind(entry.cache_read_per_mtok)
    .bind(entry.cache_write_per_mtok)
    .bind(now)
    .execute(pool)
    .await?;

    tracing::info!(
        "Updated pricing for {}: input=${}/Mtok, output=${}/Mtok",
        entry.model,
        entry.input_per_mtok,
        entry.output_per_mtok
    );

    Ok(true)
}

async fn log_sync(pool: &PgPool, models_updated: &[String], error: Option<&str>) {
    let result =
        sqlx::query("INSERT INTO pricing_sync_log (models_updated, error) VALUES ($1, $2)")
            .bind(models_updated)
            .bind(error)
            .execute(pool)
            .await;

    if let Err(e) = result {
        tracing::error!("Failed to log pricing sync: {e}");
    }
}

pub async fn last_sync_time(pool: &PgPool) -> Option<DateTime<Utc>> {
    sqlx::query_scalar::<_, DateTime<Utc>>(
        "SELECT synced_at FROM pricing_sync_log WHERE error IS NULL ORDER BY synced_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entries(json: &str) -> HashMap<String, LiteLLMPricing> {
        parse_litellm_entries(json.as_bytes(), "anthropic").unwrap()
    }

    #[test]
    fn test_parse_filters_by_provider() {
        let json = r#"{
            "claude-opus-4-6-20260310": {
                "litellm_provider": "anthropic",
                "input_cost_per_token": 0.000015,
                "output_cost_per_token": 0.000075,
                "cache_creation_input_token_cost": 0.00001875,
                "cache_read_input_token_cost": 0.0000015
            },
            "gpt-4o": {
                "litellm_provider": "openai",
                "input_cost_per_token": 0.000005,
                "output_cost_per_token": 0.000015
            }
        }"#;

        let entries = make_entries(json);
        assert_eq!(entries.len(), 1);
        assert!(entries.contains_key("claude-opus-4-6-20260310"));
        let p = &entries["claude-opus-4-6-20260310"];
        assert!((p.input_per_mtok - 15.0).abs() < 0.001);
        assert!((p.output_per_mtok - 75.0).abs() < 0.001);
        assert!((p.cache_write_per_mtok - 18.75).abs() < 0.001);
        assert!((p.cache_read_per_mtok - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_parse_skips_missing_pricing() {
        let json = r#"{
            "claude-opus-4-6-20260310": {
                "litellm_provider": "anthropic"
            }
        }"#;
        let entries = make_entries(json);
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_match_finds_best_entry() {
        let json = r#"{
            "claude-sonnet-4-5-20241022": {
                "litellm_provider": "anthropic",
                "input_cost_per_token": 0.000003,
                "output_cost_per_token": 0.000015
            },
            "claude-sonnet-4-6-20260514": {
                "litellm_provider": "anthropic",
                "input_cost_per_token": 0.000002,
                "output_cost_per_token": 0.000010
            }
        }"#;
        let entries = make_entries(json);

        // "claude-sonnet-4-6" should match "claude-sonnet-4-6-20260514"
        let matched = match_litellm_entry("claude-sonnet-4-6", &entries).unwrap();
        assert!((matched.input_per_mtok - 2.0).abs() < 0.001);

        // "claude-sonnet-4-5" should match "claude-sonnet-4-5-20241022"
        let matched = match_litellm_entry("claude-sonnet-4-5", &entries).unwrap();
        assert!((matched.input_per_mtok - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_match_with_provider_prefix() {
        let json = r#"{
            "anthropic/claude-opus-4-6-20260310": {
                "litellm_provider": "anthropic",
                "input_cost_per_token": 0.000015,
                "output_cost_per_token": 0.000075
            }
        }"#;
        let entries = make_entries(json);
        let matched = match_litellm_entry("claude-opus-4-6", &entries).unwrap();
        assert!((matched.input_per_mtok - 15.0).abs() < 0.001);
    }

    #[test]
    fn test_match_picks_latest_date_suffix() {
        let json = r#"{
            "claude-sonnet-4-6-20260101": {
                "litellm_provider": "anthropic",
                "input_cost_per_token": 0.000003,
                "output_cost_per_token": 0.000015
            },
            "claude-sonnet-4-6-20260514": {
                "litellm_provider": "anthropic",
                "input_cost_per_token": 0.000002,
                "output_cost_per_token": 0.000010
            }
        }"#;
        let entries = make_entries(json);
        let matched = match_litellm_entry("claude-sonnet-4-6", &entries).unwrap();
        // Should pick 20260514 (later)
        assert!((matched.input_per_mtok - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_match_no_match() {
        let json = r#"{
            "claude-sonnet-4-6-20260514": {
                "litellm_provider": "anthropic",
                "input_cost_per_token": 0.000002,
                "output_cost_per_token": 0.000010
            }
        }"#;
        let entries = make_entries(json);
        assert!(match_litellm_entry("claude-opus-4-6", &entries).is_none());
    }

    #[test]
    fn test_extract_date_suffix() {
        assert_eq!(extract_date_suffix("claude-opus-4-6-20260310"), "20260310");
        assert_eq!(
            extract_date_suffix("anthropic/claude-opus-4-6-20260310"),
            "20260310"
        );
        assert_eq!(extract_date_suffix("gpt-4o"), "");
        assert_eq!(extract_date_suffix("short"), "");
    }

    #[test]
    fn test_strip_provider_prefix() {
        assert_eq!(
            strip_provider_prefix("anthropic/claude-opus-4-6"),
            "claude-opus-4-6"
        );
        assert_eq!(strip_provider_prefix("claude-opus-4-6"), "claude-opus-4-6");
        assert_eq!(
            strip_provider_prefix("bedrock/claude-sonnet-4-6"),
            "claude-sonnet-4-6"
        );
    }

    #[test]
    fn test_missing_cache_defaults_to_zero() {
        let json = r#"{
            "claude-haiku-4-5-20251001": {
                "litellm_provider": "anthropic",
                "input_cost_per_token": 0.0000008,
                "output_cost_per_token": 0.000004
            }
        }"#;
        let entries = make_entries(json);
        let matched = match_litellm_entry("claude-haiku-4-5", &entries).unwrap();
        assert!((matched.cache_write_per_mtok - 0.0).abs() < 0.001);
        assert!((matched.cache_read_per_mtok - 0.0).abs() < 0.001);
    }
}
