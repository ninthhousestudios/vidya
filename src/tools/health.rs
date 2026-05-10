use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::error::Result;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct HealthArgs {}

#[derive(Debug, Serialize)]
pub struct HealthOutput {
    pub status: &'static str,
    pub db_connected: bool,
    pub domain_count: i64,
    pub claim_count: i64,
    pub version: &'static str,
}

pub async fn handle(pool: &PgPool) -> Result<HealthOutput> {
    let db_connected = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool)
        .await
        .is_ok();

    let domain_count = if db_connected {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM domains")
            .fetch_one(pool)
            .await
            .unwrap_or(0)
    } else {
        0
    };

    let claim_count = if db_connected {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM claims")
            .fetch_one(pool)
            .await
            .unwrap_or(0)
    } else {
        0
    };

    Ok(HealthOutput {
        status: if db_connected { "ok" } else { "degraded" },
        db_connected,
        domain_count,
        claim_count,
        version: env!("CARGO_PKG_VERSION"),
    })
}
