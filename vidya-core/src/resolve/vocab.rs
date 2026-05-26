use std::collections::HashMap;

use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use serde::Deserialize;

use crate::ontology;

#[derive(Debug, Default, Deserialize)]
pub struct SynonymTable {
    #[serde(default)]
    pub types: HashMap<String, String>,
    #[serde(default)]
    pub predicates: HashMap<String, String>,
}

pub fn parse_synonyms(toml_content: &str) -> Result<SynonymTable, toml::de::Error> {
    toml::from_str(toml_content)
}

#[derive(Debug)]
pub struct SchemaVocab {
    pub entity_names: HashMap<String, Vec<String>>,
    pub type_names: HashMap<String, String>,
    pub predicate_names: HashMap<String, String>,
    pub value_index: HashMap<String, Vec<(String, String)>>,
    /// Maps "{pred_local}\t{value_lowercase}" → Vec<type_iri>
    pub value_types: HashMap<String, Vec<String>>,
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
            if is_infra_type(&cls_iri, vidya_base) {
                continue;
            }
            class_iris.push(cls_iri.clone());
            if let Some(local) = local_name(&cls_iri) {
                type_names.insert(local.to_lowercase(), cls_iri.clone());
            }
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
            if is_infra_type(&cls_iri, vidya_base) {
                continue;
            }
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
               FILTER(!STRSTARTS(STR(?p), \"{rdf}\")) \
               FILTER(!STRSTARTS(STR(?p), \"{rdfs}\")) \
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

        // Build reverse index: (predicate, value) → types that have entities with that property
        let mut value_types: HashMap<String, Vec<String>> = HashMap::new();
        let vt_q = format!(
            "SELECT DISTINCT ?type ?p ?val WHERE {{ \
               GRAPH <{graph_iri}> {{ \
                 ?s <{rdf}type> ?type . \
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
        for (type_iri, pred_iri, val) in select_three_str(store, &vt_q) {
            if is_infra_type(&type_iri, vidya_base) {
                continue;
            }
            let pred_local = local_name(&pred_iri).unwrap_or_default();
            let key = format!("{}\t{}", pred_local.to_lowercase(), val.to_lowercase());
            value_types.entry(key).or_default().push(type_iri);
        }
        for v in value_types.values_mut() {
            v.sort();
            v.dedup();
        }

        Self {
            entity_names,
            type_names,
            predicate_names,
            value_index,
            value_types,
        }
    }

    pub fn apply_synonyms(&mut self, synonyms: &SynonymTable) {
        for (synonym, target) in &synonyms.types {
            let target_lower = target.to_lowercase();
            if let Some(iri) = self.type_names.get(&target_lower).cloned() {
                self.type_names.entry(synonym.clone()).or_insert(iri);
            }
        }
        for (synonym, target) in &synonyms.predicates {
            let target_lower = target.to_lowercase();
            if let Some(iri) = self.predicate_names.get(&target_lower).cloned() {
                self.predicate_names.entry(synonym.clone()).or_insert(iri);
            }
        }
    }

    pub fn types_for_value(&self, pred_local: &str, value: &str) -> &[String] {
        let key = format!("{}\t{}", pred_local.to_lowercase(), value.to_lowercase());
        self.value_types.get(&key).map(|v| v.as_slice()).unwrap_or(&[])
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

fn select_three_str(store: &Store, sparql: &str) -> Vec<(String, String, String)> {
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
            if vals.len() >= 3 {
                if let (Some(a_term), Some(b_term), Some(c_term)) =
                    (&vals[0], &vals[1], &vals[2])
                {
                    if let (Some(a), Some(b), Some(c)) = (
                        term_to_string(a_term),
                        term_to_string(b_term),
                        term_to_string(c_term),
                    ) {
                        out.push((a, b, c));
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

const RDF_NS: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const RDFS_NS: &str = "http://www.w3.org/2000/01/rdf-schema#";
const VIDYA_KEEP: &[&str] = &["Tradition", "Source"];

fn is_infra_type(iri: &str, vidya_base: &str) -> bool {
    if iri.starts_with(RDF_NS) || iri.starts_with(RDFS_NS) {
        return true;
    }
    if let Some(local) = iri.strip_prefix(vidya_base) {
        return !VIDYA_KEEP.contains(&local);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_synonyms_basic() {
        let toml = r#"
[types]
planet = "Graha"
planets = "Graha"

[predicates]
exalted = "exaltedIn"
"#;
        let table = parse_synonyms(toml).unwrap();
        assert_eq!(table.types.get("planet").unwrap(), "Graha");
        assert_eq!(table.types.get("planets").unwrap(), "Graha");
        assert_eq!(table.predicates.get("exalted").unwrap(), "exaltedIn");
    }

    #[test]
    fn parse_synonyms_empty_sections() {
        let table = parse_synonyms("").unwrap();
        assert!(table.types.is_empty());
        assert!(table.predicates.is_empty());
    }

    #[test]
    fn apply_synonyms_extends_type_names() {
        let mut vocab = SchemaVocab {
            entity_names: HashMap::new(),
            type_names: HashMap::from([("graha".to_string(), "urn:Graha".to_string())]),
            predicate_names: HashMap::from([("exaltedin".to_string(), "urn:exaltedIn".to_string())]),
            value_index: HashMap::new(),
            value_types: HashMap::new(),
        };

        let syns = SynonymTable {
            types: HashMap::from([("planet".to_string(), "Graha".to_string())]),
            predicates: HashMap::from([("exalted".to_string(), "exaltedIn".to_string())]),
        };

        vocab.apply_synonyms(&syns);

        assert_eq!(vocab.type_names.get("planet").unwrap(), "urn:Graha");
        assert_eq!(vocab.predicate_names.get("exalted").unwrap(), "urn:exaltedIn");
    }

    #[test]
    fn apply_synonyms_skips_unknown_targets() {
        let mut vocab = SchemaVocab {
            entity_names: HashMap::new(),
            type_names: HashMap::new(),
            predicate_names: HashMap::new(),
            value_index: HashMap::new(),
            value_types: HashMap::new(),
        };

        let syns = SynonymTable {
            types: HashMap::from([("planet".to_string(), "NonExistent".to_string())]),
            predicates: HashMap::new(),
        };

        vocab.apply_synonyms(&syns);
        assert!(!vocab.type_names.contains_key("planet"));
    }
}
