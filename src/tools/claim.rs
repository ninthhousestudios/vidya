use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::db;
use crate::error::{Result, VidyaError};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClaimArgs {
    /// Action: "create", "get", or "list"
    pub action: String,
    /// Domain slug
    pub domain: String,
    /// Claim template slug (required for create, optional filter for list)
    pub template: Option<String>,
    /// Claim parameters as structured JSON
    pub params: Option<serde_json::Value>,
    /// Human-readable statement (required for create)
    pub statement: Option<String>,
    /// Status: "proposed", "active", or "historical" (default: "active")
    pub status: Option<String>,
    /// Tradition name (optional, creates assertion if provided with source)
    pub tradition: Option<String>,
    /// Source reference (optional, creates assertion if provided with tradition)
    pub source_ref: Option<String>,
    /// Source kind: "text", "practitioner", "derivation", "oral" (default: "text")
    pub source_kind: Option<String>,
    /// Pramana: "pratyaksha", "anumana", "shabda", "upamana", "arthapatti", "anupalabdhi" (default: "shabda")
    pub pramana: Option<String>,
    /// Confidence 0.0-1.0 (default: 1.0)
    pub confidence: Option<f32>,
    /// Claim ID for get action
    pub id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ClaimOutput {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim: Option<db::ClaimRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assertion: Option<db::AssertionRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claims: Option<Vec<db::ClaimRow>>,
}

pub async fn handle(pool: &PgPool, args: ClaimArgs) -> Result<ClaimOutput> {
    let domain = db::get_domain_by_slug(pool, &args.domain)
        .await?
        .ok_or_else(|| VidyaError::NotFound {
            tool: "vidya_claim".into(),
            kind: format!("domain '{}'", args.domain),
        })?;

    match args.action.as_str() {
        "create" => {
            let template_slug = args.template.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_claim".into(),
                argument: "template".into(),
                constraint: "required for create".into(),
                received: "null".into(),
            })?;
            let statement = args.statement.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_claim".into(),
                argument: "statement".into(),
                constraint: "required for create".into(),
                received: "null".into(),
            })?;
            let template = db::get_claim_template(pool, domain.id, &template_slug)
                .await?
                .ok_or_else(|| VidyaError::NotFound {
                    tool: "vidya_claim".into(),
                    kind: format!("claim_template '{template_slug}'"),
                })?;
            let params = args.params.unwrap_or(serde_json::json!({}));
            let status = args.status.as_deref().unwrap_or("active");
            let claim =
                db::insert_claim(pool, domain.id, template.id, params, status, &statement).await?;

            let assertion = if let (Some(tradition_name), Some(source_ref)) =
                (args.tradition, args.source_ref)
            {
                let tradition = db::upsert_tradition(pool, domain.id, &tradition_name, None).await?;
                let source_kind = args.source_kind.as_deref().unwrap_or("text");
                let source = db::insert_source(pool, source_kind, &source_ref, None).await?;
                let pramana = args.pramana.as_deref().unwrap_or("shabda");
                let confidence = args.confidence.unwrap_or(1.0);
                let a = db::insert_assertion(
                    pool,
                    claim.id,
                    tradition.id,
                    source.id,
                    pramana,
                    confidence,
                )
                .await?;
                Some(a)
            } else {
                None
            };

            Ok(ClaimOutput {
                action: "created".into(),
                claim: Some(claim),
                assertion,
                claims: None,
            })
        }
        "get" => {
            let id_str = args.id.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_claim".into(),
                argument: "id".into(),
                constraint: "required for get".into(),
                received: "null".into(),
            })?;
            let id: uuid::Uuid = id_str.parse().map_err(|_| VidyaError::InvalidArgument {
                tool: "vidya_claim".into(),
                argument: "id".into(),
                constraint: "valid UUID".into(),
                received: id_str,
            })?;
            let claim = sqlx::query_as::<_, db::ClaimRow>("SELECT * FROM claims WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?;
            if claim.is_none() {
                return Err(VidyaError::NotFound {
                    tool: "vidya_claim".into(),
                    kind: "claim".into(),
                });
            }
            Ok(ClaimOutput {
                action: "found".into(),
                claim,
                assertion: None,
                claims: None,
            })
        }
        "list" => {
            let claims = db::list_claims(
                pool,
                domain.id,
                args.template.as_deref(),
                args.status.as_deref(),
            )
            .await?;
            Ok(ClaimOutput {
                action: "listed".into(),
                claim: None,
                assertion: None,
                claims: Some(claims),
            })
        }
        other => Err(VidyaError::InvalidArgument {
            tool: "vidya_claim".into(),
            argument: "action".into(),
            constraint: "must be create, get, or list".into(),
            received: other.into(),
        }),
    }
}
