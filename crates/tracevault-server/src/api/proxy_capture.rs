//! Ledger capture for the Anthropic proxy: the per-request context, the
//! response-stream tap, and the (spawned, at-most-once) `llm_calls` write.
//!
//! Extracted from `proxy.rs` so the request-forwarding concern (auth, header
//! allow-list, upstream dispatch) and the ledger/DB-write concern stay
//! independently auditable. `proxy.rs` constructs a [`LedgerContext`] and wraps
//! the upstream byte stream in a [`CapturingStream`]; everything from there —
//! usage tap, cost estimation, row assembly, insert — lives here.

use axum::body::Bytes;
use std::time::Instant;
use uuid::Uuid;

use crate::pricing::{estimate_cost_with_pricing, fetch_pricing_for_model};
use crate::repo::llm_calls::{LlmCallRecord, LlmCallRepo};
use crate::service::usage_capture::{ParsedUsage, UsageCapture};

/// Bundle of concurrency permits that must be held for the lifetime of a
/// proxy response (including its streaming body). Permits are released in
/// field-declaration order on drop, so the per-credential permit releases
/// before the global one — the inverse of acquisition order.
pub(crate) struct HeldPermits {
    _credential: tokio::sync::OwnedSemaphorePermit,
    _global: Option<tokio::sync::OwnedSemaphorePermit>,
}

impl HeldPermits {
    pub(crate) fn new(
        credential: tokio::sync::OwnedSemaphorePermit,
        global: Option<tokio::sync::OwnedSemaphorePermit>,
    ) -> Self {
        Self {
            _credential: credential,
            _global: global,
        }
    }
}

/// Everything the spawned ledger writer needs to assemble one `llm_calls`
/// row. Owned by the response stream and consumed exactly once when the
/// stream finalizes (body complete or client disconnect).
pub(crate) struct LedgerContext {
    pub(crate) pool: sqlx::PgPool,
    pub(crate) org_id: Uuid,
    pub(crate) user_id: Uuid,
    pub(crate) credential_id: Option<Uuid>,
    pub(crate) auth_session_id: Option<Uuid>,
    pub(crate) client_session_id: Option<String>,
    pub(crate) repo_id: Option<Uuid>,
    pub(crate) branch: Option<String>,
    pub(crate) requested_model: Option<String>,
    pub(crate) provider_model: Option<String>,
    pub(crate) http_status: i32,
    pub(crate) outcome: &'static str,
    pub(crate) anthropic_request_id: Option<String>,
    pub(crate) path: String,
    pub(crate) start: Instant,
    /// Whether cost estimation is enabled (enterprise). When false, the ledger
    /// row's cost is recorded as $0 — cost analytics is enterprise-only.
    pub(crate) cost_enabled: bool,
}

impl LedgerContext {
    /// Assemble and insert the ledger row. Only successful calls carry
    /// usage/cost (derived from `parsed`); any non-success outcome records
    /// status/duration only. Never panics — an insert failure is logged.
    async fn write_row(self, parsed: Option<ParsedUsage>) {
        // Usage/cost only when the call succeeded AND we parsed a usage object.
        let usage = (self.outcome == "success").then_some(parsed).flatten();

        let UsageFields {
            response_model,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_write_tokens,
            stop_reason,
            total_tokens,
            estimated_cost_usd,
            anthropic_message_id,
        } = match usage {
            Some(u) => self.usage_fields(u).await,
            None => UsageFields::default(),
        };

        let rec = LlmCallRecord {
            org_id: self.org_id,
            user_id: self.user_id,
            credential_id: self.credential_id,
            auth_session_id: self.auth_session_id,
            client_session_id: self.client_session_id,
            repo_id: self.repo_id,
            branch: self.branch,
            requested_model: self.requested_model,
            provider_model: self.provider_model,
            response_model,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_write_tokens,
            total_tokens,
            estimated_cost_usd,
            stop_reason,
            http_status: self.http_status,
            outcome: self.outcome.to_string(),
            duration_ms: self.start.elapsed().as_millis() as i64,
            anthropic_request_id: self.anthropic_request_id,
            path: self.path,
            anthropic_message_id,
        };

        if let Err(e) = LlmCallRepo::insert(&self.pool, &rec).await {
            tracing::warn!(
                error = %e,
                request_id = ?rec.anthropic_request_id,
                "failed to write llm_calls ledger row"
            );
        }
    }

