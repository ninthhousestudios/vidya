use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::db;
use crate::engine::{self, Engine};
use crate::error::{Result, VidyaError};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnalyzeArgs {
    /// Domain slug
    pub domain: String,
    /// Operation type (e.g. "sandhi")
    pub operation: String,
    /// Input for analysis (domain-specific JSON, e.g. {"form": "ā"})
    pub input: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct AnalyzeOutput {
    pub domain: String,
    pub operation: String,
    pub input: serde_json::Value,
    pub candidates: Vec<engine::AnalysisCandidate>,
}

pub async fn handle(pool: &PgPool, args: AnalyzeArgs) -> Result<AnalyzeOutput> {
    let engine = Engine::new();
    handle_with_engine(pool, &engine, args).await
}

pub async fn handle_with_engine(pool: &PgPool, engine: &Engine, args: AnalyzeArgs) -> Result<AnalyzeOutput> {
    let domain = db::get_domain_by_slug(pool, &args.domain)
        .await?
        .ok_or_else(|| VidyaError::NotFound {
            tool: "vidya_analyze".into(),
            kind: format!("domain '{}'", args.domain),
        })?;

    let request = engine::AnalyzeRequest {
        domain_id: domain.id,
        domain_slug: args.domain.clone(),
        operation: args.operation.clone(),
        input: args.input.clone(),
    };

    let candidates = engine.analyze(pool, request).await?;

    Ok(AnalyzeOutput {
        domain: args.domain,
        operation: args.operation,
        input: args.input,
        candidates,
    })
}
