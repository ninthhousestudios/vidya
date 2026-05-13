use std::sync::Arc;

use rmcp::{
    ErrorData, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use sqlx::PgPool;

use crate::engine::Engine;
use crate::error::to_error_data;
use crate::tools;

#[derive(Clone)]
pub struct VidyaServer {
    pool: PgPool,
    engine: Arc<Engine>,
    tool_router: ToolRouter<Self>,
}

impl VidyaServer {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            engine: Arc::new(Engine::new()),
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router(router = tool_router)]
impl VidyaServer {
    #[tool(description = "Health check. Returns DB connectivity, domain/claim counts, and version.")]
    pub async fn vidya_health(
        &self,
        Parameters(_args): Parameters<tools::HealthArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::health::handle(&self.pool)
            .await
            .map_err(to_error_data)?;
        serde_json::to_string_pretty(&out).map_err(json_err)
    }

    #[tool(description = "Domain CRUD. Actions: create (slug+title), get (slug), list.")]
    pub async fn vidya_domain(
        &self,
        Parameters(args): Parameters<tools::DomainArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::domain::handle(&self.pool, args)
            .await
            .map_err(to_error_data)?;
        serde_json::to_string_pretty(&out).map_err(json_err)
    }

    #[tool(description = "Entity CRUD. Actions: create (domain+kind+name+attrs), get (domain+name), list (domain, optional kind filter).")]
    pub async fn vidya_entity(
        &self,
        Parameters(args): Parameters<tools::EntityArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::entity::handle(&self.pool, args)
            .await
            .map_err(to_error_data)?;
        serde_json::to_string_pretty(&out).map_err(json_err)
    }

    #[tool(description = "Claim CRUD. Actions: create (domain+template+params+statement, optional tradition+source for inline assertion), get (id), list (domain, optional template/status filter), update (id+status — enforces valid transitions: proposed→active, proposed→historical, active→historical).")]
    pub async fn vidya_claim(
        &self,
        Parameters(args): Parameters<tools::ClaimArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::claim::handle(&self.pool, args)
            .await
            .map_err(to_error_data)?;
        serde_json::to_string_pretty(&out).map_err(json_err)
    }

    #[tool(description = "Relation CRUD. Actions: create (domain+kind+src_entity+dst_entity, optional src_domain/dst_domain for cross-domain), get (id), list (entity — returns all relations involving that entity, optional entity_domain).")]
    pub async fn vidya_relation(
        &self,
        Parameters(args): Parameters<tools::RelationArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::relation::handle(&self.pool, args)
            .await
            .map_err(to_error_data)?;
        serde_json::to_string_pretty(&out).map_err(json_err)
    }

    #[tool(description = "Structured knowledge query. Requires domain. Modes: (1) entity lookup — entity name returns relations + claims with provenance; relation_kind and traverse_depth control graph walk. (2) entity search — entity_kind, name_pattern, attrs filter return matching entities. (3) cross-entity predicate — entity_kind + claim_template + claim_params finds entities linked to matching claims. (4) claim provenance — claim_id returns assertion chain + derivation chain. (5) domain claims — claim_template/tradition/pramana filters on all active claims.")]
    pub async fn vidya_query(
        &self,
        Parameters(args): Parameters<tools::QueryArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::query::handle(&self.pool, args)
            .await
            .map_err(to_error_data)?;
        serde_json::to_string_pretty(&out).map_err(json_err)
    }

    #[tool(description = "Bulk load a complete domain from a JSON payload. Transactional and idempotent. Payload includes domain, entity_kinds, relation_kinds, claim_templates, traditions, sources, entities, claims (with inline assertions), and relations.")]
    pub async fn vidya_load(
        &self,
        Parameters(args): Parameters<tools::LoadArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::load::handle(&self.pool, args)
            .await
            .map_err(to_error_data)?;
        serde_json::to_string_pretty(&out).map_err(json_err)
    }

    #[tool(description = "Run the derivation engine. Requires domain, operation (e.g. 'sandhi'), and input (domain-specific JSON). Returns result and full derivation trace. Dispatches to the registered engine strategy for the given domain/operation.")]
    pub async fn vidya_derive(
        &self,
        Parameters(args): Parameters<tools::DeriveArgs>,
    ) -> Result<String, ErrorData> {
        let out = tools::derive::handle_with_engine(&self.pool, &self.engine, args)
            .await
            .map_err(to_error_data)?;
        serde_json::to_string_pretty(&out).map_err(json_err)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for VidyaServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "vidya v0.1.0 \u{2014} structured knowledge graph with reasoning. \
                 Three-layer model: ontology (entity_kinds, relation_kinds, claim_templates), \
                 facts (entities, claims, relations), epistemology (traditions, sources, \
                 assertions with pramana). Eight tools: vidya_health, vidya_domain, \
                 vidya_entity, vidya_claim, vidya_relation, vidya_query, vidya_load, vidya_derive.",
            )
    }
}

fn json_err(e: serde_json::Error) -> ErrorData {
    ErrorData::internal_error(format!("JSON serialization error: {e}"), None)
}
