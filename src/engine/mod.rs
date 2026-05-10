mod sandhi;

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Result, VidyaError};

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
