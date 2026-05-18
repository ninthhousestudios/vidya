use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::NamedNodeRef;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use std::path::Path;

use crate::error::{Result, VidyaError};
use crate::ontology;

pub struct KnowledgeStore {
    store: Store,
}

impl KnowledgeStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let store = Store::open(path)?;
        let ks = Self { store };
        ks.ensure_base_ontology()?;
        Ok(ks)
    }

    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self> {
        let store = Store::open_read_only(path)?;
        Ok(Self { store })
    }

    pub fn new_memory() -> Result<Self> {
        let store = Store::new()?;
        let ks = Self { store };
        ks.ensure_base_ontology()?;
        Ok(ks)
    }

    pub fn inner(&self) -> &Store {
        &self.store
    }

    fn ensure_base_ontology(&self) -> Result<()> {
        if self.base_ontology_complete()? {
            return Ok(());
        }
        // (Re)load — safe because RDF triples are a set; duplicates are no-ops.
        self.store
            .load_from_reader(RdfFormat::Turtle, ontology::VIDYA_TTL.as_bytes())?;
        tracing::info!(
            triples = self.store.len().unwrap_or(0),
            version = ontology::BASE_ONTOLOGY_VERSION,
            "loaded base ontology"
        );
        if !self.base_ontology_complete()? {
            return Err(VidyaError::Internal(
                "base ontology failed completeness check after loading".into(),
            ));
        }
        Ok(())
    }

    fn base_ontology_complete(&self) -> Result<bool> {
        let query = format!(
            "SELECT \
               (COUNT(DISTINCT ?p) AS ?pcount) \
             WHERE {{ \
               ?p a <{base}Pramana> . \
               <{base}BaseOntology> <{base}version> \"{ver}\" \
             }}",
            base = ontology::VIDYA_BASE,
            ver = ontology::BASE_ONTOLOGY_VERSION,
        );
        match SparqlEvaluator::new()
            .parse_query(&query)
            .map_err(|e| VidyaError::Internal(e.to_string()))?
            .on_store(&self.store)
            .execute()?
        {
            QueryResults::Solutions(solutions) => {
                if let Some(Ok(row)) = solutions.into_iter().next() {
                    let count: i64 = row
                        .get("pcount")
                        .and_then(|t| t.to_string().split('"').nth(1).map(String::from))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    Ok(count == ontology::EXPECTED_PRAMANA_COUNT)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }

    pub fn triple_count(&self) -> Result<usize> {
        Ok(self.store.len()?)
    }

    pub fn graph_count(&self) -> usize {
        self.store.named_graphs().count()
    }

    pub fn domains(&self) -> Vec<String> {
        self.store
            .named_graphs()
            .filter_map(|g| g.ok())
            .filter_map(|g| match g {
                oxigraph::model::NamedOrBlankNode::NamedNode(n) => {
                    let iri = n.as_str();
                    iri.strip_prefix(ontology::DOMAIN_BASE)
                        .and_then(|s| s.strip_suffix('/'))
                        .map(|s| s.to_string())
                }
                _ => None,
            })
            .collect()
    }

    pub fn load_domain(&self, name: &str, turtle: &str) -> Result<()> {
        let graph_iri = ontology::domain_graph_iri(name);
        let graph = NamedNodeRef::new(&graph_iri)
            .map_err(|e| VidyaError::InvalidArgument(e.to_string()))?;
        let parser = RdfParser::from_format(RdfFormat::Turtle)
            .with_default_graph(graph)
            .without_named_graphs();
        self.store.load_from_reader(parser, turtle.as_bytes())?;
        tracing::info!(
            domain = name,
            triples = self.store.len().unwrap_or(0),
            "loaded domain"
        );
        Ok(())
    }

    pub fn load_domain_from_file(&self, name: &str, path: impl AsRef<Path>) -> Result<()> {
        let turtle = std::fs::read_to_string(path.as_ref())?;
        self.load_domain(name, &turtle)
    }
}
