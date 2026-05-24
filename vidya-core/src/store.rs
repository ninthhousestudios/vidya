use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::NamedNodeRef;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::error::{Result, VidyaError};
use crate::ontology;
use crate::resolve::SchemaVocab;
use crate::vsa::{EntityIndex, Hrr};

pub struct ResolveContext {
    pub vocab: SchemaVocab,
    pub vsa: EntityIndex<Hrr>,
}

pub struct KnowledgeStore {
    store: Store,
    resolve_cache: RwLock<HashMap<String, Arc<ResolveContext>>>,
    cache_generation: AtomicU64,
}

impl KnowledgeStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let store = Store::open(path)?;
        let ks = Self { store, resolve_cache: RwLock::new(HashMap::new()), cache_generation: AtomicU64::new(0) };
        ks.ensure_base_ontology()?;
        Ok(ks)
    }

    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self> {
        let store = Store::open_read_only(path)?;
        Ok(Self { store, resolve_cache: RwLock::new(HashMap::new()), cache_generation: AtomicU64::new(0) })
    }

    pub fn new_memory() -> Result<Self> {
        let store = Store::new()?;
        let ks = Self { store, resolve_cache: RwLock::new(HashMap::new()), cache_generation: AtomicU64::new(0) };
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

    pub fn resolve_context(&self, domain: &str) -> Arc<ResolveContext> {
        if let Some(ctx) = self.resolve_cache.read().unwrap().get(domain) {
            return ctx.clone();
        }
        let gen_before = self.cache_generation.load(Ordering::Acquire);
        let vocab = SchemaVocab::build(&self.store, domain);
        let vsa = EntityIndex::build(Hrr::new(1024), &self.store, domain);
        let ctx = Arc::new(ResolveContext { vocab, vsa });
        if self.cache_generation.load(Ordering::Acquire) == gen_before {
            self.resolve_cache.write().unwrap().insert(domain.to_string(), ctx.clone());
        }
        ctx
    }

    pub fn load_domain(&self, name: &str, turtle: &str) -> Result<()> {
        let graph_iri = ontology::domain_graph_iri(name);
        let graph = NamedNodeRef::new(&graph_iri)
            .map_err(|e| VidyaError::InvalidArgument(e.to_string()))?;
        let parser = RdfParser::from_format(RdfFormat::Turtle)
            .with_default_graph(graph)
            .without_named_graphs();
        self.store.load_from_reader(parser, turtle.as_bytes())?;
        self.cache_generation.fetch_add(1, Ordering::Release);
        self.resolve_cache.write().unwrap().remove(name);
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

    pub fn describe(
        &self,
        domain: &str,
        subject: &str,
        filter: &crate::query::ProvenanceFilter,
    ) -> Result<crate::query::DescribeResult> {
        crate::query::describe(self, domain, subject, filter)
    }

    pub fn search(
        &self,
        domain: &str,
        kind: &str,
        filters: &[(String, String)],
        prov_filter: &crate::query::ProvenanceFilter,
    ) -> Result<crate::query::SearchResult> {
        crate::query::search(self, domain, kind, filters, prov_filter)
    }

    pub fn traverse(
        &self,
        domain: &str,
        subject: &str,
        predicate: &str,
        depth: u32,
        filter: &crate::query::ProvenanceFilter,
    ) -> Result<crate::query::TraverseResult> {
        crate::query::traverse(self, domain, subject, predicate, depth, filter)
    }

    pub fn provenance(
        &self,
        domain: &str,
        subject: &str,
        predicate: &str,
        object: &str,
        filter: &crate::query::ProvenanceFilter,
    ) -> Result<crate::query::ProvenanceResult> {
        crate::query::provenance(self, domain, subject, predicate, object, filter)
    }

    pub fn assert_triple(
        &self,
        domain: &str,
        subject: &str,
        predicate: &str,
        object: &str,
        is_literal: bool,
        tradition: &str,
        source: &str,
        pramana: &str,
        confidence: f64,
    ) -> Result<()> {
        if tradition.is_empty() || source.is_empty() || pramana.is_empty() {
            return Err(VidyaError::InvalidArgument(
                "tradition, source, and pramana are required".into(),
            ));
        }

        let subj_iri = ontology::resolve_iri(subject, domain);
        let pred_iri = ontology::resolve_iri(predicate, domain);
        let trad_iri = ontology::resolve_iri(tradition, domain);
        let src_iri = ontology::resolve_iri(source, domain);
        let pram_iri = ontology::resolve_iri(pramana, domain);

        for iri in [&subj_iri, &pred_iri, &trad_iri, &src_iri, &pram_iri] {
            NamedNodeRef::new(iri)
                .map_err(|_| VidyaError::InvalidArgument(format!("invalid IRI: {iri}")))?;
        }

        let obj_term = if is_literal {
            let escaped = object
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n");
            format!("\"{escaped}\"")
        } else {
            let obj_iri = ontology::resolve_iri(object, domain);
            NamedNodeRef::new(&obj_iri)
                .map_err(|_| VidyaError::InvalidArgument(format!("invalid IRI: {obj_iri}")))?;
            format!("<{obj_iri}>")
        };

        let vidya_base = ontology::VIDYA_BASE;
        let turtle = format!(
            "@prefix vidya: <{vidya_base}> .\n\
             @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n\
             \n\
             << <{subj_iri}> <{pred_iri}> {obj_term} >>\n\
                 vidya:assertedBy [\n\
                     vidya:tradition  <{trad_iri}> ;\n\
                     vidya:source     <{src_iri}> ;\n\
                     vidya:pramana    <{pram_iri}> ;\n\
                     vidya:confidence \"{confidence}\"^^xsd:float\n\
                 ] .\n"
        );

        self.load_domain(domain, &turtle)
    }
}
