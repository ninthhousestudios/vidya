use std::collections::HashMap;

use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

use crate::ontology;

#[derive(Debug)]
pub struct SchemaVocab {
    pub entity_names: HashMap<String, Vec<String>>,
    pub type_names: HashMap<String, String>,
    pub predicate_names: HashMap<String, String>,
    pub value_index: HashMap<String, Vec<(String, String)>>,
}

impl SchemaVocab {
    pub fn build(store: &Store, domain: &str) -> Self {
        let graph_iri = ontology::domain_graph_iri(domain);
        let domain_base = ontology::domain_graph_iri(domain);
        let vidya_base = ontology::VIDYA_BASE;
        let rdfs = "http://www.w3.org/2000/01/rdf-schema#";
        let rdf = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";

        let mut entity_names: HashMap<String, Vec<String>> = HashMap::new();
        let mut type_names: HashMap<String, String> = HashMap::new();
        let mut predicate_names: HashMap<String, String> = HashMap::new();
        let mut value_index: HashMap<String, Vec<(String, String)>> = HashMap::new();

        // Collect rdf:type declarations first — we need class IRIs to filter entity labels
        let type_q = format!(
            "SELECT DISTINCT ?cls WHERE {{ \
               GRAPH <{graph_iri}> {{ \
                 ?s <{rdf}type> ?cls . \
                 FILTER(isIRI(?cls)) \
               }} \
             }}"
        );
        let mut class_iris: Vec<String> = Vec::new();
        for cls_iri in select_one_str(store, &type_q) {
            class_iris.push(cls_iri.clone());
            if let Some(local) = local_name(&cls_iri) {
                type_names.insert(local.to_lowercase(), cls_iri.clone());
            }
            // Index by rdfs:label of the class (search in domain graph)
            let cls_label_q = format!(
                "SELECT ?lbl WHERE {{ \
                   GRAPH <{graph_iri}> {{ \
                     <{cls_iri}> <{rdfs}label> ?lbl . \
                   }} \
                 }}"
            );
            for lbl in select_one_str(store, &cls_label_q) {
                type_names.insert(lbl.to_lowercase(), cls_iri.clone());
            }
        }

        // Also pick up rdfs:Class declarations directly
        let class_decl_q = format!(
            "SELECT DISTINCT ?cls WHERE {{ \
               GRAPH <{graph_iri}> {{ \
                 ?cls a <{rdfs}Class> . \
               }} \
             }}"
        );
        for cls_iri in select_one_str(store, &class_decl_q) {
            if !class_iris.contains(&cls_iri) {
                class_iris.push(cls_iri.clone());
            }
            if let Some(local) = local_name(&cls_iri) {
                type_names.entry(local.to_lowercase()).or_insert(cls_iri);
            }
        }

        // Build a SPARQL VALUES clause for class filtering
        let class_filter = if class_iris.is_empty() {
            String::new()
        } else {
            let values: Vec<String> = class_iris.iter().map(|c| format!("<{c}>")).collect();
            format!("FILTER(?s NOT IN ({}))", values.join(", "))
        };

        // Collect entity labels, aliases, western names — excluding class subjects
        let label_q = format!(
            "SELECT ?s ?val WHERE {{ \
               GRAPH <{graph_iri}> {{ \
                 ?s <{rdfs}label>|<{domain_base}alias>|<{domain_base}western> ?val . \
                 FILTER(isIRI(?s)) \
                 {class_filter} \
               }} \
             }}"
        );
        for (iri, val) in select_two_str(store, &label_q) {
            let key = val.to_lowercase();
            entity_names.entry(key).or_default().push(iri.clone());
            if let Some(local) = local_name(&iri) {
                let local_key = local.to_lowercase();
                entity_names.entry(local_key).or_default().push(iri);
            }
        }
        dedup_vecs(&mut entity_names);

        // Collect predicates — both regular triples and RDF-star quoted triples
        let pred_q = format!(
            "SELECT DISTINCT ?p WHERE {{ \
               GRAPH <{graph_iri}> {{ \
                 {{ ?s ?p ?o . }} \
                 UNION \
                 {{ << ?s ?p ?o >> ?_ap ?_ao . }} \
               }} \
               FILTER(isIRI(?p)) \
               FILTER(?p != <{rdf}type>) \
               FILTER(?p != <{rdfs}label>) \
               FILTER(?p != <{rdfs}comment>) \
               FILTER(?p != <{rdfs}range>) \
               FILTER(!STRSTARTS(STR(?p), \"{vidya_base}\")) \
             }}"
        );
        for pred_iri in select_one_str(store, &pred_q) {
            if let Some(local) = local_name(&pred_iri) {
                predicate_names.insert(local.to_lowercase(), pred_iri);
            }
        }

        // Collect literal property values — excluding labels, aliases, western names,
        // and other identity predicates that are already in entity_names
        let val_q = format!(
            "SELECT ?p ?val WHERE {{ \
               GRAPH <{graph_iri}> {{ \
                 ?s ?p ?val . \
                 FILTER(isLiteral(?val)) \
                 FILTER(?p != <{rdfs}label>) \
                 FILTER(?p != <{rdfs}comment>) \
                 FILTER(?p != <{domain_base}alias>) \
                 FILTER(?p != <{domain_base}western>) \
                 FILTER(?p != <{domain_base}sanskritName>) \
               }} \
             }}"
        );
        for (pred_iri, val) in select_two_str(store, &val_q) {
            let key = val.to_lowercase();
            let pred_local = local_name(&pred_iri).unwrap_or_default();
            value_index
                .entry(key)
                .or_default()
                .push((pred_local, val));
        }
        dedup_value_vecs(&mut value_index);

        Self {
            entity_names,
            type_names,
            predicate_names,
            value_index,
        }
    }

