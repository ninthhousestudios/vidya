use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::db;
use crate::engine;
use crate::error::{Result, VidyaError};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeriveArgs {
    /// Domain slug
    pub domain: String,
    /// Operation type (e.g. "sandhi", "dignity")
    pub operation: String,
    /// Input for the derivation (domain-specific JSON)
    pub input: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct DeriveOutput {
    pub domain: String,
    pub operation: String,
    pub input: serde_json::Value,
    pub result: serde_json::Value,
    pub trace: Vec<engine::TraceStep>,
}

pub async fn handle(pool: &PgPool, args: DeriveArgs) -> Result<DeriveOutput> {
    let domain = db::get_domain_by_slug(pool, &args.domain)
        .await?
        .ok_or_else(|| VidyaError::NotFound {
            tool: "vidya_derive".into(),
            kind: format!("domain '{}'", args.domain),
        })?;

    let request = engine::DeriveRequest {
        domain_id: domain.id,
        domain_slug: args.domain.clone(),
        operation: args.operation.clone(),
        input: args.input.clone(),
    };

    let result = engine::derive(pool, request).await?;

    Ok(DeriveOutput {
        domain: args.domain,
        operation: args.operation,
        input: args.input,
        result: result.output,
        trace: result.trace,
    })
}
