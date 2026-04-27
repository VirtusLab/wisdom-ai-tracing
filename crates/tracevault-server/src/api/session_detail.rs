use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use tracevault_core::agent_adapter::AgentAdapter;

use crate::error::AppError;
use crate::extractors::OrgAuth;
use crate::pricing::{self, ModelPricing};
use crate::AppState;

#[derive(Serialize)]
pub struct SessionDetailResponse {
    pub session_id: String,
    pub model: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub total_tokens: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub estimated_cost_usd: f64,
    pub api_calls: i32,
    pub user_messages: i32,
    pub assistant_messages: i32,
    pub total_tool_calls: i32,
    pub compactions: i32,
    pub cache_savings: CacheSavings,
    pub per_call: Vec<PerCallUsage>,
    pub cost_breakdown: CostBreakdown,
    pub token_distribution: TokenDistribution,
    pub transcript_records: Vec<TranscriptRecord>,
}

#[derive(Serialize)]
pub struct CacheSavings {
    pub gross_savings_usd: f64,
    pub cache_write_overhead_usd: f64,
    pub net_savings_usd: f64,
    pub cache_hit_percentage: f64,
}

#[derive(Serialize)]
pub struct PerCallUsage {
    pub index: u32,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub cost_usd: f64,
    pub cumulative_cost_usd: f64,
    pub model: String,
}

#[derive(Serialize)]
pub struct CostBreakdown {
    pub input_cost: f64,
    pub output_cost: f64,
    pub cache_read_cost: f64,
    pub cache_write_cost: f64,
    pub total_cost: f64,
}

#[derive(Serialize)]
pub struct TokenDistribution {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
}

#[derive(Debug, Serialize)]
pub struct RecordUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Serialize)]
pub struct TranscriptRecord {
    pub record_type: String,
    pub timestamp: Option<String>,
    pub content_types: Vec<String>,
    pub tool_name: Option<String>,
    pub text: Option<String>,
    pub usage: Option<RecordUsage>,
    pub model: Option<String>,
}

pub fn parse_transcript(
    transcript: &serde_json::Value,
    pricing: &ModelPricing,
    adapter: &dyn AgentAdapter,
) -> (
    Vec<PerCallUsage>,
    Vec<TranscriptRecord>,
    TokenDistribution,
    CostBreakdown,
    CacheSavings,
) {
    let records = transcript.as_array().map(|a| a.as_slice()).unwrap_or(&[]);

    let mut per_call = Vec::new();
    let mut transcript_records = Vec::new();
    let mut cumulative_cost = 0.0;
    let mut call_index: u32 = 0;

    let mut total_input: i64 = 0;
    let mut total_output: i64 = 0;
    let mut total_cache_read: i64 = 0;
    let mut total_cache_write: i64 = 0;

    for record in records {
        if let Some(parsed) = adapter.parse_transcript_record(record) {
            let usage = if parsed.raw_input_tokens.is_some() {
                let total_input_raw = parsed.raw_input_tokens.unwrap_or(0);
                let output = parsed.raw_output_tokens.unwrap_or(0);
                let cache_read = parsed.raw_cache_read_tokens.unwrap_or(0);
                let cache_write = parsed.raw_cache_write_tokens.unwrap_or(0);
                let fresh_input = (total_input_raw - cache_read - cache_write).max(0);
                let cost = pricing::estimate_cost_with_pricing(
                    pricing,
                    fresh_input,
                    output,
                    cache_read,
                    cache_write,
                );
                Some(RecordUsage {
                    input_tokens: fresh_input,
                    output_tokens: output,
                    cache_read_tokens: cache_read,
                    cache_write_tokens: cache_write,
                    cost_usd: cost,
                })
            } else {
                None
            };

            let tr = TranscriptRecord {
                record_type: parsed.record_type.clone(),
                timestamp: parsed.timestamp,
                content_types: parsed.content_types,
                tool_name: parsed.tool_name,
                text: parsed.text,
                usage,
                model: parsed.model,
            };

            if tr.record_type == "assistant" {
                if let Some(ref usage) = tr.usage {
                    let model = tr.model.as_deref().unwrap_or("unknown");
                    if model != "<synthetic>"
                        && (usage.input_tokens > 0
                            || usage.output_tokens > 0
                            || usage.cache_read_tokens > 0
                            || usage.cache_write_tokens > 0)
                    {
                        call_index += 1;
                        cumulative_cost += usage.cost_usd;

                        total_input += usage.input_tokens;
                        total_output += usage.output_tokens;
                        total_cache_read += usage.cache_read_tokens;
                        total_cache_write += usage.cache_write_tokens;

                        per_call.push(PerCallUsage {
                            index: call_index,
                            input_tokens: usage.input_tokens,
                            output_tokens: usage.output_tokens,
                            cache_read_tokens: usage.cache_read_tokens,
                            cache_write_tokens: usage.cache_write_tokens,
                            cost_usd: usage.cost_usd,
                            cumulative_cost_usd: cumulative_cost,
                            model: model.to_string(),
                        });
                    }
                }
            }
            transcript_records.push(tr);
        }
    }

    let token_distribution = TokenDistribution {
        input_tokens: total_input,
        output_tokens: total_output,
        cache_read_tokens: total_cache_read,
        cache_write_tokens: total_cache_write,
    };

    let cost_breakdown = CostBreakdown {
        input_cost: (total_input as f64 / 1_000_000.0) * pricing.input_per_m,
        output_cost: (total_output as f64 / 1_000_000.0) * pricing.output_per_m,
        cache_read_cost: (total_cache_read as f64 / 1_000_000.0) * pricing.cache_read_per_m,
        cache_write_cost: (total_cache_write as f64 / 1_000_000.0) * pricing.cache_write_per_m,
        total_cost: cumulative_cost,
    };

    let total_input_side = total_cache_read + total_cache_write + total_input;
    let gross_savings =
        (total_cache_read as f64 / 1_000_000.0) * (pricing.input_per_m - pricing.cache_read_per_m);
    let write_overhead = pricing::estimate_cache_write_overhead(pricing, total_cache_write);

    let cache_savings = CacheSavings {
        gross_savings_usd: gross_savings,
        cache_write_overhead_usd: write_overhead,
        net_savings_usd: gross_savings - write_overhead,
        cache_hit_percentage: if total_input_side > 0 {
            (total_cache_read as f64 / total_input_side as f64) * 100.0
        } else {
            0.0
        },
    };

    (
        per_call,
        transcript_records,
        token_distribution,
        cost_breakdown,
        cache_savings,
    )
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    session_id: String,
    model: Option<String>,
    tool: Option<String>,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
    duration_ms: Option<i64>,
    total_tokens: Option<i64>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
    estimated_cost_usd: Option<f64>,
    user_messages: Option<i32>,
    assistant_messages: Option<i32>,
    total_tool_calls: Option<i32>,
}

