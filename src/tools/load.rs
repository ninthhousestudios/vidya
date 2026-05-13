use jsonschema::Validator;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::{Result, VidyaError};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LoadArgs {
    /// Complete domain payload as JSON string (or inline JSON object)
    pub payload: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct DomainPayload {
    pub domain: DomainDef,
    #[serde(default)]
    pub entity_kinds: Vec<EntityKindDef>,
    #[serde(default)]
    pub relation_kinds: Vec<RelationKindDef>,
    #[serde(default)]
    pub claim_templates: Vec<ClaimTemplateDef>,
    #[serde(default)]
    pub traditions: Vec<TraditionDef>,
    #[serde(default)]
    pub sources: Vec<SourceDef>,
    #[serde(default)]
    pub entities: Vec<EntityDef>,
    #[serde(default)]
    pub claims: Vec<ClaimDef>,
    #[serde(default)]
    pub relations: Vec<RelationDef>,
}

#[derive(Debug, Deserialize)]
pub struct DomainDef {
    pub slug: String,
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct EntityKindDef {
    pub slug: String,
    pub schema: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct RelationKindDef {
    pub slug: String,
    pub src_kind: Option<String>,
    pub dst_kind: Option<String>,
    pub schema: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimTemplateDef {
    pub slug: String,
    pub param_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct TraditionDef {
    pub name: String,
    pub parent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SourceDef {
    pub slug: String,
    pub kind: String,
    pub reference: String,
    pub reliability: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct EntityDef {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub attrs: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ClaimDef {
    pub template: String,
    pub params: serde_json::Value,
    pub statement: String,
    #[serde(default = "default_active")]
    pub status: String,
    #[serde(default)]
    pub assertions: Vec<AssertionDef>,
}

fn default_active() -> String {
    "active".into()
}

#[derive(Debug, Deserialize)]
pub struct AssertionDef {
    pub tradition: String,
    pub source: String,
    #[serde(default = "default_shabda")]
    pub pramana: String,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

fn default_shabda() -> String {
    "shabda".into()
}

fn default_confidence() -> f32 {
    1.0
}

#[derive(Debug, Deserialize)]
pub struct RelationDef {
    pub kind: String,
    pub src: String,
    pub dst: String,
    #[serde(default)]
    pub attrs: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct LoadOutput {
    pub domain: String,
    pub entity_kinds: usize,
    pub relation_kinds: usize,
    pub claim_templates: usize,
    pub traditions: usize,
    pub sources: usize,
    pub entities: usize,
    pub claims: usize,
    pub assertions: usize,
    pub relations: usize,
}

pub async fn handle(pool: &PgPool, args: LoadArgs) -> Result<LoadOutput> {
    let payload: DomainPayload =
        serde_json::from_value(args.payload).map_err(|e| VidyaError::InvalidArgument {
            tool: "vidya_load".into(),
            argument: "payload".into(),
            constraint: "valid DomainPayload JSON".into(),
            received: e.to_string(),
        })?;

    let mut tx = pool.begin().await?;

    // Domain
    let domain_id = Uuid::now_v7();
    let domain_row = sqlx::query_as::<_, db::DomainRow>(
        "INSERT INTO domains (id, slug, title) VALUES ($1, $2, $3) \
         ON CONFLICT (slug) DO UPDATE SET slug = domains.slug \
         RETURNING *",
    )
    .bind(domain_id)
    .bind(&payload.domain.slug)
    .bind(&payload.domain.title)
    .fetch_one(&mut *tx)
    .await?;
    let did = domain_row.id;

    // Entity kinds
    let mut kind_map: std::collections::HashMap<String, Uuid> = std::collections::HashMap::new();
    for ek in &payload.entity_kinds {
        let id = Uuid::now_v7();
        let row = sqlx::query_as::<_, db::EntityKindRow>(
            "INSERT INTO entity_kinds (id, domain_id, slug, schema) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (domain_id, slug) DO UPDATE SET slug = entity_kinds.slug \
             RETURNING *",
        )
        .bind(id)
        .bind(did)
        .bind(&ek.slug)
        .bind(&ek.schema)
        .fetch_one(&mut *tx)
        .await?;
        kind_map.insert(ek.slug.clone(), row.id);
    }

    // Relation kinds
    let mut rk_map: std::collections::HashMap<String, Uuid> = std::collections::HashMap::new();
    for rk in &payload.relation_kinds {
        let id = Uuid::now_v7();
        let src_kind_id = rk.src_kind.as_ref().and_then(|s| kind_map.get(s)).copied();
        let dst_kind_id = rk.dst_kind.as_ref().and_then(|s| kind_map.get(s)).copied();
        let row = sqlx::query_as::<_, db::RelationKindRow>(
            "INSERT INTO relation_kinds (id, domain_id, slug, src_kind_id, dst_kind_id, schema) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (domain_id, slug) DO UPDATE SET slug = relation_kinds.slug \
             RETURNING *",
        )
        .bind(id)
        .bind(did)
        .bind(&rk.slug)
        .bind(src_kind_id)
        .bind(dst_kind_id)
        .bind(&rk.schema)
        .fetch_one(&mut *tx)
        .await?;
        rk_map.insert(rk.slug.clone(), row.id);
    }

    // Claim templates
    let mut tmpl_map: std::collections::HashMap<String, Uuid> = std::collections::HashMap::new();
    for ct in &payload.claim_templates {
        let id = Uuid::now_v7();
        let row = sqlx::query_as::<_, db::ClaimTemplateRow>(
            "INSERT INTO claim_templates (id, domain_id, slug, param_schema) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (domain_id, slug) DO UPDATE SET slug = claim_templates.slug \
             RETURNING *",
        )
        .bind(id)
        .bind(did)
        .bind(&ct.slug)
        .bind(&ct.param_schema)
        .fetch_one(&mut *tx)
        .await?;
        tmpl_map.insert(ct.slug.clone(), row.id);
    }

    // Traditions (two-pass for parent references)
    let mut trad_map: std::collections::HashMap<String, Uuid> = std::collections::HashMap::new();
    // First pass: insert all without parents
    for t in &payload.traditions {
        let id = Uuid::now_v7();
        let row = sqlx::query_as::<_, db::TraditionRow>(
            "INSERT INTO traditions (id, domain_id, name, parent_id) VALUES ($1, $2, $3, NULL) \
             ON CONFLICT (domain_id, name) DO UPDATE SET name = traditions.name \
             RETURNING *",
        )
        .bind(id)
        .bind(did)
        .bind(&t.name)
        .fetch_one(&mut *tx)
        .await?;
        trad_map.insert(t.name.clone(), row.id);
    }
    // Second pass: set parent_id where specified
    for t in &payload.traditions {
        if let Some(parent_name) = &t.parent {
            if let Some(&parent_id) = trad_map.get(parent_name) {
                if let Some(&child_id) = trad_map.get(&t.name) {
                    sqlx::query("UPDATE traditions SET parent_id = $1 WHERE id = $2")
                        .bind(parent_id)
                        .bind(child_id)
                        .execute(&mut *tx)
                        .await?;
                }
            }
        }
    }

    // Sources (upsert on slug — sources are global, not domain-scoped)
    let mut source_map: std::collections::HashMap<String, Uuid> = std::collections::HashMap::new();
    for s in &payload.sources {
        let id = Uuid::now_v7();
        let row = sqlx::query_as::<_, db::SourceRow>(
            "INSERT INTO sources (id, slug, kind, reference, reliability) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (slug) DO UPDATE SET slug = sources.slug \
             RETURNING *",
        )
        .bind(id)
        .bind(&s.slug)
        .bind(&s.kind)
        .bind(&s.reference)
        .bind(s.reliability)
        .fetch_one(&mut *tx)
        .await?;
        source_map.insert(s.slug.clone(), row.id);
    }

    // Entities
    let mut entity_map: std::collections::HashMap<String, Uuid> = std::collections::HashMap::new();
    for e in &payload.entities {
        let kind_id = kind_map.get(&e.kind).ok_or_else(|| VidyaError::InvalidArgument {
            tool: "vidya_load".into(),
            argument: "entities[].kind".into(),
            constraint: format!("must reference a defined entity_kind"),
            received: e.kind.clone(),
        })?;
        let id = Uuid::now_v7();
        let row = sqlx::query_as::<_, db::EntityRow>(
            "INSERT INTO entities (id, domain_id, kind_id, name, attrs) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (domain_id, kind_id, name) DO UPDATE SET name = entities.name \
             RETURNING *",
        )
        .bind(id)
        .bind(did)
        .bind(kind_id)
        .bind(&e.name)
        .bind(&e.attrs)
        .fetch_one(&mut *tx)
        .await?;
        entity_map.insert(e.name.clone(), row.id);
    }

    // Build validators from claim template param_schemas
    let mut validators: std::collections::HashMap<String, Validator> =
        std::collections::HashMap::new();
    for ct in &payload.claim_templates {
        if !ct.param_schema.is_null()
            && ct.param_schema != serde_json::json!({})
        {
            let v = Validator::new(&ct.param_schema).map_err(|e| {
                VidyaError::InvalidArgument {
                    tool: "vidya_load".into(),
                    argument: format!("claim_templates[{}].param_schema", ct.slug),
                    constraint: format!("invalid JSON Schema: {e}"),
                    received: ct.param_schema.to_string(),
                }
            })?;
            validators.insert(ct.slug.clone(), v);
        }
    }

    // Validate all claim params before inserting any
    for (i, c) in payload.claims.iter().enumerate() {
        if let Some(validator) = validators.get(&c.template) {
            if let Err(error) = validator.validate(&c.params) {
                let path = error.instance_path().to_string();
                let field = if path.is_empty() {
                    "(root)".to_string()
                } else {
                    path
                };
                return Err(VidyaError::InvalidArgument {
                    tool: "vidya_load".into(),
                    argument: format!("claims[{i}].params"),
                    constraint: format!(
                        "must match template '{}' param_schema at {field}: {error}",
                        c.template,
                    ),
                    received: c.params.to_string(),
                });
            }
        }
    }

    // Claims + assertions
    let mut total_assertions = 0usize;
    for c in &payload.claims {
        let template_id = tmpl_map.get(&c.template).ok_or_else(|| VidyaError::InvalidArgument {
            tool: "vidya_load".into(),
            argument: "claims[].template".into(),
            constraint: "must reference a defined claim_template".into(),
            received: c.template.clone(),
        })?;
        let id = Uuid::now_v7();
        let claim = sqlx::query_as::<_, db::ClaimRow>(
            "INSERT INTO claims (id, domain_id, template_id, params, status, statement) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (domain_id, template_id, md5(params::text)) \
             DO UPDATE SET id = claims.id \
             RETURNING *",
        )
        .bind(id)
        .bind(did)
        .bind(template_id)
        .bind(&c.params)
        .bind(&c.status)
        .bind(&c.statement)
        .fetch_one(&mut *tx)
        .await?;

        for a in &c.assertions {
            let tradition_id = trad_map.get(&a.tradition).ok_or_else(|| {
                VidyaError::InvalidArgument {
                    tool: "vidya_load".into(),
                    argument: "assertions[].tradition".into(),
                    constraint: "must reference a defined tradition".into(),
                    received: a.tradition.clone(),
                }
            })?;
            let source_id =
                source_map.get(&a.source).ok_or_else(|| VidyaError::InvalidArgument {
                    tool: "vidya_load".into(),
                    argument: "assertions[].source".into(),
                    constraint: "must reference a defined source slug".into(),
                    received: a.source.clone(),
                })?;
            let aid = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO assertions (id, claim_id, tradition_id, source_id, pramana, confidence) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT DO NOTHING",
            )
            .bind(aid)
            .bind(claim.id)
            .bind(tradition_id)
            .bind(source_id)
            .bind(&a.pramana)
            .bind(a.confidence)
            .execute(&mut *tx)
            .await?;
            total_assertions += 1;
        }
    }

    // Relations
    let mut total_relations = 0usize;
    for r in &payload.relations {
        let rk_id = rk_map.get(&r.kind).ok_or_else(|| VidyaError::InvalidArgument {
            tool: "vidya_load".into(),
            argument: "relations[].kind".into(),
            constraint: "must reference a defined relation_kind".into(),
            received: r.kind.clone(),
        })?;
        let src_id = entity_map.get(&r.src).ok_or_else(|| VidyaError::InvalidArgument {
            tool: "vidya_load".into(),
            argument: "relations[].src".into(),
            constraint: "must reference a defined entity name".into(),
            received: r.src.clone(),
        })?;
        let dst_id = entity_map.get(&r.dst).ok_or_else(|| VidyaError::InvalidArgument {
            tool: "vidya_load".into(),
            argument: "relations[].dst".into(),
            constraint: "must reference a defined entity name".into(),
            received: r.dst.clone(),
        })?;
        sqlx::query(
            "INSERT INTO relations (id, domain_id, kind_id, src_entity_id, dst_entity_id, attrs) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (domain_id, kind_id, src_entity_id, dst_entity_id) DO NOTHING",
        )
        .bind(Uuid::now_v7())
        .bind(did)
        .bind(rk_id)
        .bind(src_id)
        .bind(dst_id)
        .bind(&r.attrs)
        .execute(&mut *tx)
        .await?;
        total_relations += 1;
    }

    tx.commit().await?;

    Ok(LoadOutput {
        domain: payload.domain.slug,
        entity_kinds: payload.entity_kinds.len(),
        relation_kinds: payload.relation_kinds.len(),
        claim_templates: payload.claim_templates.len(),
        traditions: payload.traditions.len(),
        sources: payload.sources.len(),
        entities: payload.entities.len(),
        claims: payload.claims.len(),
        assertions: total_assertions,
        relations: total_relations,
    })
}
