//! Transparent Anthropic API proxy (issue softwaremill/tracevault#207,
//! parent #181).
//!
//! Mounted as a catch-all at `/proxy/anthropic/{*path}`. Clients point their
//! tool's `ANTHROPIC_BASE_URL` at `<tv-server>/proxy/anthropic` and use their
//! TV `auth_sessions` token as the `x-api-key` value. The handler:
//!
//! 1. Resolves the TV session token in `x-api-key` to a user.
//! 2. Resolves that user's default routing rule to a `credentials` row,
//!    decrypts its key, and substitutes it into `x-api-key`.
//! 3. Forwards the request to the credential's `base_url`/{path} with an
//!    allow-listed set of headers.
//! 4. Streams the response body back byte-for-byte via
//!    `reqwest::Response::bytes_stream()` — no SSE parsing.
//!
//! Proxy-originated errors use the Anthropic error envelope shape so that
//! unmodified Anthropic clients surface them through their existing error
//! paths. Upstream errors are passed through verbatim.
//!
//! Explicitly **not** in this slice: event capture, model routing,
//! organization-level keys, OpenAI support, dedicated long-lived proxy
//! tokens.

use axum::{
    body::{Body, Bytes},
    extract::{OriginalUri, Path, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::time::Instant;
use uuid::Uuid;

use crate::auth::sha256_hex;
use crate::encryption;
use crate::repo::credentials::CredentialRepo;
use crate::AppState;

/// Default Anthropic API base URL. Seeds `AppState.default_credential_base_url`
/// (the base applied to newly-stored credentials that don't specify one).
pub const DEFAULT_ANTHROPIC_UPSTREAM_BASE: &str = "https://api.anthropic.com";

/// Request headers we forward upstream. Anything not on this list is dropped
/// — including `host` (reqwest sets it correctly), `authorization`, `cookie`,
/// `x-api-key` (we inject the decrypted key), `x-forwarded-*`, `via`, and
/// hop-by-hop headers. Allow-list is more conservative than a deny-list and
/// fails closed when new client-side headers appear.
const FORWARDED_REQUEST_HEADERS: &[&str] = &[
    "accept",
    "accept-encoding",
    "anthropic-beta",
    "anthropic-dangerous-direct-browser-access",
    "anthropic-version",
    "content-type",
    "user-agent",
];

/// Response headers we forward downstream. We always forward all
/// `anthropic-*` headers (forward compat with new headers like
/// `anthropic-organization-id` or billing). Hop-by-hop headers
/// (`transfer-encoding`, `connection`, `content-length`) are dropped so that
/// Axum / hyper can re-frame the body correctly for the downstream client.
const FORWARDED_RESPONSE_HEADERS: &[&str] = &[
    "cache-control",
    "content-type",
    "content-encoding",
    "request-id",
];

/// `error.type` discriminants used in the Anthropic-shaped error envelope.
/// Mirrors the documented Anthropic API error types so unmodified clients
/// route these the same way they'd route a real api.anthropic.com error.
//
// All variants share the `*Error` suffix to mirror Anthropic's wire
// vocabulary (the `error.type` JSON field).
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy)]
enum ProxyErrorKind {
    AuthenticationError,
    ApiError,
    /// Mirrors Anthropic's `overloaded_error` — agents already back off
    /// gracefully on this `type` value, so reusing it for our internal
    /// concurrency caps keeps client behavior identical to a real upstream
    /// overload.
    OverloadedError,
}

impl ProxyErrorKind {
    fn as_str(self) -> &'static str {
        match self {
            ProxyErrorKind::AuthenticationError => "authentication_error",
            ProxyErrorKind::ApiError => "api_error",
            ProxyErrorKind::OverloadedError => "overloaded_error",
        }
    }
}

fn anthropic_error(status: StatusCode, kind: ProxyErrorKind, message: &str) -> Response {
    (
        status,
        Json(json!({
            "type": "error",
            "error": {
                "type": kind.as_str(),
                "message": message,
            }
        })),
    )
        .into_response()
}

