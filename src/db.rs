use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::config::Config;
use crate::error::Result;

pub async fn connect(cfg: &Config) -> std::result::Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(cfg.db_max_connections)
        .acquire_timeout(cfg.db_acquire_timeout)
        .idle_timeout(cfg.db_idle_timeout)
        .connect(&cfg.database_url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> std::result::Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

// -- Ontology rows --

#[derive(Debug, FromRow, Serialize)]
pub struct DomainRow {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
}

#[derive(Debug, FromRow, Serialize)]
pub struct EntityKindRow {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub slug: String,
    pub schema: Option<serde_json::Value>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct RelationKindRow {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub slug: String,
    pub src_kind_id: Option<Uuid>,
    pub dst_kind_id: Option<Uuid>,
    pub schema: Option<serde_json::Value>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct ClaimTemplateRow {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub slug: String,
    pub param_schema: serde_json::Value,
}

// -- Fact rows --

#[derive(Debug, FromRow, Serialize)]
pub struct EntityRow {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub kind_id: Uuid,
    pub name: String,
    pub attrs: serde_json::Value,
}

#[derive(Debug, FromRow, Serialize)]
pub struct ClaimRow {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub template_id: Uuid,
    pub params: serde_json::Value,
    pub status: String,
    pub statement: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct RelationRow {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub kind_id: Uuid,
    pub src_entity_id: Uuid,
    pub dst_entity_id: Uuid,
    pub attrs: serde_json::Value,
}

// -- Epistemology rows --

#[derive(Debug, FromRow, Serialize)]
pub struct TraditionRow {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct SourceRow {
    pub id: Uuid,
    pub slug: String,
    pub kind: String,
    pub reference: String,
    pub reliability: Option<f32>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct AssertionRow {
    pub id: Uuid,
    pub claim_id: Uuid,
    pub tradition_id: Uuid,
    pub source_id: Uuid,
    pub pramana: String,
    pub confidence: f32,
    pub asserted_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct DerivationRow {
    pub id: Uuid,
    pub conclusion_claim_id: Uuid,
    pub premise_claim_id: Uuid,
    pub step_order: i32,
    pub created_at: DateTime<Utc>,
}

// -- Domain CRUD --

pub async fn insert_domain(pool: &PgPool, slug: &str, title: &str) -> Result<DomainRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, DomainRow>(
        "INSERT INTO domains (id, slug, title) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(id)
    .bind(slug)
    .bind(title)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_domain_by_slug(pool: &PgPool, slug: &str) -> Result<Option<DomainRow>> {
    let row = sqlx::query_as::<_, DomainRow>("SELECT * FROM domains WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn list_domains(pool: &PgPool) -> Result<Vec<DomainRow>> {
    let rows = sqlx::query_as::<_, DomainRow>("SELECT * FROM domains ORDER BY slug")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

// -- Entity kind CRUD --

pub async fn insert_entity_kind(
    pool: &PgPool,
    domain_id: Uuid,
    slug: &str,
    schema: Option<serde_json::Value>,
) -> Result<EntityKindRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, EntityKindRow>(
        "INSERT INTO entity_kinds (id, domain_id, slug, schema) VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(id)
    .bind(domain_id)
    .bind(slug)
    .bind(schema)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_entity_kind(pool: &PgPool, domain_id: Uuid, slug: &str) -> Result<Option<EntityKindRow>> {
    let row = sqlx::query_as::<_, EntityKindRow>(
        "SELECT * FROM entity_kinds WHERE domain_id = $1 AND slug = $2",
    )
    .bind(domain_id)
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

// -- Relation kind CRUD --

pub async fn insert_relation_kind(
    pool: &PgPool,
    domain_id: Uuid,
    slug: &str,
    src_kind_id: Option<Uuid>,
    dst_kind_id: Option<Uuid>,
    schema: Option<serde_json::Value>,
) -> Result<RelationKindRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, RelationKindRow>(
        "INSERT INTO relation_kinds (id, domain_id, slug, src_kind_id, dst_kind_id, schema) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(id)
    .bind(domain_id)
    .bind(slug)
    .bind(src_kind_id)
    .bind(dst_kind_id)
    .bind(schema)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

// -- Claim template CRUD --

pub async fn insert_claim_template(
    pool: &PgPool,
    domain_id: Uuid,
    slug: &str,
    param_schema: serde_json::Value,
) -> Result<ClaimTemplateRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, ClaimTemplateRow>(
        "INSERT INTO claim_templates (id, domain_id, slug, param_schema) \
         VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(id)
    .bind(domain_id)
    .bind(slug)
    .bind(param_schema)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_claim_template(pool: &PgPool, domain_id: Uuid, slug: &str) -> Result<Option<ClaimTemplateRow>> {
    let row = sqlx::query_as::<_, ClaimTemplateRow>(
        "SELECT * FROM claim_templates WHERE domain_id = $1 AND slug = $2",
    )
    .bind(domain_id)
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

// -- Entity CRUD --

pub async fn insert_entity(
    pool: &PgPool,
    domain_id: Uuid,
    kind_id: Uuid,
    name: &str,
    attrs: serde_json::Value,
) -> Result<EntityRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, EntityRow>(
        "INSERT INTO entities (id, domain_id, kind_id, name, attrs) \
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(id)
    .bind(domain_id)
    .bind(kind_id)
    .bind(name)
    .bind(attrs)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_entity_by_name(
    pool: &PgPool,
    domain_id: Uuid,
    name: &str,
) -> Result<Option<EntityRow>> {
    let row = sqlx::query_as::<_, EntityRow>(
        "SELECT * FROM entities WHERE domain_id = $1 AND name = $2",
    )
    .bind(domain_id)
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_entities(
    pool: &PgPool,
    domain_id: Uuid,
    kind_slug: Option<&str>,
) -> Result<Vec<EntityRow>> {
    if let Some(kind) = kind_slug {
        let rows = sqlx::query_as::<_, EntityRow>(
            "SELECT e.* FROM entities e \
             JOIN entity_kinds ek ON e.kind_id = ek.id \
             WHERE e.domain_id = $1 AND ek.slug = $2 \
             ORDER BY e.name",
        )
        .bind(domain_id)
        .bind(kind)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    } else {
        let rows = sqlx::query_as::<_, EntityRow>(
            "SELECT * FROM entities WHERE domain_id = $1 ORDER BY name",
        )
        .bind(domain_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}

// -- Claim CRUD --

pub async fn insert_claim(
    pool: &PgPool,
    domain_id: Uuid,
    template_id: Uuid,
    params: serde_json::Value,
    status: &str,
    statement: &str,
) -> Result<ClaimRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, ClaimRow>(
        "INSERT INTO claims (id, domain_id, template_id, params, status, statement) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(id)
    .bind(domain_id)
    .bind(template_id)
    .bind(params)
    .bind(status)
    .bind(statement)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn upsert_claim(
    pool: &PgPool,
    domain_id: Uuid,
    template_id: Uuid,
    params: serde_json::Value,
    status: &str,
    statement: &str,
) -> Result<ClaimRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, ClaimRow>(
        "INSERT INTO claims (id, domain_id, template_id, params, status, statement) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (domain_id, template_id, md5(params::text)) DO UPDATE SET id = claims.id \
         RETURNING *",
    )
    .bind(id)
    .bind(domain_id)
    .bind(template_id)
    .bind(params)
    .bind(status)
    .bind(statement)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn list_claims(
    pool: &PgPool,
    domain_id: Uuid,
    template_slug: Option<&str>,
    status: Option<&str>,
) -> Result<Vec<ClaimRow>> {
    let rows = if let Some(tmpl) = template_slug {
        sqlx::query_as::<_, ClaimRow>(
            "SELECT c.* FROM claims c \
             JOIN claim_templates ct ON c.template_id = ct.id \
             WHERE c.domain_id = $1 AND ct.slug = $2 \
             AND ($3::text IS NULL OR c.status = $3) \
             ORDER BY c.created_at",
        )
        .bind(domain_id)
        .bind(tmpl)
        .bind(status)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, ClaimRow>(
            "SELECT * FROM claims WHERE domain_id = $1 \
             AND ($2::text IS NULL OR status = $2) \
             ORDER BY created_at",
        )
        .bind(domain_id)
        .bind(status)
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

// -- Relation CRUD --

pub async fn insert_relation(
    pool: &PgPool,
    domain_id: Uuid,
    kind_id: Uuid,
    src_entity_id: Uuid,
    dst_entity_id: Uuid,
    attrs: serde_json::Value,
) -> Result<RelationRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, RelationRow>(
        "INSERT INTO relations (id, domain_id, kind_id, src_entity_id, dst_entity_id, attrs) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (domain_id, kind_id, src_entity_id, dst_entity_id) DO UPDATE SET id = relations.id \
         RETURNING *",
    )
    .bind(id)
    .bind(domain_id)
    .bind(kind_id)
    .bind(src_entity_id)
    .bind(dst_entity_id)
    .bind(attrs)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn list_relations_for_entity(
    pool: &PgPool,
    entity_id: Uuid,
) -> Result<Vec<RelationRow>> {
    let rows = sqlx::query_as::<_, RelationRow>(
        "SELECT * FROM relations WHERE src_entity_id = $1 OR dst_entity_id = $1 \
         ORDER BY kind_id",
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// -- Tradition CRUD --

pub async fn insert_tradition(
    pool: &PgPool,
    domain_id: Uuid,
    name: &str,
    parent_id: Option<Uuid>,
) -> Result<TraditionRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, TraditionRow>(
        "INSERT INTO traditions (id, domain_id, name, parent_id) \
         VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(id)
    .bind(domain_id)
    .bind(name)
    .bind(parent_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn upsert_tradition(
    pool: &PgPool,
    domain_id: Uuid,
    name: &str,
    parent_id: Option<Uuid>,
) -> Result<TraditionRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, TraditionRow>(
        "INSERT INTO traditions (id, domain_id, name, parent_id) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (domain_id, name) DO UPDATE SET id = traditions.id \
         RETURNING *",
    )
    .bind(id)
    .bind(domain_id)
    .bind(name)
    .bind(parent_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

// -- Source CRUD --

pub async fn insert_source(
    pool: &PgPool,
    slug: &str,
    kind: &str,
    reference: &str,
    reliability: Option<f32>,
) -> Result<SourceRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, SourceRow>(
        "INSERT INTO sources (id, slug, kind, reference, reliability) \
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(id)
    .bind(slug)
    .bind(kind)
    .bind(reference)
    .bind(reliability)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn upsert_source(
    pool: &PgPool,
    slug: &str,
    kind: &str,
    reference: &str,
    reliability: Option<f32>,
) -> Result<SourceRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, SourceRow>(
        "INSERT INTO sources (id, slug, kind, reference, reliability) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (slug) DO UPDATE SET slug = sources.slug \
         RETURNING *",
    )
    .bind(id)
    .bind(slug)
    .bind(kind)
    .bind(reference)
    .bind(reliability)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_source_by_slug(pool: &PgPool, slug: &str) -> Result<Option<SourceRow>> {
    let row = sqlx::query_as::<_, SourceRow>("SELECT * FROM sources WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

// -- Assertion CRUD --

pub async fn insert_assertion(
    pool: &PgPool,
    claim_id: Uuid,
    tradition_id: Uuid,
    source_id: Uuid,
    pramana: &str,
    confidence: f32,
) -> Result<AssertionRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, AssertionRow>(
        "INSERT INTO assertions (id, claim_id, tradition_id, source_id, pramana, confidence) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(id)
    .bind(claim_id)
    .bind(tradition_id)
    .bind(source_id)
    .bind(pramana)
    .bind(confidence)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn list_assertions_for_claim(
    pool: &PgPool,
    claim_id: Uuid,
) -> Result<Vec<AssertionRow>> {
    let rows = sqlx::query_as::<_, AssertionRow>(
        "SELECT * FROM assertions WHERE claim_id = $1 ORDER BY asserted_at",
    )
    .bind(claim_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// -- Derivation CRUD --

pub async fn insert_derivation(
    pool: &PgPool,
    conclusion_claim_id: Uuid,
    premise_claim_id: Uuid,
    step_order: i32,
) -> Result<DerivationRow> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, DerivationRow>(
        "INSERT INTO derivations (id, conclusion_claim_id, premise_claim_id, step_order) \
         VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(id)
    .bind(conclusion_claim_id)
    .bind(premise_claim_id)
    .bind(step_order)
    .fetch_one(pool)
    .await?;
    Ok(row)
}
