use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::db;
use crate::error::{Result, VidyaError};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryArgs {
    /// Domain slug (required)
    pub domain: String,
    /// Entity name to look up (returns entity with its claims and relations)
    pub entity: Option<String>,
    /// Filter by entity kind slug
    pub entity_kind: Option<String>,
    /// Entity name pattern (substring match, case-insensitive)
    pub name_pattern: Option<String>,
    /// Filter entities by attribute predicates (jsonb containment, e.g. {"class": "short"})
    pub attrs: Option<serde_json::Value>,
    /// Filter claims by tradition name
    pub tradition: Option<String>,
    /// Filter assertions by pramana type
    pub pramana: Option<String>,
    /// Filter claims by template slug
    pub claim_template: Option<String>,
    /// Include provenance (assertions + sources) in results
    #[serde(default = "default_true")]
    pub include_provenance: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct QueryOutput {
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<EntityWithContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<Vec<db::EntityRow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claims: Option<Vec<ClaimWithProvenance>>,
}

#[derive(Debug, Serialize)]
pub struct EntityWithContext {
    pub entity: db::EntityRow,
    pub relations: Vec<RelationExpanded>,
    pub claims: Vec<ClaimWithProvenance>,
}

#[derive(Debug, Serialize)]
pub struct RelationExpanded {
    pub relation: db::RelationRow,
    pub kind_slug: String,
    pub other_entity_name: String,
    pub direction: String,
}

#[derive(Debug, Serialize)]
pub struct ClaimWithProvenance {
    pub claim: db::ClaimRow,
    pub template_slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assertions: Option<Vec<AssertionExpanded>>,
}

#[derive(Debug, Serialize)]
pub struct AssertionExpanded {
    pub assertion: db::AssertionRow,
    pub tradition_name: String,
    pub source_reference: String,
}

pub async fn handle(pool: &PgPool, args: QueryArgs) -> Result<QueryOutput> {
    let domain = db::get_domain_by_slug(pool, &args.domain)
        .await?
        .ok_or_else(|| VidyaError::NotFound {
            tool: "vidya_query".into(),
            kind: format!("domain '{}'", args.domain),
        })?;

    if let Some(entity_name) = &args.entity {
        let entity = db::get_entity_by_name(pool, domain.id, entity_name)
            .await?
            .ok_or_else(|| VidyaError::NotFound {
                tool: "vidya_query".into(),
                kind: format!("entity '{entity_name}'"),
            })?;

        let relations = load_relations_expanded(pool, &entity).await?;
        let claims = load_claims_for_entity(
            pool,
            domain.id,
            &entity,
            args.tradition.as_deref(),
            args.pramana.as_deref(),
            args.claim_template.as_deref(),
            args.include_provenance,
        )
        .await?;

        Ok(QueryOutput {
            domain: args.domain,
            entity: Some(EntityWithContext {
                entity,
                relations,
                claims,
            }),
            entities: None,
            claims: None,
        })
    } else if args.entity_kind.is_some() || args.name_pattern.is_some() || args.attrs.is_some() {
        let mut entities = db::list_entities(pool, domain.id, args.entity_kind.as_deref()).await?;
        if let Some(ref pattern) = args.name_pattern {
            let pattern_lower = pattern.to_lowercase();
            entities.retain(|e| e.name.to_lowercase().contains(&pattern_lower));
        }
        if let Some(ref predicate) = args.attrs {
            if let Some(pred_obj) = predicate.as_object() {
                entities.retain(|e| {
                    if let Some(attrs_obj) = e.attrs.as_object() {
                        pred_obj.iter().all(|(k, v)| attrs_obj.get(k) == Some(v))
                    } else {
                        false
                    }
                });
            }
        }
        return Ok(QueryOutput {
            domain: args.domain,
            entity: None,
            entities: Some(entities),
            claims: None,
        });
    } else {
        let claims = db::list_claims(
            pool,
            domain.id,
            args.claim_template.as_deref(),
            Some("active"),
        )
        .await?;

        let mut result = Vec::new();
        for claim in claims {
            let template_slug = sqlx::query_scalar::<_, String>(
                "SELECT slug FROM claim_templates WHERE id = $1",
            )
            .bind(claim.template_id)
            .fetch_one(pool)
            .await?;

            let assertions = if args.include_provenance {
                Some(load_assertions_expanded(pool, claim.id, args.tradition.as_deref(), args.pramana.as_deref()).await?)
            } else {
                None
            };

            if args.tradition.is_some() || args.pramana.is_some() {
                if let Some(ref a) = assertions {
                    if a.is_empty() {
                        continue;
                    }
                }
            }

            result.push(ClaimWithProvenance {
                claim,
                template_slug,
                assertions,
            });
        }

        Ok(QueryOutput {
            domain: args.domain,
            entity: None,
            entities: None,
            claims: Some(result),
        })
    }
}

async fn load_relations_expanded(
    pool: &PgPool,
    entity: &db::EntityRow,
) -> Result<Vec<RelationExpanded>> {
    let relations = db::list_relations_for_entity(pool, entity.id).await?;
    let mut expanded = Vec::new();
    for rel in relations {
        let kind_slug =
            sqlx::query_scalar::<_, String>("SELECT slug FROM relation_kinds WHERE id = $1")
                .bind(rel.kind_id)
                .fetch_one(pool)
                .await?;

        let (other_id, direction) = if rel.src_entity_id == entity.id {
            (rel.dst_entity_id, "outgoing")
        } else {
            (rel.src_entity_id, "incoming")
        };

        let other_name =
            sqlx::query_scalar::<_, String>("SELECT name FROM entities WHERE id = $1")
                .bind(other_id)
                .fetch_one(pool)
                .await?;

        expanded.push(RelationExpanded {
            relation: rel,
            kind_slug,
            other_entity_name: other_name,
            direction: direction.into(),
        });
    }
    Ok(expanded)
}

async fn load_claims_for_entity(
    pool: &PgPool,
    domain_id: uuid::Uuid,
    entity: &db::EntityRow,
    tradition: Option<&str>,
    pramana: Option<&str>,
    template: Option<&str>,
    include_provenance: bool,
) -> Result<Vec<ClaimWithProvenance>> {
    // Find claims that reference this entity in their params (by name)
    let entity_name = &entity.name;
    let claims = sqlx::query_as::<_, db::ClaimRow>(
        "SELECT c.* FROM claims c \
         LEFT JOIN claim_templates ct ON c.template_id = ct.id \
         WHERE c.domain_id = $1 AND c.status = 'active' \
         AND c.params::text ILIKE '%' || $2 || '%' \
         AND ($3::text IS NULL OR ct.slug = $3) \
         ORDER BY c.created_at",
    )
    .bind(domain_id)
    .bind(entity_name)
    .bind(template)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for claim in claims {
        let template_slug = sqlx::query_scalar::<_, String>(
            "SELECT slug FROM claim_templates WHERE id = $1",
        )
        .bind(claim.template_id)
        .fetch_one(pool)
        .await?;

        let assertions = if include_provenance {
            Some(load_assertions_expanded(pool, claim.id, tradition, pramana).await?)
        } else {
            None
        };

        if tradition.is_some() || pramana.is_some() {
            if let Some(ref a) = assertions {
                if a.is_empty() {
                    continue;
                }
            }
        }

        result.push(ClaimWithProvenance {
            claim,
            template_slug,
            assertions,
        });
    }
    Ok(result)
}

async fn load_assertions_expanded(
    pool: &PgPool,
    claim_id: uuid::Uuid,
    tradition: Option<&str>,
    pramana: Option<&str>,
) -> Result<Vec<AssertionExpanded>> {
    let assertions = sqlx::query_as::<_, db::AssertionRow>(
        "SELECT a.* FROM assertions a \
         JOIN traditions t ON a.tradition_id = t.id \
         WHERE a.claim_id = $1 \
         AND ($2::text IS NULL OR t.name = $2) \
         AND ($3::text IS NULL OR a.pramana = $3) \
         ORDER BY a.asserted_at",
    )
    .bind(claim_id)
    .bind(tradition)
    .bind(pramana)
    .fetch_all(pool)
    .await?;

    let mut expanded = Vec::new();
    for a in assertions {
        let tradition_name = sqlx::query_scalar::<_, String>(
            "SELECT name FROM traditions WHERE id = $1",
        )
        .bind(a.tradition_id)
        .fetch_one(pool)
        .await?;

        let source_reference = sqlx::query_scalar::<_, String>(
            "SELECT reference FROM sources WHERE id = $1",
        )
        .bind(a.source_id)
        .fetch_one(pool)
        .await?;

        expanded.push(AssertionExpanded {
            assertion: a,
            tradition_name,
            source_reference,
        });
    }
    Ok(expanded)
}