pub async fn get_session_detail(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, session_uuid)): Path<(String, Uuid)>,
) -> Result<Json<SessionDetailResponse>, AppError> {
    let org_id = auth.org_id;

    let row = sqlx::query_as::<_, SessionRow>(
        "SELECT s.session_id, s.model, s.tool, s.started_at, s.ended_at, s.duration_ms,
                s.total_tokens, s.input_tokens, s.output_tokens,
                s.cache_read_tokens, s.cache_write_tokens,
                s.estimated_cost_usd,
                s.user_messages, s.assistant_messages,
                s.total_tool_calls
         FROM sessions s
         JOIN repos r ON s.repo_id = r.id
         WHERE s.id = $1 AND r.org_id = $2",
    )
    .bind(session_uuid)
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".into()))?;

    let pricing = pricing::fetch_pricing_for_model(
        &state.pool,
        row.model.as_deref().unwrap_or("sonnet"),
        row.started_at,
    )
    .await;

    // Reassemble transcript from transcript_chunks
    let chunks: Vec<(serde_json::Value,)> = sqlx::query_as(
        "SELECT data FROM transcript_chunks
         WHERE session_id = $1
         ORDER BY chunk_index ASC",
    )
    .bind(session_uuid)
    .fetch_all(&state.pool)
    .await?;

    let transcript_array: Vec<serde_json::Value> = chunks.into_iter().map(|(d,)| d).collect();
    let transcript_val = serde_json::Value::Array(transcript_array);
    let adapter = state
        .agent_registry
        .get(row.tool.as_deref().unwrap_or("claude-code"));
    let (per_call, transcript_records, token_distribution, cost_breakdown, cache_savings) =
        parse_transcript(&transcript_val, &pricing, adapter);

    // Count API calls from per_call data since api_calls column doesn't exist on sessions
    let api_calls = per_call.len() as i32;

    Ok(Json(SessionDetailResponse {
        session_id: row.session_id,
        model: row.model,
        started_at: row.started_at,
        ended_at: row.ended_at,
        duration_ms: row.duration_ms,
        total_tokens: row.total_tokens.unwrap_or(0),
        input_tokens: row.input_tokens.unwrap_or(0),
        output_tokens: row.output_tokens.unwrap_or(0),
        cache_read_tokens: row.cache_read_tokens.unwrap_or(0),
        cache_write_tokens: row.cache_write_tokens.unwrap_or(0),
        estimated_cost_usd: row.estimated_cost_usd.unwrap_or(0.0),
        api_calls,
        user_messages: row.user_messages.unwrap_or(0),
        assistant_messages: row.assistant_messages.unwrap_or(0),
        total_tool_calls: row.total_tool_calls.unwrap_or(0),
        compactions: 0,
        cache_savings,
        per_call,
        cost_breakdown,
        token_distribution,
        transcript_records,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracevault_core::agent_adapter::AgentAdapterRegistry;

    fn test_pricing() -> ModelPricing {
        ModelPricing {
            input_per_m: 15.0,
            output_per_m: 75.0,
            cache_write_per_m: 18.75,
            cache_read_per_m: 1.50,
        }
    }

    #[test]
    fn test_parse_empty_transcript() {
        let transcript = serde_json::json!([]);
        let registry = AgentAdapterRegistry::new();
        let adapter = registry.get("claude-code");
        let (per_call, records, dist, cost, savings) =
            parse_transcript(&transcript, &test_pricing(), adapter);
        assert!(per_call.is_empty());
        assert!(records.is_empty());
        assert_eq!(dist.input_tokens, 0);
        assert_eq!(cost.total_cost, 0.0);
        assert_eq!(savings.cache_hit_percentage, 0.0);
    }

    #[test]
    fn test_parse_assistant_with_usage() {
        let transcript = serde_json::json!([
            {
                "type": "assistant",
                "timestamp": "2026-03-23T13:17:16Z",
                "message": {
                    "model": "claude-opus-4-6",
                    "content": [{"type": "text", "text": "Hello"}],
                    "usage": {
                        "input_tokens": 100,
                        "output_tokens": 50,
                        "cache_read_input_tokens": 1000,
                        "cache_creation_input_tokens": 500
                    }
                }
            }
        ]);
        let registry = AgentAdapterRegistry::new();
        let adapter = registry.get("claude-code");
        let (per_call, records, dist, _cost, _savings) =
            parse_transcript(&transcript, &test_pricing(), adapter);
        assert_eq!(per_call.len(), 1);
        assert_eq!(per_call[0].index, 1);
        assert_eq!(per_call[0].cache_read_tokens, 1000);
        assert_eq!(records.len(), 1);
        assert_eq!(dist.output_tokens, 50);
    }

    #[test]
    fn test_skips_synthetic_records() {
        let transcript = serde_json::json!([
            {
                "type": "assistant",
                "timestamp": "2026-03-23T13:17:16Z",
                "message": {
                    "model": "<synthetic>",
                    "content": [],
                    "usage": { "input_tokens": 0, "output_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0 }
                }
            }
        ]);
        let registry = AgentAdapterRegistry::new();
        let adapter = registry.get("claude-code");
        let (per_call, records, _dist, _cost, _savings) =
            parse_transcript(&transcript, &test_pricing(), adapter);
        assert!(per_call.is_empty());
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_cache_savings_calculation() {
        let transcript = serde_json::json!([
            {
                "type": "assistant",
                "timestamp": "2026-03-23T13:17:16Z",
                "message": {
                    "model": "claude-opus-4-6",
                    "content": [{"type": "text", "text": "test"}],
                    "usage": {
                        "input_tokens": 0,
                        "output_tokens": 0,
                        "cache_read_input_tokens": 1_000_000,
                        "cache_creation_input_tokens": 100_000
                    }
                }
            }
        ]);
        let pricing = test_pricing();
        let registry = AgentAdapterRegistry::new();
        let adapter = registry.get("claude-code");
        let (_per_call, _records, _dist, _cost, savings) =
            parse_transcript(&transcript, &pricing, adapter);
        assert!((savings.gross_savings_usd - 13.5).abs() < 0.001);
        assert!((savings.cache_write_overhead_usd - 0.375).abs() < 0.001);
        assert!((savings.net_savings_usd - 13.125).abs() < 0.001);
    }

    #[test]
    fn test_cumulative_cost_accumulates() {
        let transcript = serde_json::json!([
            {
                "type": "assistant",
                "timestamp": "2026-03-23T13:17:16Z",
                "message": {
                    "model": "claude-opus-4-6",
                    "content": [{"type": "text", "text": "a"}],
                    "usage": { "input_tokens": 1_000_000, "output_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0 }
                }
            },
            {
                "type": "assistant",
                "timestamp": "2026-03-23T13:17:20Z",
                "message": {
                    "model": "claude-opus-4-6",
                    "content": [{"type": "text", "text": "b"}],
                    "usage": { "input_tokens": 1_000_000, "output_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0 }
                }
            }
        ]);
        let registry = AgentAdapterRegistry::new();
        let adapter = registry.get("claude-code");
        let (per_call, _, _, _, _) = parse_transcript(&transcript, &test_pricing(), adapter);
        assert_eq!(per_call.len(), 2);
        assert!((per_call[0].cumulative_cost_usd - 15.0).abs() < 0.001);
        assert!((per_call[1].cumulative_cost_usd - 30.0).abs() < 0.001);
    }
}