/// Replace the top-level `model` field in a JSON request body with the
/// routing rule's provider-side model name, re-serializing the result.
/// Returns None if the body is not a JSON object (the caller keeps the
/// original bytes), so a non-JSON body or one without an object root is
/// forwarded verbatim rather than dropped or mangled.
fn rewrite_model(body: &Bytes, provider_model: &str) -> Option<Bytes> {
    let mut v: serde_json::Value = serde_json::from_slice(body).ok()?;
    let obj = v.as_object_mut()?;
    obj.insert(
        "model".into(),
        serde_json::Value::String(provider_model.to_string()),
    );
    Some(Bytes::from(serde_json::to_vec(&v).ok()?))
}

/// Catch-all proxy handler. Mounted at `/proxy/anthropic/{*path}`.
///
/// Path layout: `path` is everything after `/proxy/anthropic/` (no leading
/// slash). Query string is forwarded verbatim from the original URI.
///
/// This is a thin orchestration shell: it sequences three concerns that
/// live in their own private functions so the responsibilities are easy
/// to audit independently:
///
///   1. `authenticate` — resolve `x-api-key` to a user_id and load the
///      user's decrypted upstream credential.
///   2. `forward_to_upstream` — construct the upstream request (URL,
///      header allow-list, key injection) and dispatch it.
///   3. `build_downstream_response` — stream the upstream body back to
///      the client with response-header forwarding.
pub async fn anthropic_proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    OriginalUri(original_uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let start = Instant::now();

    // Defense in depth: reject `..` segments in the captured path before
    // composing the upstream URL. `reqwest`/`url` normalize `..` before
    // sending, so today this only collapses paths within api.anthropic.com
    // (no host escape is possible). But the credential `base_url` is a
    // user-configurable string — if it ever carries a path prefix (e.g. a
    // future Anthropic regional endpoint with `/v1/` baked in), `..` could
    // escape that prefix. Rejecting at the entry point keeps this safe
    // regardless of how the base URL is configured later.
    if path.split(['/', '\\']).any(|seg| seg == "..") {
        tracing::warn!(
            error_type = "api_error",
            reason = "path_traversal_segment",
            path = %path,
            "proxy rejected path containing '..'"
        );
        return anthropic_error(
            StatusCode::BAD_REQUEST,
            ProxyErrorKind::ApiError,
            "Invalid path",
        );
    }

    // Best-effort parse of the request `model` from the JSON body. A non-JSON
    // body or a missing `model` field yields None, which routes to the user's
    // default credential — the parse never fails the request.
    let requested_model = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(str::to_string));

    let (user_id, upstream_key, max_concurrent, base_url, protocol, provider_model) =
        match authenticate(&state, &headers, &path, requested_model.as_deref()).await {
            Ok(tuple) => tuple,
            Err(resp) => return resp,
        };

    // If the matched routing rule rewrites the model, patch the body's
    // top-level `model` before forwarding. On parse failure we keep the
    // original bytes (a body that survived as JSON above will re-serialize
    // fine; this is purely defensive).
    let body = if let Some(pm) = provider_model.as_deref() {
        rewrite_model(&body, pm).unwrap_or(body)
    } else {
        body
    };

    // Acquire concurrency permits BEFORE dispatching upstream. Global cap
    // first, then per-credential — see HeldPermits / build_downstream_response
    // for why permits travel with the response stream rather than living
    // as locals.
    let global_permit = match try_acquire_global_permit(&state, user_id, &path) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let credential_permit =
        match try_acquire_credential_permit(&state, user_id, max_concurrent, &path) {
            Ok(p) => p,
            Err(resp) => return resp,
        };
    let permits = HeldPermits {
        _credential: credential_permit,
        _global: global_permit,
    };

    let upstream_resp = match forward_to_upstream(
        &state,
        &method,
        &path,
        original_uri.query().unwrap_or(""),
        &headers,
        body,
        &upstream_key,
        &base_url,
        &protocol,
        user_id,
        start,
    )
    .await
    {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    let upstream_status = upstream_resp.status();
    tracing::info!(
        user_id = %user_id,
        path = %path,
        upstream_status = upstream_status.as_u16(),
        duration_ms = start.elapsed().as_millis() as u64,
        "proxied request"
    );

    build_downstream_response(upstream_resp, permits)
}

