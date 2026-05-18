use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use std::path::PathBuf;
use vidya_core::{KnowledgeStore, ProvenanceFilter, VidyaError};

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

fn load_jyotish(store: &KnowledgeStore) {
    let seed_path = project_root().join("seeds/jyotish.ttl");
    store.load_domain_from_file("jyotish", &seed_path).unwrap();
}

#[test]
fn describe_surya_returns_properties_and_provenance() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store.describe("jyotish", "surya", &ProvenanceFilter::default()).unwrap();

    assert_eq!(result.label.as_deref(), Some("Sūrya"));
    assert!(result.types.iter().any(|t| t.contains("Graha")));

    let elements: Vec<_> = result
        .properties
        .iter()
        .filter(|p| p.predicate.contains("element"))
        .collect();
    assert_eq!(elements.len(), 1);
    assert_eq!(elements[0].value, "fire");

    assert!(!result.annotated_triples.is_empty());
    let exaltation = result
        .annotated_triples
        .iter()
        .find(|at| at.predicate.contains("exaltedIn"))
        .expect("should have exaltedIn annotation");
    assert!(exaltation.object.contains("mesha"));
    assert!(exaltation.provenance.is_some());
    let prov = exaltation.provenance.as_ref().unwrap();
    assert!(prov.pramana.contains("shabda"));

    // Non-assertedBy annotations (e.g. exaltationDegree) should also be present
    let has_degree = exaltation
        .annotations
        .iter()
        .any(|a| a.predicate.contains("exaltationDegree"));
    assert!(has_degree, "exaltedIn should have exaltationDegree annotation");
}

#[test]
fn search_grahas_fire_element() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store
        .search("jyotish", "Graha", &[("element".into(), "fire".into())], &ProvenanceFilter::default())
        .unwrap();

    let mut names: Vec<&str> = result.entities.iter().map(|e| e.name.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["mangala", "surya"]);
}

#[test]
fn describe_nonexistent_returns_not_found() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store.describe("jyotish", "nonexistent", &ProvenanceFilter::default());
    assert!(matches!(result, Err(VidyaError::NotFound(_))));
}

#[test]
fn search_all_grahas() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store.search("jyotish", "Graha", &[], &ProvenanceFilter::default()).unwrap();
    assert_eq!(result.entities.len(), 9);
}

#[test]
fn describe_rejects_invalid_iri_chars() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store.describe("jyotish", "surya> <http://evil", &ProvenanceFilter::default());
    assert!(matches!(result, Err(VidyaError::InvalidArgument(_))));
}

#[test]
fn search_rejects_invalid_filter_attr() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store.search(
        "jyotish",
        "Graha",
        &[("element> <http://evil".into(), "fire".into())],
        &ProvenanceFilter::default(),
    );
    assert!(matches!(result, Err(VidyaError::InvalidArgument(_))));
}

// ---------------------------------------------------------------------------
// Traverse tests
// ---------------------------------------------------------------------------

#[test]
fn traverse_natural_friend_depth_1() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store
        .traverse("jyotish", "surya", "naturalFriend", 1, &ProvenanceFilter::default())
        .unwrap();

    assert_eq!(result.origin, "surya");
    assert_eq!(result.predicate, "naturalFriend");

    let mut names: Vec<&str> = result.entities.iter().map(|e| e.iri.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["chandra", "guru", "mangala"]);
    assert!(result.entities.iter().all(|e| e.depth == 1));
}

#[test]
fn traverse_natural_friend_depth_2() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store
        .traverse("jyotish", "surya", "naturalFriend", 2, &ProvenanceFilter::default())
        .unwrap();

    let depth1: Vec<&str> = result
        .entities
        .iter()
        .filter(|e| e.depth == 1)
        .map(|e| e.iri.as_str())
        .collect();
    assert_eq!(depth1.len(), 3);

    let mut depth2: Vec<&str> = result
        .entities
        .iter()
        .filter(|e| e.depth == 2)
        .map(|e| e.iri.as_str())
        .collect();
    depth2.sort();
    assert!(depth2.contains(&"budha"), "chandra's friend budha should appear at depth 2, got {depth2:?}");
}

#[test]
fn traverse_max_depth_exceeded() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store.traverse("jyotish", "surya", "naturalFriend", 11, &ProvenanceFilter::default());
    assert!(matches!(result, Err(VidyaError::InvalidArgument(_))));
}

