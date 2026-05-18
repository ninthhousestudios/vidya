use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use vidya_core::KnowledgeStore;

#[test]
fn new_memory_store_loads_base_ontology() {
    let store = KnowledgeStore::new_memory().unwrap();
    assert!(store.triple_count().unwrap() > 0);
}

#[test]
fn base_ontology_has_six_pramana_instances() {
    let store = KnowledgeStore::new_memory().unwrap();
    let query = r#"
        PREFIX vidya: <http://vidya.ninthhouse.studio/ontology/>
        SELECT (COUNT(?p) AS ?count) WHERE { ?p a vidya:Pramana }
    "#;
    let results = SparqlEvaluator::new()
        .parse_query(query)
        .unwrap()
        .on_store(store.inner())
        .execute()
        .unwrap();
    if let QueryResults::Solutions(solutions) = results {
        let row = solutions.into_iter().next().unwrap().unwrap();
        let count: i64 = row
            .get("count")
            .unwrap()
            .to_string()
            .split('"')
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(count, 6);
    } else {
        panic!("expected SELECT results");
    }
}

#[test]
fn base_ontology_has_four_classes() {
    let store = KnowledgeStore::new_memory().unwrap();
    let query = r#"
        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
        SELECT (COUNT(?c) AS ?count) WHERE { ?c a rdfs:Class }
    "#;
    let results = SparqlEvaluator::new()
        .parse_query(query)
        .unwrap()
        .on_store(store.inner())
        .execute()
        .unwrap();
    if let QueryResults::Solutions(solutions) = results {
        let row = solutions.into_iter().next().unwrap().unwrap();
        let count: i64 = row
            .get("count")
            .unwrap()
            .to_string()
            .split('"')
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(count, 4);
    } else {
        panic!("expected SELECT results");
    }
}

#[test]
fn graph_count_zero_before_loading_domain() {
    let store = KnowledgeStore::new_memory().unwrap();
    assert_eq!(store.graph_count(), 0);
}

#[test]
fn domains_empty_before_loading() {
    let store = KnowledgeStore::new_memory().unwrap();
    assert!(store.domains().is_empty());
}

#[test]
fn resolve_iri_domain_prefix() {
    assert_eq!(
        vidya_core::ontology::resolve_iri("surya", "jyotish"),
        "http://vidya.ninthhouse.studio/domain/jyotish/surya"
    );
}

#[test]
fn resolve_iri_vidya_prefix() {
    assert_eq!(
        vidya_core::ontology::resolve_iri("vidya:pratyaksha", "jyotish"),
        "http://vidya.ninthhouse.studio/ontology/pratyaksha"
    );
}

#[test]
fn persistent_store_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test-store");

    let count_before = {
        let store = KnowledgeStore::open(&path).unwrap();
        store.triple_count().unwrap()
    };

    let count_after = {
        let store = KnowledgeStore::open(&path).unwrap();
        store.triple_count().unwrap()
    };

    assert!(count_before > 0);
    assert_eq!(count_before, count_after);
}