/// Concern 1: extract `x-api-key`, resolve it to a user, and load that
/// user's decrypted Anthropic credential — selected by the request `model`
/// (an exact-match routing rule wins; otherwise the user's default rule).
/// Returns the
/// `(user_id, upstream_plaintext_key, max_concurrent, base_url, protocol, provider_model)`
/// tuple on success, or an Anthropic-shaped error envelope on any
/// auth/credential failure. `provider_model` is Some when the matched rule
/// rewrites the model to a provider-side name.
async fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
    path: &str,
    model: Option<&str>,
) -> Result<(Uuid, String, i32, String, String, Option<String>), Response> {
    let tv_token = match headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        Some(t) if !t.is_empty() => t,
        _ => {
            tracing::warn!(
                error_type = "authentication_error",
                reason = "missing_x_api_key",
                path = %path,
                "proxy auth failed"
            );
            return Err(anthropic_error(
                StatusCode::UNAUTHORIZED,
                ProxyErrorKind::AuthenticationError,
                "Missing x-api-key header",
            ));
        }
    };

    let token_hash = sha256_hex(tv_token);
    let user_id = resolve_token(state, &token_hash).await?;
    let (upstream_key, max_concurrent, base_url, protocol, provider_model) =
        load_credential(state, user_id, model).await?;
    Ok((
        user_id,
        upstream_key,
        max_concurrent,
        base_url,
        protocol,
        provider_model,
    ))
}

/// Concern 2: build the upstream request from the user's downstream
/// request — URL composition, header allow-list, decrypted-key injection —
/// then dispatch it.
#[allow(clippy::too_many_arguments)]
async fn forward_to_upstream(
    state: &AppState,
    method: &Method,
    path: &str,
    query: &str,
    headers: &HeaderMap,
    body: Bytes,
    upstream_key: &str,
    base_url: &str,
    protocol: &str,
    user_id: Uuid,
    start: Instant,
) -> Result<reqwest::Response, Response> {
    let base = base_url.trim_end_matches('/');
    let upstream_url = if query.is_empty() {
        format!("{base}/{path}")
    } else {
        format!("{base}/{path}?{query}")
    };

    let mut upstream_req = state
        .proxy_http_client
        .request(method.clone(), &upstream_url)
        .body(body);

    for header_name in FORWARDED_REQUEST_HEADERS {
        if let Some(value) = headers.get(*header_name) {
            upstream_req = upstream_req.header(*header_name, value);
        }
    }
    // Inject the decrypted upstream key per the credential's protocol. Done
    // after the allow-list loop so a client-sent auth header cannot bleed
    // through even if the allow-list is ever broadened by mistake.
    match protocol {
        "anthropic" => {
            upstream_req = upstream_req.header("x-api-key", upstream_key);
        }
        other => {
            tracing::error!(
                user_id = %user_id,
                protocol = %other,
                "unsupported credential protocol reached forward path"
            );
            return Err(anthropic_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ProxyErrorKind::ApiError,
                "Unsupported credential protocol",
            ));
        }
    }

    upstream_req.send().await.map_err(|e| {
        tracing::warn!(
            user_id = %user_id,
            path = %path,
            error_type = "api_error",
            duration_ms = start.elapsed().as_millis() as u64,
            err = %e,
            "upstream request to Anthropic failed"
        );
        anthropic_error(
            StatusCode::BAD_GATEWAY,
            ProxyErrorKind::ApiError,
            "Upstream Anthropic API unreachable",
        )
    })
}

/// Concern 3: turn the upstream `reqwest::Response` into an axum
/// `Response` — copies status + allow-listed response headers and streams
/// the body byte-for-byte via `bytes_stream()` so SSE responses pass
/// through without buffering.
///
/// `permits` carries any concurrency permits acquired earlier in the
/// handler. We attach them to the response stream so they are dropped
/// only when the *streaming body* finishes — not when this function
/// returns. Otherwise SSE streams would release capacity the moment the
/// upstream's headers came back, allowing far more concurrent in-flight
/// upstream connections than the cap allows.
fn build_downstream_response(upstream_resp: reqwest::Response, permits: HeldPermits) -> Response {
    let upstream_status = upstream_resp.status();
    let upstream_headers = upstream_resp.headers().clone();
    let body_stream = PermitHoldingStream {
        inner: upstream_resp.bytes_stream(),
        _permits: permits,
    };

    let mut downstream = Response::builder().status(upstream_status);
    if let Some(hdrs) = downstream.headers_mut() {
        copy_response_headers(&upstream_headers, hdrs);
    }

    downstream
        .body(Body::from_stream(body_stream))
        .unwrap_or_else(|e| {
            tracing::error!(err = %e, "failed to build downstream response");
            anthropic_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ProxyErrorKind::ApiError,
                "Failed to construct downstream response",
            )
        })
}