// ---------------------------------------------------------------------------
// Provenance tests
// ---------------------------------------------------------------------------

#[test]
fn provenance_surya_exalted_in_mesha() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store
        .provenance("jyotish", "surya", "exaltedIn", "mesha", &ProvenanceFilter::default())
        .unwrap();

    assert_eq!(result.subject, "surya");
    assert_eq!(result.predicate, "exaltedIn");
    assert_eq!(result.assertions.len(), 1);

    let a = &result.assertions[0];
    assert!(a.tradition.contains("tradition-bphs"));
    assert!(a.pramana.contains("shabda"));
    assert_eq!(a.confidence, "1");
}

#[test]
fn provenance_nonexistent_triple() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store
        .provenance("jyotish", "surya", "exaltedIn", "karka", &ProvenanceFilter::default())
        .unwrap();

    assert!(result.assertions.is_empty());
}

#[test]
fn provenance_with_literal_object() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store
        .provenance("jyotish", "surya", "nature", "malefic", &ProvenanceFilter::default())
        .unwrap();

    assert_eq!(result.assertions.len(), 1);
    assert!(result.assertions[0].tradition.contains("tradition-bphs"));
}

// ---------------------------------------------------------------------------
// Cross-cutting filter tests
// ---------------------------------------------------------------------------

#[test]
fn search_filtered_by_tradition_parasara() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let filter = ProvenanceFilter {
        tradition: Some(vidya_core::ontology::resolve_iri("tradition-parasara", "jyotish")),
        pramana: None,
    };
    let result = store.search("jyotish", "Graha", &[], &filter).unwrap();

    let mut names: Vec<&str> = result.entities.iter().map(|e| e.name.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["guru", "mangala", "shani"]);
}

#[test]
fn describe_filtered_by_pramana_shabda() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let filter = ProvenanceFilter {
        tradition: None,
        pramana: Some(vidya_core::ontology::resolve_iri("vidya:shabda", "jyotish")),
    };
    let result = store.describe("jyotish", "surya", &filter).unwrap();

    assert!(!result.annotated_triples.is_empty());
    assert!(result.annotated_triples.iter().all(|at| at.provenance.is_some()));
}

#[test]
fn traverse_with_tradition_filter() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let filter = ProvenanceFilter {
        tradition: Some(vidya_core::ontology::resolve_iri("tradition-bphs", "jyotish")),
        pramana: None,
    };
    let result = store
        .traverse("jyotish", "surya", "naturalFriend", 1, &filter)
        .unwrap();

    let mut names: Vec<&str> = result.entities.iter().map(|e| e.iri.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["chandra", "guru", "mangala"]);
}

#[test]
fn provenance_with_tradition_filter() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let filter_bphs = ProvenanceFilter {
        tradition: Some(vidya_core::ontology::resolve_iri("tradition-bphs", "jyotish")),
        pramana: None,
    };
    let result = store
        .provenance("jyotish", "surya", "exaltedIn", "mesha", &filter_bphs)
        .unwrap();
    assert_eq!(result.assertions.len(), 1);

    let filter_jaimini = ProvenanceFilter {
        tradition: Some(vidya_core::ontology::resolve_iri("tradition-jaimini", "jyotish")),
        pramana: None,
    };
    let result = store
        .provenance("jyotish", "surya", "exaltedIn", "mesha", &filter_jaimini)
        .unwrap();
    assert!(result.assertions.is_empty());
}

#[test]
fn compose_tradition_and_pramana_filters() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let filter_match = ProvenanceFilter {
        tradition: Some(vidya_core::ontology::resolve_iri("tradition-bphs", "jyotish")),
        pramana: Some(vidya_core::ontology::resolve_iri("vidya:shabda", "jyotish")),
    };
    let result = store.search("jyotish", "Graha", &[], &filter_match).unwrap();
    assert_eq!(result.entities.len(), 9);

    let filter_mismatch = ProvenanceFilter {
        tradition: Some(vidya_core::ontology::resolve_iri("tradition-bphs", "jyotish")),
        pramana: Some(vidya_core::ontology::resolve_iri("vidya:pratyaksha", "jyotish")),
    };
    let result = store.search("jyotish", "Graha", &[], &filter_mismatch).unwrap();
    assert!(result.entities.is_empty());
}
