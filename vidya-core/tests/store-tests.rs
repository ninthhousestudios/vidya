use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use std::path::PathBuf;
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

#[test]
fn load_domain_creates_named_graph() {
    let store = KnowledgeStore::new_memory().unwrap();
    let ttl = r#"
        @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
        <http://vidya.ninthhouse.studio/domain/test/foo> rdfs:label "Foo" .
    "#;
    store.load_domain("test", ttl).unwrap();
    assert_eq!(store.graph_count(), 1);
    assert_eq!(store.domains(), vec!["test"]);
}

#[test]
fn load_domain_triples_in_correct_graph() {
    let store = KnowledgeStore::new_memory().unwrap();
    let ttl = r#"
        @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
        <http://vidya.ninthhouse.studio/domain/test/foo> rdfs:label "Foo" .
    "#;
    store.load_domain("test", ttl).unwrap();
    let query = r#"
        SELECT ?label WHERE {
            GRAPH <http://vidya.ninthhouse.studio/domain/test/> {
                <http://vidya.ninthhouse.studio/domain/test/foo> <http://www.w3.org/2000/01/rdf-schema#label> ?label
            }
        }
    "#;
    let results = SparqlEvaluator::new()
        .parse_query(query)
        .unwrap()
        .on_store(store.inner())
        .execute()
        .unwrap();
    if let QueryResults::Solutions(solutions) = results {
        let row = solutions.into_iter().next().unwrap().unwrap();
        let label = row.get("label").unwrap().to_string();
        assert!(label.contains("Foo"));
    } else {
        panic!("expected SELECT results");
    }
}

#[test]
fn load_domain_from_file_missing_file() {
    let store = KnowledgeStore::new_memory().unwrap();
    let result = store.load_domain_from_file("test", "/nonexistent/path.ttl");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, vidya_core::VidyaError::Io(_)));
}

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

#[test]
fn load_jyotish_seed() {
    let store = KnowledgeStore::new_memory().unwrap();
    let seed_path = project_root().join("seeds/jyotish.ttl");
    store.load_domain_from_file("jyotish", &seed_path).unwrap();

    assert!(store.triple_count().unwrap() > 50);
    assert!(store.domains().contains(&"jyotish".to_string()));

    // Query surya by subject IRI
    let q_entity = r#"
        SELECT ?label WHERE {
            GRAPH <http://vidya.ninthhouse.studio/domain/jyotish/> {
                <http://vidya.ninthhouse.studio/domain/jyotish/surya>
                    <http://www.w3.org/2000/01/rdf-schema#label> ?label
            }
        }
    "#;
    let results = SparqlEvaluator::new()
        .parse_query(q_entity)
        .unwrap()
        .on_store(store.inner())
        .execute()
        .unwrap();
    if let QueryResults::Solutions(solutions) = results {
        let row = solutions.into_iter().next().expect("surya should exist").unwrap();
        let label = row.get("label").unwrap().to_string();
        assert!(label.contains("Sūrya"), "expected Sūrya, got {label}");
    } else {
        panic!("expected SELECT results");
    }

    // Query RDF-star provenance annotation
    let q_prov = r#"
        SELECT ?assertion WHERE {
            GRAPH <http://vidya.ninthhouse.studio/domain/jyotish/> {
                << <http://vidya.ninthhouse.studio/domain/jyotish/surya>
                   <http://vidya.ninthhouse.studio/domain/jyotish/exaltedIn>
                   <http://vidya.ninthhouse.studio/domain/jyotish/mesha> >>
                    <http://vidya.ninthhouse.studio/ontology/assertedBy> ?assertion
            }
        }
    "#;
    let results = SparqlEvaluator::new()
        .parse_query(q_prov)
        .unwrap()
        .on_store(store.inner())
        .execute()
        .unwrap();
    if let QueryResults::Solutions(solutions) = results {
        let row = solutions.into_iter().next().expect("provenance annotation should exist").unwrap();
        assert!(row.get("assertion").is_some());
    } else {
        panic!("expected SELECT results");
    }
}
