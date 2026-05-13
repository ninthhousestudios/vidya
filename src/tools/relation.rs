use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::db;
use crate::error::{Result, VidyaError};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RelationArgs {
    /// Action: "create", "get", or "list"
    pub action: String,
    /// Domain slug (where the relation_kind is defined)
    pub domain: String,
    /// Relation kind slug (required for create)
    pub kind: Option<String>,
    /// Source entity name (required for create)
    pub src_entity: Option<String>,
    /// Destination entity name (required for create)
    pub dst_entity: Option<String>,
    /// Source entity domain slug (optional, defaults to relation's domain)
    pub src_domain: Option<String>,
    /// Destination entity domain slug (optional, defaults to relation's domain)
    pub dst_domain: Option<String>,
    /// Relation attributes as JSON
    pub attrs: Option<serde_json::Value>,
    /// Relation ID for get action
    pub id: Option<String>,
    /// Entity name for list action (lists all relations involving this entity)
    pub entity: Option<String>,
    /// Entity domain slug for list action (optional, defaults to relation's domain)
    pub entity_domain: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RelationOutput {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<db::RelationRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations: Option<Vec<db::RelationRow>>,
}

pub async fn handle(pool: &PgPool, args: RelationArgs) -> Result<RelationOutput> {
    let domain = db::get_domain_by_slug(pool, &args.domain)
        .await?
        .ok_or_else(|| VidyaError::NotFound {
            tool: "vidya_relation".into(),
            kind: format!("domain '{}'", args.domain),
        })?;

    match args.action.as_str() {
        "create" => {
            let kind_slug = args.kind.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_relation".into(),
                argument: "kind".into(),
                constraint: "required for create".into(),
                received: "null".into(),
            })?;
            let src_name = args.src_entity.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_relation".into(),
                argument: "src_entity".into(),
                constraint: "required for create".into(),
                received: "null".into(),
            })?;
            let dst_name = args.dst_entity.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_relation".into(),
                argument: "dst_entity".into(),
                constraint: "required for create".into(),
                received: "null".into(),
            })?;

            let kind = db::get_relation_kind(pool, domain.id, &kind_slug)
                .await?
                .ok_or_else(|| VidyaError::NotFound {
                    tool: "vidya_relation".into(),
                    kind: format!("relation_kind '{kind_slug}'"),
                })?;

            let src_domain_id = if let Some(ref sd) = args.src_domain {
                db::get_domain_by_slug(pool, sd)
                    .await?
                    .ok_or_else(|| VidyaError::NotFound {
                        tool: "vidya_relation".into(),
                        kind: format!("domain '{sd}'"),
                    })?
                    .id
            } else {
                domain.id
            };
            let dst_domain_id = if let Some(ref dd) = args.dst_domain {
                db::get_domain_by_slug(pool, dd)
                    .await?
                    .ok_or_else(|| VidyaError::NotFound {
                        tool: "vidya_relation".into(),
                        kind: format!("domain '{dd}'"),
                    })?
                    .id
            } else {
                domain.id
            };

            let src = db::get_entity_by_name(pool, src_domain_id, &src_name)
                .await?
                .ok_or_else(|| VidyaError::NotFound {
                    tool: "vidya_relation".into(),
                    kind: format!("entity '{src_name}'"),
                })?;
            let dst = db::get_entity_by_name(pool, dst_domain_id, &dst_name)
                .await?
                .ok_or_else(|| VidyaError::NotFound {
                    tool: "vidya_relation".into(),
                    kind: format!("entity '{dst_name}'"),
                })?;

            if let Some(expected) = kind.src_kind_id {
                if src.kind_id != expected {
                    return Err(VidyaError::InvalidArgument {
                        tool: "vidya_relation".into(),
                        argument: "src_entity".into(),
                        constraint: format!(
                            "relation_kind '{kind_slug}' requires src entity of the declared kind",
                        ),
                        received: src_name,
                    });
                }
            }
            if let Some(expected) = kind.dst_kind_id {
                if dst.kind_id != expected {
                    return Err(VidyaError::InvalidArgument {
                        tool: "vidya_relation".into(),
                        argument: "dst_entity".into(),
                        constraint: format!(
                            "relation_kind '{kind_slug}' requires dst entity of the declared kind",
                        ),
                        received: dst_name,
                    });
                }
            }

            let attrs = args.attrs.unwrap_or(serde_json::json!({}));
            let relation =
                db::insert_relation(pool, domain.id, kind.id, src.id, dst.id, attrs).await?;
            Ok(RelationOutput {
                action: "created".into(),
                relation: Some(relation),
                relations: None,
            })
        }
        "get" => {
            let id_str = args.id.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_relation".into(),
                argument: "id".into(),
                constraint: "required for get".into(),
                received: "null".into(),
            })?;
            let id: uuid::Uuid = id_str.parse().map_err(|_| VidyaError::InvalidArgument {
                tool: "vidya_relation".into(),
                argument: "id".into(),
                constraint: "valid UUID".into(),
                received: id_str,
            })?;
            let relation =
                sqlx::query_as::<_, db::RelationRow>("SELECT * FROM relations WHERE id = $1")
                    .bind(id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| VidyaError::NotFound {
                        tool: "vidya_relation".into(),
                        kind: "relation".into(),
                    })?;
            Ok(RelationOutput {
                action: "found".into(),
                relation: Some(relation),
                relations: None,
            })
        }
        "list" => {
            let entity_name = args.entity.ok_or_else(|| VidyaError::InvalidArgument {
                tool: "vidya_relation".into(),
                argument: "entity".into(),
                constraint: "required for list (entity name to find relations for)".into(),
                received: "null".into(),
            })?;
            let entity_domain_id = if let Some(ref ed) = args.entity_domain {
                db::get_domain_by_slug(pool, ed)
                    .await?
                    .ok_or_else(|| VidyaError::NotFound {
                        tool: "vidya_relation".into(),
                        kind: format!("domain '{ed}'"),
                    })?
                    .id
            } else {
                domain.id
            };
            let entity = db::get_entity_by_name(pool, entity_domain_id, &entity_name)
                .await?
                .ok_or_else(|| VidyaError::NotFound {
                    tool: "vidya_relation".into(),
                    kind: format!("entity '{entity_name}'"),
                })?;
            let relations = db::list_relations_for_entity(pool, entity.id).await?;
            Ok(RelationOutput {
                action: "listed".into(),
                relation: None,
                relations: Some(relations),
            })
        }
        other => Err(VidyaError::InvalidArgument {
            tool: "vidya_relation".into(),
            argument: "action".into(),
            constraint: "must be create, get, or list".into(),
            received: other.into(),
        }),
    }
}
