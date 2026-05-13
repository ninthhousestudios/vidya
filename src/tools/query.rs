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
    /// Filter claims by param values (jsonb containment, for cross-entity predicate queries)
    pub claim_params: Option<serde_json::Value>,
    /// Filter relations by kind slug (single-entity mode)
    pub relation_kind: Option<String>,
    /// Relation traversal depth (default 1, single-entity mode)
    #[serde(default = "default_one")]
    pub traverse_depth: i32,
    /// Claim UUID for direct provenance lookup (assertions + derivation chain)
    pub claim_id: Option<String>,
    /// Include provenance (assertions + sources) in results
    #[serde(default = "default_true")]
    pub include_provenance: bool,
}

fn default_true() -> bool {
    true
}

fn default_one() -> i32 {
    1
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ProvenanceResult>,
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
    pub depth: i32,
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

#[derive(Debug, Serialize)]
pub struct ProvenanceResult {
    pub claim: db::ClaimRow,
    pub template_slug: String,
    pub assertions: Vec<AssertionExpanded>,
    pub derivation_chain: Vec<DerivationStep>,
}

#[derive(Debug, Serialize)]
pub struct DerivationStep {
    pub step_order: i32,
    pub premise: db::ClaimRow,
    pub premise_template_slug: String,
}

pub async fn handle(pool: &PgPool, args: QueryArgs) -> Result<QueryOutput> {
    let domain = db::get_domain_by_slug(pool, &args.domain)
        .await?
        .ok_or_else(|| VidyaError::NotFound {
            tool: "vidya_query".into(),
            kind: format!("domain '{}'", args.domain),
        })?;

    if let Some(ref claim_id_str) = args.claim_id {
        let claim_id: uuid::Uuid = claim_id_str.parse().map_err(|_| VidyaError::InvalidArgument {
            tool: "vidya_query".into(),
            argument: "claim_id".into(),
            constraint: "valid UUID".into(),
            received: claim_id_str.clone(),
        })?;

        let claim = sqlx::query_as::<_, db::ClaimRow>(
            "SELECT * FROM claims WHERE id = $1 AND domain_id = $2",
        )
        .bind(claim_id)
        .bind(domain.id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| VidyaError::NotFound {
            tool: "vidya_query".into(),
            kind: format!("claim '{claim_id_str}'"),
        })?;

        let template_slug = sqlx::query_scalar::<_, String>(
            "SELECT slug FROM claim_templates WHERE id = $1",
        )
        .bind(claim.template_id)
        .fetch_one(pool)
        .await?;

        let assertions = load_assertions_expanded(pool, claim.id, None, None).await?;

        let derivation_chain = load_derivation_chain(pool, claim.id).await?;

        return Ok(QueryOutput {
            domain: args.domain,
            entity: None,
            entities: None,
            claims: None,
            provenance: Some(ProvenanceResult {
                claim,
                template_slug,
                assertions,
                derivation_chain,
            }),
        });
    } else if let Some(entity_name) = &args.entity {
        let entity = db::get_entity_by_name(pool, domain.id, entity_name)
            .await?
            .ok_or_else(|| VidyaError::NotFound {
                tool: "vidya_query".into(),
                kind: format!("entity '{entity_name}'"),
            })?;

        let relations = load_relations_expanded(pool, &entity, args.relation_kind.as_deref(), args.traverse_depth).await?;
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
            provenance: None,
        })
    } else if args.entity_kind.is_some() && args.claim_template.is_some() && args.claim_params.is_some() {
        let kind_slug = args.entity_kind.as_ref().unwrap();
        let tmpl_slug = args.claim_template.as_ref().unwrap();
        let params = args.claim_params.as_ref().unwrap();

        let entities = sqlx::query_as::<_, db::EntityRow>(
            "SELECT DISTINCT e.* FROM entities e \
             JOIN entity_kinds ek ON e.kind_id = ek.id \
             JOIN claims c ON c.domain_id = e.domain_id AND c.status = 'active' \
             JOIN claim_templates ct ON c.template_id = ct.id \
             WHERE e.domain_id = $1 AND ek.slug = $2 AND ct.slug = $3 \
             AND c.params @> $4 \
             AND EXISTS (SELECT 1 FROM jsonb_each_text(c.params) kv WHERE kv.value = e.name) \
             ORDER BY e.name",
        )
        .bind(domain.id)
        .bind(kind_slug)
        .bind(tmpl_slug)
        .bind(params)
        .fetch_all(pool)
        .await?;

        return Ok(QueryOutput {
            domain: args.domain,
            entity: None,
            entities: Some(entities),
            claims: None,
            provenance: None,
        });
    } else if args.entity_kind.is_some() || args.name_pattern.is_some() || args.attrs.is_some() {
        if let Some(ref predicate) = args.attrs {
            if !predicate.is_object() {
                return Err(VidyaError::InvalidArgument {
                    tool: "vidya_query".into(),
                    argument: "attrs".into(),
                    constraint: "JSON object".into(),
                    received: predicate.to_string(),
                });
            }
        }

        let entities = if let Some(ref predicate) = args.attrs {
            sqlx::query_as::<_, db::EntityRow>(
                "SELECT e.* FROM entities e \
                 JOIN entity_kinds ek ON e.kind_id = ek.id \
                 WHERE e.domain_id = $1 \
                 AND ($2::text IS NULL OR ek.slug = $2) \
                 AND e.attrs @> $3 \
                 ORDER BY e.name",
            )
            .bind(domain.id)
            .bind(args.entity_kind.as_deref())
            .bind(predicate)
            .fetch_all(pool)
            .await?
        } else {
            db::list_entities(pool, domain.id, args.entity_kind.as_deref()).await?
        };

        let entities = if let Some(ref pattern) = args.name_pattern {
            let pattern_lower = pattern.to_lowercase();
            entities.into_iter().filter(|e| e.name.to_lowercase().contains(&pattern_lower)).collect()
        } else {
            entities
        };

        return Ok(QueryOutput {
            domain: args.domain,
            entity: None,
            entities: Some(entities),
            claims: None,
            provenance: None,
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
            provenance: None,
        })
    }
}

