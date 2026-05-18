use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ErrorData, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LoadArgs {
    /// Domain name (e.g. "jyotish")
    pub domain: String,
    /// Inline Turtle data (mutually exclusive with path)
    #[serde(default)]
    pub turtle: Option<String>,
    /// File path to a .ttl file (mutually exclusive with turtle)
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoadOutput {
    pub domain: String,
    pub triple_count: usize,
    pub graph_count: usize,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AssertArgs {
    /// Domain name (e.g. "jyotish")
    pub domain: String,
    /// Subject short name (e.g. "surya")
    pub subject: String,
    /// Predicate short name (e.g. "exaltedIn")
    pub predicate: String,
    /// Object short name (entity) or literal value
    pub object: String,
    /// If true (default), object is a literal string; set false to resolve as entity reference
    #[serde(default)]
    pub literal: Option<bool>,
    /// Tradition short name (e.g. "tradition-bphs") — required
    pub tradition: String,
    /// Source short name (e.g. "source-bphs") — required
    pub source: String,
    /// Pramana short name or vidya-prefixed (e.g. "vidya:shabda") — required
    pub pramana: String,
    /// Confidence value 0.0-1.0 (defaults to 1.0)
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct AssertOutput {
    pub domain: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub triple_count: usize,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMode {
    /// All properties, relationships, and provenance for a subject
    Describe,
    /// Find entities by kind with optional attribute filters
    Search,
    /// Walk relationships from a subject to depth N
    Traverse,
    /// Epistemological metadata for a specific triple
    Provenance,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryArgs {
    /// Query mode
    pub mode: QueryMode,
    /// Domain name (e.g. "jyotish")
    pub domain: String,
    /// [describe, traverse, provenance] Subject short name or prefixed name (e.g. "surya")
    #[serde(default)]
    pub subject: Option<String>,
    /// [search] Kind short name (e.g. "Graha", "Rashi")
    #[serde(default)]
    pub kind: Option<String>,
    /// [search] Attribute filters as key-value pairs (e.g. {"element": "fire"})
    #[serde(default)]
    pub filters: Option<HashMap<String, String>>,
    /// [traverse, provenance] Predicate short name (e.g. "naturalFriend", "exaltedIn")
    #[serde(default)]
    pub predicate: Option<String>,
    /// [traverse] Max traversal depth (defaults to 1, max 10)
    #[serde(default)]
    pub depth: Option<u32>,
    /// [provenance] Object short name or literal (e.g. "mesha", "malefic")
    #[serde(default)]
    pub object: Option<String>,
    /// Cross-cutting: filter by tradition (e.g. "tradition-bphs")
    #[serde(default)]
    pub tradition: Option<String>,
    /// Cross-cutting: filter by pramana (e.g. "vidya:shabda")
    #[serde(default)]
    pub pramana: Option<String>,
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

    #[tool(
        description = "Query the knowledge graph. Modes: 'describe' (subject) returns all properties and provenance; 'search' (kind, filters) finds entities; 'traverse' (subject, predicate, depth) walks relationships; 'provenance' (subject, predicate, object) returns epistemological metadata. Optional cross-cutting filters: 'tradition' and 'pramana' scope any mode to matching assertions."
    )]
    pub async fn vidya_query(
        &self,
        Parameters(args): Parameters<QueryArgs>,
    ) -> Result<String, ErrorData> {
        let prov_filter = vidya_core::ProvenanceFilter {
            tradition: args
                .tradition
                .as_deref()
                .map(|t| vidya_core::ontology::resolve_iri(t, &args.domain)),
            pramana: args
                .pramana
                .as_deref()
                .map(|p| vidya_core::ontology::resolve_iri(p, &args.domain)),
        };

        match args.mode {
            QueryMode::Describe => {
                let subject = args.subject.ok_or_else(|| {
                    ErrorData::invalid_params("'subject' is required for describe mode", None)
                })?;
                let result = self
                    .store
                    .describe(&args.domain, &subject, &prov_filter)
                    .map_err(to_error_data)?;
                serde_json::to_string_pretty(&result)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
            QueryMode::Search => {
                let kind = args.kind.ok_or_else(|| {
                    ErrorData::invalid_params("'kind' is required for search mode", None)
                })?;
                let filters: Vec<(String, String)> =
                    args.filters.unwrap_or_default().into_iter().collect();
                let result = self
                    .store
                    .search(&args.domain, &kind, &filters, &prov_filter)
                    .map_err(to_error_data)?;
                serde_json::to_string_pretty(&result)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
            QueryMode::Traverse => {
                let subject = args.subject.ok_or_else(|| {
                    ErrorData::invalid_params("'subject' is required for traverse mode", None)
                })?;
                let predicate = args.predicate.ok_or_else(|| {
                    ErrorData::invalid_params("'predicate' is required for traverse mode", None)
                })?;
                let depth = args.depth.unwrap_or(1);
                let result = self
                    .store
                    .traverse(&args.domain, &subject, &predicate, depth, &prov_filter)
                    .map_err(to_error_data)?;
                serde_json::to_string_pretty(&result)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
            QueryMode::Provenance => {
                let subject = args.subject.ok_or_else(|| {
                    ErrorData::invalid_params("'subject' is required for provenance mode", None)
                })?;
                let predicate = args.predicate.ok_or_else(|| {
                    ErrorData::invalid_params("'predicate' is required for provenance mode", None)
                })?;
                let object = args.object.ok_or_else(|| {
                    ErrorData::invalid_params("'object' is required for provenance mode", None)
                })?;
                let result = self
                    .store
                    .provenance(
                        &args.domain,
                        &subject,
                        &predicate,
                        &object,
                        &prov_filter,
                    )
                    .map_err(to_error_data)?;
                serde_json::to_string_pretty(&result)
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))
            }
        }
    }

    #[tool(
        description = "Assert a single triple with required provenance. Subject, predicate, and object are short names resolved to IRIs by domain prefix (use 'vidya:' prefix for base ontology terms). Object defaults to literal string; set literal=false for entity references (named nodes). Provenance (tradition, source, pramana) is required; confidence defaults to 1.0."
    )]
    pub async fn vidya_assert(
        &self,
        Parameters(args): Parameters<AssertArgs>,
    ) -> Result<String, ErrorData> {
        let is_literal = args.literal.unwrap_or(true);
        let confidence = args.confidence.unwrap_or(1.0);

        self.store
            .assert_triple(
                &args.domain,
                &args.subject,
                &args.predicate,
                &args.object,
                is_literal,
                &args.tradition,
                &args.source,
                &args.pramana,
                confidence,
            )
            .map_err(to_error_data)?;

        let triple_count = self.store.triple_count().map_err(to_error_data)?;

        let out = AssertOutput {
            domain: args.domain,
            subject: args.subject,
            predicate: args.predicate,
            object: args.object,
            triple_count,
        };
        serde_json::to_string_pretty(&out)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }

    #[tool(
        description = "Load domain data from Turtle (RDF). Provide domain name plus either inline turtle string or a file path. Triples load into a named graph for the domain."
    )]
    pub async fn vidya_load(
        &self,
        Parameters(args): Parameters<LoadArgs>,
    ) -> Result<String, ErrorData> {
        match (args.turtle, args.path) {
            (Some(turtle), None) => {
                self.store
                    .load_domain(&args.domain, &turtle)
                    .map_err(to_error_data)?;
            }
            (None, Some(path)) => {
                self.store
                    .load_domain_from_file(&args.domain, &path)
                    .map_err(to_error_data)?;
            }
            _ => {
                return Err(ErrorData::invalid_params(
                    "provide exactly one of 'turtle' or 'path'",
                    None,
                ));
            }
        }

        let triple_count = self.store.triple_count().map_err(to_error_data)?;
        let graph_count = self.store.graph_count();

        let out = LoadOutput {
            domain: args.domain,
            triple_count,
            graph_count,
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
             and SPARQL internally. Tools: vidya_health, vidya_load, vidya_query, vidya_assert.",
        )
    }
}
