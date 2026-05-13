mod sandhi;

use std::future::Future;
use std::pin::Pin;

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Result, VidyaError};

// -- Request/response types --

#[derive(Debug)]
pub struct DeriveRequest {
    pub domain_id: Uuid,
    pub domain_slug: String,
    pub operation: String,
    pub input: serde_json::Value,
}

#[derive(Debug)]
pub struct DeriveResult {
    pub output: serde_json::Value,
    pub trace: Vec<TraceStep>,
}

#[derive(Debug, Serialize)]
pub struct TraceStep {
    pub step: usize,
    pub rule: String,
    pub rule_ref: Option<String>,
    pub input_state: String,
    pub output_state: String,
}

#[derive(Debug)]
pub struct AnalyzeRequest {
    pub domain_id: Uuid,
    pub domain_slug: String,
    pub operation: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct AnalysisCandidate {
    pub decomposition: serde_json::Value,
    pub rule: String,
    pub rule_ref: Option<String>,
    pub specificity: f64,
}

// -- Strategy trait --

pub trait EngineStrategy: Send + Sync {
    fn can_handle(&self, domain: &str, operation: &str) -> bool;

    fn derive<'a>(
        &'a self,
        pool: &'a PgPool,
        request: &'a DeriveRequest,
    ) -> Pin<Box<dyn Future<Output = Result<DeriveResult>> + Send + 'a>>;

    fn analyze<'a>(
        &'a self,
        pool: &'a PgPool,
        request: &'a AnalyzeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<AnalysisCandidate>>> + Send + 'a>>;
}

// -- Dispatch (hardcoded until strategies are registered) --

pub async fn derive(pool: &PgPool, request: DeriveRequest) -> Result<DeriveResult> {
    match request.operation.as_str() {
        "sandhi" => sandhi::derive_sandhi(pool, &request).await,
        other => Err(VidyaError::InvalidArgument {
            tool: "vidya_derive".into(),
            argument: "operation".into(),
            constraint: "supported operations: sandhi".into(),
            received: other.into(),
        }),
    }
}
