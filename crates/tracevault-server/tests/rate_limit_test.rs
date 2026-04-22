//! Exercises `tower_governor` middleware end-to-end: attaches a GovernorLayer
//! to a tiny router and verifies that exceeding the burst size yields 429.
//! Pins the middleware wiring so 0.6 → 0.8 upgrades don't silently break it.

use axum::{routing::get, Router};
use http::{Request, StatusCode};
use tower::util::ServiceExt;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};

fn build_router() -> Router {
    // SmartIpKeyExtractor reads X-Forwarded-For / X-Real-IP before falling
    // back to the peer socket; the latter isn't available under tower's
    // oneshot, so this is how we drive the per-IP bucket in tests.
    let config = GovernorConfigBuilder::default()
        .per_second(1)
        .burst_size(3)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .expect("failed to build governor config");

    Router::new()
        .route("/ping", get(|| async { "pong" }))
        .layer(GovernorLayer::new(config))
}

fn req_with_ip(ip: &str) -> Request<axum::body::Body> {
    Request::builder()
        .uri("/ping")
        .header("x-forwarded-for", ip)
        .body(axum::body::Body::empty())
        .unwrap()
}

#[tokio::test]
async fn bursts_within_limit_succeed() {
    let app = build_router();

    for _ in 0..3 {
        let resp = app.clone().oneshot(req_with_ip("10.0.0.1")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn exceeding_burst_returns_429() {
    let app = build_router();

    // Burn through the burst
    for _ in 0..3 {
        let _ = app.clone().oneshot(req_with_ip("10.0.0.2")).await.unwrap();
    }
    // Next request from same IP should be throttled
    let resp = app.clone().oneshot(req_with_ip("10.0.0.2")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn different_ips_have_independent_buckets() {
    let app = build_router();

    // Burn through IP A's burst
    for _ in 0..3 {
        let _ = app.clone().oneshot(req_with_ip("10.0.0.3")).await.unwrap();
    }

    // IP B should still have its full allowance
    let resp = app.clone().oneshot(req_with_ip("10.0.0.4")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