/// Bundle of concurrency permits that must be held for the lifetime of a
/// proxy response (including its streaming body). Permits are released
/// in field-declaration order on drop, so the per-credential permit
/// releases before the global one — the inverse of acquisition order.
struct HeldPermits {
    _credential: tokio::sync::OwnedSemaphorePermit,
    _global: Option<tokio::sync::OwnedSemaphorePermit>,
}

/// Stream wrapper that owns concurrency permits alongside the inner
/// `bytes_stream()`. Dropping the stream (including via the response
/// body completing or the client disconnecting) drops the permits.
struct PermitHoldingStream<S> {
    inner: S,
    _permits: HeldPermits,
}

impl<S> futures_util::Stream for PermitHoldingStream<S>
where
    S: futures_util::Stream + Unpin,
{
    type Item = S::Item;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::pin::Pin::new(&mut self.inner).poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// Resolve a sha256'd TV token to a user_id. Returns:
///   - Ok(user_id) when the token is a valid, non-expired `auth_sessions` row
///   - Err(401 envelope) when the token is missing or matches an org
///     `api_keys` row (the proxy is per-user; org-scoped api_keys have no
///     user context)
///   - Err(401 envelope) when the token does not match anything
///   - Err(502 envelope) on database error so unmodified clients route it
///     through their existing "upstream error" path
async fn resolve_token(state: &AppState, token_hash: &str) -> Result<Uuid, Response> {
    // Try auth_sessions first (the user-session path).
    let session_row = sqlx::query_as::<_, (Uuid,)>(
        "SELECT user_id FROM auth_sessions WHERE token_hash = $1 AND expires_at > NOW()",
    )
    .bind(token_hash)
    .fetch_optional(&state.pool)
    .await;

    match session_row {
        Ok(Some((user_id,))) => return Ok(user_id),
        Err(e) => {
            tracing::warn!(error_type = "api_error", err = %e, "auth_sessions lookup failed");
            return Err(anthropic_error(
                StatusCode::BAD_GATEWAY,
                ProxyErrorKind::ApiError,
                "Upstream Anthropic API unreachable",
            ));
        }
        Ok(None) => { /* fall through to api_keys check for a clearer error */ }
    }

    // Fall back to api_keys so we can give a precise error message when the
    // user accidentally pastes an org-scoped ingestion API key.
    let api_key_row =
        sqlx::query_scalar::<_, Uuid>("SELECT org_id FROM api_keys WHERE key_hash = $1")
            .bind(token_hash)
            .fetch_optional(&state.pool)
            .await;

    match api_key_row {
        Ok(Some(_)) => {
            tracing::warn!(
                error_type = "authentication_error",
                reason = "org_api_key_used",
                "proxy auth failed"
            );
            Err(anthropic_error(
                StatusCode::UNAUTHORIZED,
                ProxyErrorKind::AuthenticationError,
                "Proxy requires a user session token, not an org API key",
            ))
        }
        Ok(None) => {
            tracing::warn!(
                error_type = "authentication_error",
                reason = "unknown_token",
                "proxy auth failed"
            );
            Err(anthropic_error(
                StatusCode::UNAUTHORIZED,
                ProxyErrorKind::AuthenticationError,
                "Invalid or expired TraceVault session token",
            ))
        }
        Err(e) => {
            tracing::warn!(error_type = "api_error", err = %e, "api_keys lookup failed");
            Err(anthropic_error(
                StatusCode::BAD_GATEWAY,
                ProxyErrorKind::ApiError,
                "Upstream Anthropic API unreachable",
            ))
        }
    }
}

/// Fetch the user's stored credential (encrypted Anthropic key + cap +
/// upstream) for the request `model` and decrypt the key. An exact-match
/// routing rule selects the credential; otherwise the user's default rule
/// applies. Returns the
/// `(plaintext_key, max_concurrent, base_url, protocol, provider_model)`
/// tuple on success or an Anthropic-shaped error envelope on any failure (no
/// key configured, no master key on this server, ciphertext corrupted, DB
/// error). `provider_model` is Some when the matched rule rewrites the model.
async fn load_credential(
    state: &AppState,
    user_id: Uuid,
    model: Option<&str>,
) -> Result<(String, i32, String, String, Option<String>), Response> {
    let credential = match CredentialRepo::resolve_for_model(&state.pool, user_id, model).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::warn!(
                user_id = %user_id,
                error_type = "authentication_error",
                reason = "no_anthropic_key_configured",
                "proxy auth failed"
            );
            return Err(anthropic_error(
                StatusCode::UNAUTHORIZED,
                ProxyErrorKind::AuthenticationError,
                "No Anthropic API key configured — set one at /me/proxy",
            ));
        }
        Err(e) => {
            tracing::warn!(
                user_id = %user_id,
                error_type = "api_error",
                err = %e,
                "failed to load credential row"
            );
            return Err(anthropic_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ProxyErrorKind::ApiError,
                "Failed to load upstream credentials",
            ));
        }
    };

    let master_key = state.encryption_key.as_deref().ok_or_else(|| {
        tracing::error!(
            user_id = %user_id,
            error_type = "api_error",
            "server has no encryption_key configured but a credential row exists"
        );
        anthropic_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ProxyErrorKind::ApiError,
            "Server is not configured with an encryption key",
        )
    })?;

    let plaintext = encryption::decrypt(&credential.encrypted, &credential.nonce, master_key)
        .map_err(|e| {
            tracing::error!(
                user_id = %user_id,
                error_type = "api_error",
                err = %e,
                "failed to decrypt stored Anthropic key"
            );
            anthropic_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ProxyErrorKind::ApiError,
                "Failed to decrypt upstream credentials",
            )
        })?;
    Ok((
        plaintext,
        credential.max_concurrent,
        credential.base_url,
        credential.protocol,
        credential.provider_model,
    ))
}