    /// Derive the response-model / token / cost fields from a parsed usage
    /// object, estimating cost from the resolved model's pricing. Token counts
    /// are stored verbatim; the cost-input copies are zero-filled.
    async fn usage_fields(&self, u: ParsedUsage) -> UsageFields {
        // Model for pricing: prefer what the response reported, then the
        // routed provider model, then what the client requested.
        let model = u
            .model
            .clone()
            .or_else(|| self.provider_model.clone())
            .or_else(|| self.requested_model.clone())
            .unwrap_or_else(|| "unknown".into());

        let input = u.input_tokens.unwrap_or(0);
        let output = u.output_tokens.unwrap_or(0);
        let cache_read = u.cache_read_tokens.unwrap_or(0);
        let cache_write = u.cache_write_tokens.unwrap_or(0);

        // Cost analytics is enterprise-only — under the community pricing
        // provider the ledger row's cost stays $0.
        let cost = if self.cost_enabled {
            let pricing = fetch_pricing_for_model(&self.pool, &model, None).await;
            estimate_cost_with_pricing(&pricing, input, output, cache_read, cache_write)
        } else {
            0.0
        };

        UsageFields {
            response_model: u.model,
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
            cache_read_tokens: u.cache_read_tokens,
            cache_write_tokens: u.cache_write_tokens,
            stop_reason: u.stop_reason,
            // total_tokens excludes cache, matching the hook/session convention
            // (sessions store total = input + output). Keeping the two sources
            // consistent is required for a correct mixed `both` analytics view.
            total_tokens: Some(input + output),
            estimated_cost_usd: Some(cost),
            anthropic_message_id: u.message_id,
        }
    }
}

/// The usage/cost-derived subset of a ledger row. Defaults to all-`None`
/// (used for error outcomes that carry no usage).
#[derive(Default)]
struct UsageFields {
    response_model: Option<String>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
    stop_reason: Option<String>,
    total_tokens: Option<i64>,
    estimated_cost_usd: Option<f64>,
    anthropic_message_id: Option<String>,
}

/// Stream wrapper that owns concurrency permits alongside the inner
/// `bytes_stream()`, taps each chunk into a `UsageCapture`, and writes one
/// `llm_calls` ledger row when the stream finalizes. Dropping the stream
/// (body complete or client disconnect) both drops the permits and triggers
/// the (at-most-once) ledger write.
pub(crate) struct CapturingStream<S> {
    pub(crate) inner: S,
    pub(crate) _permits: HeldPermits,
    pub(crate) capture: Option<UsageCapture>,
    pub(crate) ctx: Option<LedgerContext>,
}

impl<S> CapturingStream<S> {
    /// Run the ledger write at most once. Takes `ctx` + `capture` so a second
    /// call (e.g. Drop after a natural `Ready(None)`) is a no-op. The actual
    /// insert is spawned so it never blocks the response stream's task.
    fn finalize(&mut self) {
        let (Some(ctx), Some(capture)) = (self.ctx.take(), self.capture.take()) else {
            return;
        };
        let parsed = capture.finish();
        tokio::spawn(async move {
            ctx.write_row(parsed).await;
        });
    }
}

