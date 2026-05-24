use std::collections::{HashMap, HashSet};

use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

use super::{VsaOps, fnv1a};

pub struct EntityIndex<A: VsaOps> {
    ops: A,
    entity_vectors: HashMap<String, A::Vector>,
    symbol_vectors: HashMap<String, A::Vector>,
}

impl<A: VsaOps> EntityIndex<A> {
    pub fn build(ops: A, store: &Store, domain: &str) -> Self {
        let graph_iri = crate::ontology::domain_graph_iri(domain);

        // Collect both regular triples and RDF-star quoted triples via SPARQL
        let sparql = format!(
            "SELECT ?s ?p ?o WHERE {{ \
               GRAPH <{graph_iri}> {{ \
                 {{ ?s ?p ?o . FILTER(isIRI(?s)) }} \
                 UNION \
                 {{ << ?s ?p ?o >> ?_ap ?_ao . FILTER(isIRI(?s)) }} \
               }} \
             }}"
        );

        let mut triples_by_subject: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut all_iris: HashSet<String> = HashSet::new();

        let results = SparqlEvaluator::new()
            .parse_query(&sparql)
            .expect("valid SPARQL")
            .on_store(store)
            .execute()
            .expect("query execution");

        if let QueryResults::Solutions(solutions) = results {
            for row in solutions {
                let row = match row {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let subject = match row.get("s") {
                    Some(s) => iri_str(s),
                    None => continue,
                };
                let predicate = match row.get("p") {
                    Some(p) => iri_str(p),
                    None => continue,
                };
                let object = match row.get("o") {
                    Some(o) => term_str(o),
                    None => continue,
                };
                let (subject, predicate, object) = match (subject, predicate, object) {
                    (Some(s), Some(p), Some(o)) => (s, p, o),
                    _ => continue,
                };

                all_iris.insert(subject.clone());
                all_iris.insert(predicate.clone());
                all_iris.insert(object.clone());

                triples_by_subject
                    .entry(subject)
                    .or_default()
                    .push((predicate, object));
            }
        }

        let mut symbol_vectors: HashMap<String, A::Vector> = HashMap::new();
        for iri in &all_iris {
            let seed = fnv1a(iri.as_bytes());
            symbol_vectors.insert(iri.clone(), ops.random_vector(seed));
        }

        let mut entity_vectors: HashMap<String, A::Vector> = HashMap::new();
        for (subject, triples) in &triples_by_subject {
            if triples.is_empty() {
                continue;
            }
            let bound_pairs: Vec<A::Vector> = triples
                .iter()
                .filter_map(|(pred, obj)| {
                    let pred_vec = symbol_vectors.get(pred)?;
                    let obj_vec = symbol_vectors.get(obj)?;
                    Some(ops.bind(pred_vec, obj_vec))
                })
                .collect();
            if !bound_pairs.is_empty() {
                entity_vectors.insert(subject.clone(), ops.bundle(&bound_pairs));
            }
        }

        Self {
            ops,
            entity_vectors,
            symbol_vectors,
        }
    }

    pub fn similar(&self, iri: &str, top_k: usize) -> Vec<(String, f64)> {
        let query_vec = match self.entity_vectors.get(iri) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let mut scored: Vec<(String, f64)> = self
            .entity_vectors
            .iter()
            .filter(|(k, _)| k.as_str() != iri)
            .map(|(k, v)| (k.clone(), self.ops.similarity(query_vec, v)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(top_k);
        scored
    }

    pub fn unbind_query(&self, subject_iri: &str, predicate_iri: &str, top_k: usize) -> Vec<(String, f64)> {
        let subject_vec = match self.entity_vectors.get(subject_iri) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let predicate_vec = match self.symbol_vectors.get(predicate_iri) {
            Some(v) => v,
            None => return Vec::new(),
        };

        let recovered = self.ops.unbind(subject_vec, predicate_vec);

        let mut scored: Vec<(String, f64)> = self
            .symbol_vectors
            .iter()
            .map(|(k, v)| (k.clone(), self.ops.similarity(&recovered, v)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(top_k);
        scored
    }

    pub fn entity_count(&self) -> usize {
        self.entity_vectors.len()
    }

    pub fn symbol_count(&self) -> usize {
        self.symbol_vectors.len()
    }

    pub fn entity_iris(&self) -> Vec<&str> {
        self.entity_vectors.keys().map(String::as_str).collect()
    }

    pub fn entity_similarity(&self, a: &str, b: &str) -> Option<f64> {
        let va = self.entity_vectors.get(a)?;
        let vb = self.entity_vectors.get(b)?;
        Some(self.ops.similarity(va, vb))
    }
}

fn iri_str(term: &oxigraph::model::Term) -> Option<String> {
    match term {
        oxigraph::model::Term::NamedNode(n) => Some(n.as_str().to_string()),
        _ => None,
    }
}

fn term_str(term: &oxigraph::model::Term) -> Option<String> {
    match term {
        oxigraph::model::Term::NamedNode(n) => Some(n.as_str().to_string()),
        oxigraph::model::Term::Literal(l) => Some(l.value().to_string()),
        _ => None,
    }
}