/// Try to acquire a permit from the optional global concurrency cap. If
/// no global cap is configured, returns `Ok(None)` so the per-credential
/// cap is the only gate. On capacity exhaustion returns an
/// Anthropic-shaped 429 with `overloaded_error`.
//
// `Result<_, Response>` is the established error-return shape in this
// module (see `authenticate`, `forward_to_upstream`).
#[allow(clippy::result_large_err)]
fn try_acquire_global_permit(
    state: &AppState,
    user_id: Uuid,
    path: &str,
) -> Result<Option<tokio::sync::OwnedSemaphorePermit>, Response> {
    let Some(sem) = state.proxy_global_semaphore.as_ref() else {
        return Ok(None);
    };
    match sem.clone().try_acquire_owned() {
        Ok(p) => Ok(Some(p)),
        Err(_) => {
            tracing::warn!(
                user_id = %user_id,
                error_type = "overloaded_error",
                reason = "global_cap",
                path = %path,
                "proxy rejected request: global concurrency cap reached"
            );
            Err(anthropic_error(
                StatusCode::TOO_MANY_REQUESTS,
                ProxyErrorKind::OverloadedError,
                "Server is at capacity. Retry shortly.",
            ))
        }
    }
}

/// Try to acquire a permit from the per-credential concurrency cap.
/// Lazily creates the semaphore on first use, sized to `max_concurrent`.
/// On capacity exhaustion returns an Anthropic-shaped 429 with
/// `overloaded_error` and a message naming the configured cap so the user
/// can debug it from their `/me/proxy/` UI.
#[allow(clippy::result_large_err)]
fn try_acquire_credential_permit(
    state: &AppState,
    user_id: Uuid,
    max_concurrent: i32,
    path: &str,
) -> Result<tokio::sync::OwnedSemaphorePermit, Response> {
    // i32 -> usize is safe because the DB CHECK constraint clamps to (0, 256].
    // Defensive clamp at the lower end protects against an out-of-spec row.
    let cap = max_concurrent.max(1) as usize;

    // Look up or insert the per-credential semaphore. The DashMap entry
    // guard is held only across the `.clone()` of the Arc — never across
    // the .await/.acquire — so there is no chance of a guard living across
    // a yield point or self-deadlocking on the same shard.
    let sem = state
        .proxy_per_credential_semaphores
        .entry(user_id)
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Semaphore::new(cap)))
        .clone();

    match sem.try_acquire_owned() {
        Ok(p) => Ok(p),
        Err(_) => {
            tracing::warn!(
                user_id = %user_id,
                error_type = "overloaded_error",
                reason = "per_credential_cap",
                cap_value = max_concurrent,
                path = %path,
                "proxy rejected request: per-credential concurrency cap reached"
            );
            Err(anthropic_error(
                StatusCode::TOO_MANY_REQUESTS,
                ProxyErrorKind::OverloadedError,
                &format!(
                    "Too many concurrent requests against this credential (cap: {max_concurrent}). Retry shortly."
                ),
            ))
        }
    }
}