async fn load_relations_expanded(
    pool: &PgPool,
    entity: &db::EntityRow,
    relation_kind_filter: Option<&str>,
    max_depth: i32,
) -> Result<Vec<RelationExpanded>> {
    use std::collections::HashSet;

    let max_depth = max_depth.max(1).min(10);
    let mut expanded = Vec::new();
    let mut visited = HashSet::new();
    visited.insert(entity.id);
    let mut frontier = vec![entity.id];

    for current_depth in 1..=max_depth {
        let mut next_frontier = Vec::new();
        for entity_id in &frontier {
            let relations = db::list_relations_for_entity(pool, *entity_id).await?;
            for rel in relations {
                let kind_slug =
                    sqlx::query_scalar::<_, String>("SELECT slug FROM relation_kinds WHERE id = $1")
                        .bind(rel.kind_id)
                        .fetch_one(pool)
                        .await?;

                if let Some(filter) = relation_kind_filter {
                    if kind_slug != filter {
                        continue;
                    }
                }

                let (other_id, direction) = if rel.src_entity_id == *entity_id {
                    (rel.dst_entity_id, "outgoing")
                } else {
                    (rel.src_entity_id, "incoming")
                };

                if visited.contains(&other_id) {
                    continue;
                }

                let other_name =
                    sqlx::query_scalar::<_, String>("SELECT name FROM entities WHERE id = $1")
                        .bind(other_id)
                        .fetch_one(pool)
                        .await?;

                visited.insert(other_id);
                next_frontier.push(other_id);

                expanded.push(RelationExpanded {
                    relation: rel,
                    kind_slug,
                    other_entity_name: other_name,
                    direction: direction.into(),
                    depth: current_depth,
                });
            }
        }
        if next_frontier.is_empty() {
            break;
        }
        frontier = next_frontier;
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
         AND EXISTS (SELECT 1 FROM jsonb_each_text(c.params) kv WHERE kv.value = $2) \
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

async fn load_derivation_chain(
    pool: &PgPool,
    root_claim_id: uuid::Uuid,
) -> Result<Vec<DerivationStep>> {
    use std::collections::HashSet;

    let mut chain = Vec::new();
    let mut visited = HashSet::new();
    visited.insert(root_claim_id);
    let mut queue = vec![root_claim_id];
    let mut global_step = 0;

    while let Some(conclusion_id) = queue.pop() {
        let derivation_rows = sqlx::query_as::<_, db::DerivationRow>(
            "SELECT * FROM derivations WHERE conclusion_claim_id = $1 ORDER BY step_order",
        )
        .bind(conclusion_id)
        .fetch_all(pool)
        .await?;

        for d in derivation_rows {
            let premise = sqlx::query_as::<_, db::ClaimRow>(
                "SELECT * FROM claims WHERE id = $1",
            )
            .bind(d.premise_claim_id)
            .fetch_one(pool)
            .await?;

            let premise_template_slug = sqlx::query_scalar::<_, String>(
                "SELECT slug FROM claim_templates WHERE id = $1",
            )
            .bind(premise.template_id)
            .fetch_one(pool)
            .await?;

            global_step += 1;
            chain.push(DerivationStep {
                step_order: global_step,
                premise,
                premise_template_slug,
            });

            if visited.insert(d.premise_claim_id) {
                queue.push(d.premise_claim_id);
            }
        }
    }
    Ok(chain)
}
