use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::db;
use crate::error::{Result, VidyaError};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EntityArgs {
    /// Action: "create", "get", or "list"
    pub action: String,
    /// Domain slug
    pub domain: String,
    /// Entity kind slug (required for create, optional filter for list)
    pub kind: Option<String>,
    /// Entity name (required for create/get)
    pub name: Option<String>,
    /// Entity attributes as JSON
    pub attrs: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct EntityOutput {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<db::EntityRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<Vec<db::EntityRow>>,
}

pub async fn handle(pool: &PgPool, args: EntityArgs) -> Result<EntityOutput> {
    let domain = db::get_domain_by_slug(pool, &args.domain)
        .await?
        .ok_or_else(|| VidyaError::NotFound {
            tool: "vidya_entity".into(),
            kind: format!("domain '{}'", args.domain),
        })?;

    match args.action.as_str() {
        "create" => {
            let kind_slug = args.kind.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_entity".into(),
                argument: "kind".into(),
                constraint: "required for create".into(),
                received: "null".into(),
            })?;
            let name = args.name.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_entity".into(),
                argument: "name".into(),
                constraint: "required for create".into(),
                received: "null".into(),
            })?;
            let kind = db::get_entity_kind(pool, domain.id, &kind_slug)
                .await?
                .ok_or_else(|| VidyaError::NotFound {
                    tool: "vidya_entity".into(),
                    kind: format!("entity_kind '{kind_slug}'"),
                })?;
            let attrs = args.attrs.unwrap_or(serde_json::json!({}));
            let entity = db::insert_entity(pool, domain.id, kind.id, &name, attrs).await?;
            Ok(EntityOutput {
                action: "created".into(),
                entity: Some(entity),
                entities: None,
            })
        }
        "get" => {
            let name = args.name.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_entity".into(),
                argument: "name".into(),
                constraint: "required for get".into(),
                received: "null".into(),
            })?;
            let entity = db::get_entity_by_name(pool, domain.id, &name).await?;
            if entity.is_none() {
                return Err(VidyaError::NotFound {
                    tool: "vidya_entity".into(),
                    kind: format!("entity '{name}'"),
                });
            }
            Ok(EntityOutput {
                action: "found".into(),
                entity,
                entities: None,
            })
        }
        "list" => {
            let entities =
                db::list_entities(pool, domain.id, args.kind.as_deref()).await?;
            Ok(EntityOutput {
                action: "listed".into(),
                entity: None,
                entities: Some(entities),
            })
        }
        other => Err(VidyaError::InvalidArgument {
            tool: "vidya_entity".into(),
            argument: "action".into(),
            constraint: "must be create, get, or list".into(),
            received: other.into(),
        }),
    }
}