/// Copy allow-listed and `anthropic-*` headers from `src` into `dst`.
///
/// `reqwest::HeaderMap` and `axum`/`http`'s `HeaderMap` share the same
/// underlying types from the `http` crate, so we can clone names and values
/// directly without round-tripping through `from_bytes` (which would
/// re-validate already-valid headers and silently drop them on the unlikely
/// failure path).
fn copy_response_headers(src: &reqwest::header::HeaderMap, dst: &mut HeaderMap) {
    for (name, value) in src.iter() {
        // Header names from `http::HeaderName` are already lowercase by
        // construction, so a plain `starts_with` is sufficient and avoids
        // an allocation per response header.
        let name_str = name.as_str();
        let allow = FORWARDED_RESPONSE_HEADERS
            .iter()
            .any(|h| h.eq_ignore_ascii_case(name_str))
            || name_str.starts_with("anthropic-");
        if !allow {
            continue;
        }
        dst.insert(name.clone(), value.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_list_forwards_expected_request_headers() {
        for h in [
            "content-type",
            "accept",
            "anthropic-version",
            "anthropic-beta",
            "user-agent",
        ] {
            assert!(
                FORWARDED_REQUEST_HEADERS
                    .iter()
                    .any(|x| x.eq_ignore_ascii_case(h)),
                "expected {h} to be in the request allow-list"
            );
        }
    }

    #[test]
    fn allow_list_excludes_dangerous_request_headers() {
        for h in [
            "host",
            "authorization",
            "cookie",
            "x-api-key",
            "x-forwarded-for",
            "x-forwarded-proto",
            "x-real-ip",
            "via",
            "transfer-encoding",
            "content-length",
        ] {
            assert!(
                !FORWARDED_REQUEST_HEADERS
                    .iter()
                    .any(|x| x.eq_ignore_ascii_case(h)),
                "{h} must not be in the request allow-list"
            );
        }
    }

    #[test]
    fn copy_response_headers_forwards_allow_list_and_anthropic_star() {
        let mut src = reqwest::header::HeaderMap::new();
        src.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        src.insert(
            reqwest::header::HeaderName::from_static("anthropic-request-id"),
            reqwest::header::HeaderValue::from_static("req_abc123"),
        );
        src.insert(
            reqwest::header::HeaderName::from_static("anthropic-organization-id"),
            reqwest::header::HeaderValue::from_static("org_xyz"),
        );
        src.insert(
            reqwest::header::HeaderName::from_static("x-internal-secret"),
            reqwest::header::HeaderValue::from_static("must-not-leak"),
        );
        src.insert(
            reqwest::header::HeaderName::from_static("set-cookie"),
            reqwest::header::HeaderValue::from_static("session=must-not-leak"),
        );

        let mut dst = HeaderMap::new();
        copy_response_headers(&src, &mut dst);

        assert_eq!(
            dst.get("content-type").and_then(|v| v.to_str().ok()),
            Some("application/json")
        );
        assert_eq!(
            dst.get("anthropic-request-id")
                .and_then(|v| v.to_str().ok()),
            Some("req_abc123")
        );
        // Forward-compat: anthropic-* headers we have not heard of must
        // still pass through so future Anthropic features keep working.
        assert_eq!(
            dst.get("anthropic-organization-id")
                .and_then(|v| v.to_str().ok()),
            Some("org_xyz")
        );
        assert!(
            dst.get("x-internal-secret").is_none(),
            "non-allow-listed header must not be forwarded"
        );
        assert!(
            dst.get("set-cookie").is_none(),
            "set-cookie must never leak downstream"
        );
    }

    #[test]
    fn proxy_error_kind_strings_match_anthropic_vocabulary() {
        assert_eq!(
            ProxyErrorKind::AuthenticationError.as_str(),
            "authentication_error"
        );
        assert_eq!(ProxyErrorKind::ApiError.as_str(), "api_error");
        assert_eq!(ProxyErrorKind::OverloadedError.as_str(), "overloaded_error");
    }
}