    pub fn all_known_tokens(&self) -> Vec<String> {
        let mut tokens: Vec<String> = Vec::new();
        tokens.extend(self.entity_names.keys().cloned());
        tokens.extend(self.type_names.keys().cloned());
        tokens.extend(self.predicate_names.keys().cloned());
        tokens.extend(self.value_index.keys().cloned());
        tokens.sort();
        tokens.dedup();
        tokens
    }
}

fn local_name(iri: &str) -> Option<String> {
    iri.rsplit_once('/').map(|(_, local)| local.to_string())
}

fn select_one_str(store: &Store, sparql: &str) -> Vec<String> {
    let results = match SparqlEvaluator::new()
        .parse_query(sparql)
        .ok()
        .and_then(|q| q.on_store(store).execute().ok())
    {
        Some(r) => r,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    if let QueryResults::Solutions(solutions) = results {
        for row in solutions.flatten() {
            if let Some(Some(val)) = row.values().first() {
                if let Some(s) = term_to_string(val) {
                    out.push(s);
                }
            }
        }
    }
    out
}

fn select_two_str(store: &Store, sparql: &str) -> Vec<(String, String)> {
    let results = match SparqlEvaluator::new()
        .parse_query(sparql)
        .ok()
        .and_then(|q| q.on_store(store).execute().ok())
    {
        Some(r) => r,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    if let QueryResults::Solutions(solutions) = results {
        for row in solutions.flatten() {
            let vals = row.values();
            if vals.len() >= 2 {
                if let (Some(a_term), Some(b_term)) = (&vals[0], &vals[1]) {
                    if let (Some(a), Some(b)) = (term_to_string(a_term), term_to_string(b_term)) {
                        out.push((a, b));
                    }
                }
            }
        }
    }
    out
}

fn term_to_string(term: &oxigraph::model::Term) -> Option<String> {
    match term {
        oxigraph::model::Term::NamedNode(n) => Some(n.as_str().to_string()),
        oxigraph::model::Term::Literal(l) => Some(l.value().to_string()),
        _ => None,
    }
}

fn dedup_vecs(map: &mut HashMap<String, Vec<String>>) {
    for v in map.values_mut() {
        v.sort();
        v.dedup();
    }
}

fn dedup_value_vecs(map: &mut HashMap<String, Vec<(String, String)>>) {
    for v in map.values_mut() {
        v.sort();
        v.dedup();
    }
}
