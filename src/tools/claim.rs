use jsonschema::Validator;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::db;
use crate::error::{Result, VidyaError};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClaimArgs {
    /// Action: "create", "get", "list", or "update"
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

            if !template.param_schema.is_null()
                && template.param_schema != serde_json::json!({})
            {
                let validator = Validator::new(&template.param_schema).map_err(|e| {
                    VidyaError::InvalidArgument {
                        tool: "vidya_claim".into(),
                        argument: "template.param_schema".into(),
                        constraint: format!(
                            "template '{template_slug}' has invalid param_schema: {e}",
                        ),
                        received: template.param_schema.to_string(),
                    }
                })?;
                if let Err(error) = validator.validate(&params) {
                    let path = error.instance_path().to_string();
                    let field = if path.is_empty() {
                        "(root)".to_string()
                    } else {
                        path
                    };
                    return Err(VidyaError::InvalidArgument {
                        tool: "vidya_claim".into(),
                        argument: "params".into(),
                        constraint: format!(
                            "must match template '{template_slug}' param_schema at {field}: {error}",
                        ),
                        received: params.to_string(),
                    });
                }
            }

            let status = args.status.as_deref().unwrap_or("active");
            let claim =
                db::insert_claim(pool, domain.id, template.id, params, status, &statement).await?;

            let assertion = if let (Some(tradition_name), Some(source_ref)) =
                (args.tradition, args.source_ref)
            {
                let tradition = db::upsert_tradition(pool, domain.id, &tradition_name, None).await?;
                let source_kind = args.source_kind.as_deref().unwrap_or("text");
                let source_slug = slug_from_ref(&source_ref);
                let source =
                    db::upsert_source(pool, &source_slug, source_kind, &source_ref, None).await?;
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
        "update" => {
            let id_str = args.id.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_claim".into(),
                argument: "id".into(),
                constraint: "required for update".into(),
                received: "null".into(),
            })?;
            let id: uuid::Uuid = id_str.parse().map_err(|_| VidyaError::InvalidArgument {
                tool: "vidya_claim".into(),
                argument: "id".into(),
                constraint: "valid UUID".into(),
                received: id_str,
            })?;
            let new_status = args.status.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_claim".into(),
                argument: "status".into(),
                constraint: "required for update".into(),
                received: "null".into(),
            })?;

            // Fetch current claim scoped by domain (fix #2: cross-domain boundary)
            let current = sqlx::query_as::<_, db::ClaimRow>(
                "SELECT * FROM claims WHERE id = $1 AND domain_id = $2",
            )
            .bind(id)
            .bind(domain.id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| VidyaError::NotFound {
                tool: "vidya_claim".into(),
                kind: "claim".into(),
            })?;

            let allowed = matches!(
                (current.status.as_str(), new_status.as_str()),
                ("proposed", "active") | ("proposed", "historical") | ("active", "historical")
            );
            if !allowed {
                return Err(VidyaError::InvalidArgument {
                    tool: "vidya_claim".into(),
                    argument: "status".into(),
                    constraint: format!(
                        "transition from '{}' to '{new_status}' is not allowed",
                        current.status,
                    ),
                    received: new_status,
                });
            }

            // Atomic conditional update (fix #1: race condition)
            let claim = db::update_claim_status(
                pool,
                id,
                domain.id,
                &current.status,
                &new_status,
            )
            .await?
            .ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_claim".into(),
                argument: "status".into(),
                constraint: "status was concurrently modified".into(),
                received: new_status,
            })?;

            Ok(ClaimOutput {
                action: "updated".into(),
                claim: Some(claim),
                assertion: None,
                claims: None,
            })
        }
        other => Err(VidyaError::InvalidArgument {
            tool: "vidya_claim".into(),
            argument: "action".into(),
            constraint: "must be create, get, list, or update".into(),
            received: other.into(),
        }),
    }
}

fn slug_from_ref(reference: &str) -> String {
    reference
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