impl<S> futures_util::Stream for CapturingStream<S>
where
    S: futures_util::Stream<Item = reqwest::Result<Bytes>> + Unpin,
{
    type Item = reqwest::Result<Bytes>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match std::pin::Pin::new(&mut self.inner).poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(chunk))) => {
                if let Some(cap) = self.capture.as_mut() {
                    cap.feed(&chunk);
                }
                std::task::Poll::Ready(Some(Ok(chunk)))
            }
            std::task::Poll::Ready(Some(Err(e))) => {
                // A 2xx upstream whose body errors mid-stream must NOT be
                // recorded as a successful ledger row. Mark the outcome before
                // finalizing so the spawned writer sees the failure.
                if let Some(ctx) = self.ctx.as_mut() {
                    ctx.outcome = "stream_error";
                }
                self.finalize();
                std::task::Poll::Ready(Some(Err(e)))
            }
            std::task::Poll::Ready(None) => {
                self.finalize();
                std::task::Poll::Ready(None)
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<S> Drop for CapturingStream<S> {
    fn drop(&mut self) {
        // Catches client disconnect mid-stream: poll_next never reached
        // Ready(None), so finalize here. No-op if already finalized.
        //
        // Reaching Drop with `ctx` still present means we did NOT finalize via
        // a natural `Ready(None)` or a `stream_error` — the response body was
        // dropped before completing, i.e. the client disconnected mid-stream.
        // A 2xx left as "success" here would record partial/zero usage (the
        // final `message_delta` may never have arrived), so relabel it. The
        // "success"-gated usage in `write_row` then records status/duration
        // only, never misleading partial token counts.
        if let Some(ctx) = self.ctx.as_mut() {
            if ctx.outcome == "success" {
                ctx.outcome = "client_disconnect";
            }
        }
        self.finalize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;

    /// A stream dropped before reaching `Ready(None)` (client disconnect mid
    /// body) must NOT be recorded as `success`. The relabel happens in `Drop`.
    #[sqlx::test(migrations = "./migrations")]
    async fn client_disconnect_is_not_recorded_as_success(pool: sqlx::PgPool) {
        let org_id: Uuid =
            sqlx::query_scalar("INSERT INTO orgs (name) VALUES ('disc-test') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();
        let user_id: Uuid = sqlx::query_scalar(
            "INSERT INTO users (email, password_hash, name) VALUES ($1, 'h', 'n') RETURNING id",
        )
        .bind(format!("disc-{org_id}@test.test"))
        .fetch_one(&pool)
        .await
        .unwrap();

        let ctx = LedgerContext {
            pool: pool.clone(),
            org_id,
            user_id,
            credential_id: None,
            auth_session_id: None,
            client_session_id: None,
            repo_id: None,
            branch: None,
            requested_model: None,
            provider_model: None,
            http_status: 200,
            outcome: "success",
            anthropic_request_id: Some("req_disc".into()),
            path: "v1/messages".into(),
            start: Instant::now(),
            cost_enabled: true,
        };
        let permits = HeldPermits::new(
            std::sync::Arc::new(tokio::sync::Semaphore::new(1))
                .try_acquire_owned()
                .unwrap(),
            None,
        );
        // Construct the wrapper and drop it WITHOUT polling to Ready(None) —
        // models a client that disconnected mid-stream.
        let s = CapturingStream {
            inner: stream::empty::<reqwest::Result<Bytes>>(),
            _permits: permits,
            capture: Some(UsageCapture::new(Some("text/event-stream"))),
            ctx: Some(ctx),
        };
        drop(s);

        // The ledger write is spawned; poll until the row lands.
        let mut outcome = None;
        for _ in 0..100 {
            outcome = sqlx::query_scalar::<_, String>(
                "SELECT outcome FROM llm_calls WHERE anthropic_request_id = 'req_disc'",
            )
            .fetch_optional(&pool)
            .await
            .unwrap();
            if outcome.is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(
            outcome.as_deref(),
            Some("client_disconnect"),
            "a mid-stream disconnect must not be recorded as success"
        );
    }
}
