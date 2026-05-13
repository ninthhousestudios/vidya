mod sandhi;

use std::future::Future;
use std::pin::Pin;

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

pub struct Engine {
    strategies: Vec<Box<dyn EngineStrategy>>,
}

impl Engine {
    pub fn new() -> Self {
        let strategies: Vec<Box<dyn EngineStrategy>> = vec![
            Box::new(sandhi::VyakaranaSandhiStrategy),
        ];
        Self { strategies }
    }

    pub async fn derive(&self, pool: &PgPool, request: DeriveRequest) -> Result<DeriveResult> {
        for strategy in &self.strategies {
            if strategy.can_handle(&request.domain_slug, &request.operation) {
                return strategy.derive(pool, &request).await;
            }
        }
        Err(VidyaError::InvalidArgument {
            tool: "vidya_derive".into(),
            argument: "domain/operation".into(),
            constraint: "no strategy registered for this domain/operation combination".into(),
            received: format!("{}/{}", request.domain_slug, request.operation),
        })
    }

    pub async fn analyze(
        &self,
        pool: &PgPool,
        request: AnalyzeRequest,
    ) -> Result<Vec<AnalysisCandidate>> {
        for strategy in &self.strategies {
            if strategy.can_handle(&request.domain_slug, &request.operation) {
                return strategy.analyze(pool, &request).await;
            }
        }
        Err(VidyaError::InvalidArgument {
            tool: "vidya_analyze".into(),
            argument: "domain/operation".into(),
            constraint: "no strategy registered for this domain/operation combination".into(),
            received: format!("{}/{}", request.domain_slug, request.operation),
        })
    }
}
