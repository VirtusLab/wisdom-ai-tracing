//! Generic, feature-agnostic extension seams. The OSS lib knows only these
//! *categories* of extension — never a specific feature (cost, compliance, …).
//! All collections are empty by default, so a stock OSS build wires nothing.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use uuid::Uuid;

use crate::AppState;

/// A self-describing metric contributed to a view payload. The lib does not
/// interpret `key`/`format` — it serializes them for the frontend to render.
#[derive(Debug, Clone, Serialize)]
pub struct Metric {
    pub key: String,
    pub label: String,
    pub value: serde_json::Value,
    pub format: String, // e.g. "usd", "count", "duration_ms"
}

/// Context handed to a metric contributor for a single session view.
#[derive(Debug, Clone)]
pub struct SessionMetricContext {
    pub org_id: Uuid,
    pub session_db_id: Uuid,
    pub model: Option<String>,
}

/// Context handed to ingest hooks once a session is finalized.
#[derive(Debug, Clone)]
pub struct SessionFinalizedContext {
    pub org_id: Uuid,
    pub repo_id: Uuid,
    pub user_id: Uuid,
    pub session_db_id: Uuid,
    pub model: Option<String>,
}

/// Contributes metrics to a named view slot (e.g. "session.detail").
#[async_trait]
pub trait MetricContributor: Send + Sync {
    fn slot(&self) -> &'static str;
    async fn contribute(&self, state: &AppState, ctx: &SessionMetricContext) -> Vec<Metric>;
}

/// Mounts additional HTTP routes. Returned router is merged into the app
/// before `.with_state(...)`, so it must be `Router<AppState>`.
pub trait RoutePlugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn router(&self) -> axum::Router<AppState>;
}

/// When a registered background task runs.
#[derive(Debug, Clone, Copy)]
pub enum Schedule {
    Startup,
    Interval(Duration),
}

/// Periodic or startup background work.
#[async_trait]
pub trait BackgroundTask: Send + Sync {
    fn name(&self) -> &'static str;
    fn schedule(&self) -> Schedule;
    async fn run(&self, state: AppState);
}

/// Observes a session once it has been finalized during ingest.
#[async_trait]
pub trait IngestHook: Send + Sync {
    async fn on_session_finalized(&self, state: &AppState, ctx: &SessionFinalizedContext);
}

/// The generic extension registry. Empty by default → stock OSS wires nothing.
#[derive(Default, Clone)]
pub struct Plugins {
    pub metrics: Vec<Arc<dyn MetricContributor>>,
    pub routes: Vec<Arc<dyn RoutePlugin>>,
    pub tasks: Vec<Arc<dyn BackgroundTask>>,
    pub ingest_hooks: Vec<Arc<dyn IngestHook>>,
    /// Capability keys advertised to the frontend (e.g. "cost"). Empty in OSS.
    pub capabilities: BTreeSet<String>,
}

impl Plugins {
    /// Collect metrics from all contributors registered for `slot`.
    pub async fn metrics_for(
        &self,
        slot: &str,
        state: &AppState,
        ctx: &SessionMetricContext,
    ) -> Vec<Metric> {
        let mut out = Vec::new();
        for c in self.metrics.iter().filter(|c| c.slot() == slot) {
            out.extend(c.contribute(state, ctx).await);
        }
        out
    }
}
