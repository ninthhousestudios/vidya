use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ErrorData, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use vidya_core::KnowledgeStore;

use crate::error::to_error_data;

#[derive(Clone)]
pub struct VidyaServer {
    store: Arc<KnowledgeStore>,
    tool_router: ToolRouter<Self>,
}

impl VidyaServer {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self {
            store,
            tool_router: Self::tool_router(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct HealthArgs {}

#[derive(Debug, Serialize)]
pub struct HealthOutput {
    pub status: &'static str,
    pub triple_count: usize,
    pub graph_count: usize,
    pub domains: Vec<String>,
    pub version: &'static str,
}

#[tool_router(router = tool_router)]
impl VidyaServer {
    #[tool(
        description = "Health check. Returns store status, triple/graph counts, loaded domains, and version."
    )]
    pub async fn vidya_health(
        &self,
        Parameters(_args): Parameters<HealthArgs>,
    ) -> Result<String, ErrorData> {
        let triple_count = self.store.triple_count().map_err(to_error_data)?;
        let graph_count = self.store.graph_count();
        let domains = self.store.domains();

        let out = HealthOutput {
            status: "ok",
            triple_count,
            graph_count,
            domains,
            version: env!("CARGO_PKG_VERSION"),
        };
        serde_json::to_string_pretty(&out)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for VidyaServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "vidya — structured knowledge graph backed by Oxigraph. \
             RDF triplestore with named graphs per domain, RDF-star provenance, \
             and SPARQL internally. Tools: vidya_health.",
        )
    }
}
