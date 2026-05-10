use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::db;
use crate::error::{Result, VidyaError};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DomainArgs {
    /// Action: "create", "get", or "list"
    pub action: String,
    /// Domain slug (required for create/get)
    pub slug: Option<String>,
    /// Domain title (required for create)
    pub title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DomainOutput {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<db::DomainRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domains: Option<Vec<db::DomainRow>>,
}

pub async fn handle(pool: &PgPool, args: DomainArgs) -> Result<DomainOutput> {
    match args.action.as_str() {
        "create" => {
            let slug = args.slug.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_domain".into(),
                argument: "slug".into(),
                constraint: "required for create".into(),
                received: "null".into(),
            })?;
            let title = args.title.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_domain".into(),
                argument: "title".into(),
                constraint: "required for create".into(),
                received: "null".into(),
            })?;
            let domain = db::insert_domain(pool, &slug, &title).await?;
            Ok(DomainOutput {
                action: "created".into(),
                domain: Some(domain),
                domains: None,
            })
        }
        "get" => {
            let slug = args.slug.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_domain".into(),
                argument: "slug".into(),
                constraint: "required for get".into(),
                received: "null".into(),
            })?;
            let domain = db::get_domain_by_slug(pool, &slug).await?;
            if domain.is_none() {
                return Err(VidyaError::NotFound {
                    tool: "vidya_domain".into(),
                    kind: format!("domain '{slug}'"),
                });
            }
            Ok(DomainOutput {
                action: "found".into(),
                domain,
                domains: None,
            })
        }
        "list" => {
            let domains = db::list_domains(pool).await?;
            Ok(DomainOutput {
                action: "listed".into(),
                domain: None,
                domains: Some(domains),
            })
        }
        other => Err(VidyaError::InvalidArgument {
            tool: "vidya_domain".into(),
            argument: "action".into(),
            constraint: "must be create, get, or list".into(),
            received: other.into(),
        }),
    }
}
