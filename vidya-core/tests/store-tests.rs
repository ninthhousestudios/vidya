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
    assert!(!exaltation.provenance.is_empty());
    let prov = &exaltation.provenance[0];
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
    assert!(result.annotated_triples.iter().all(|at| !at.provenance.is_empty()));
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

// ---------------------------------------------------------------------------
// Assert tests
// ---------------------------------------------------------------------------

#[test]
fn assert_triple_literal_round_trips_via_describe() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    store
        .assert_triple(
            "jyotish",
            "surya",
            "customNote",
            "test value",
            true,
            "tradition-bphs",
            "source-bphs",
            "vidya:shabda",
            0.9,
        )
        .unwrap();

    let result = store
        .describe("jyotish", "surya", &ProvenanceFilter::default())
        .unwrap();

    let note = result
        .annotated_triples
        .iter()
        .find(|at| at.predicate.contains("customNote") && at.object == "test value")
        .expect("should have customNote annotated triple");

    assert!(!note.provenance.is_empty(), "should have provenance");
    let prov = &note.provenance[0];
    assert!(prov.tradition.contains("tradition-bphs"));
    assert!(prov.source.contains("source-bphs"));
    assert!(prov.pramana.contains("shabda"));
    assert!(prov.confidence.contains("0.9"));
}

#[test]
fn assert_triple_entity_object_round_trips_via_provenance() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    store
        .assert_triple(
            "jyotish",
            "surya",
            "aspectedBy",
            "guru",
            false,
            "tradition-bphs",
            "source-bphs",
            "vidya:shabda",
            1.0,
        )
        .unwrap();

    let result = store
        .provenance("jyotish", "surya", "aspectedBy", "guru", &ProvenanceFilter::default())
        .unwrap();

    assert_eq!(result.assertions.len(), 1);
    assert!(result.assertions[0].tradition.contains("tradition-bphs"));
}

#[test]
fn assert_triple_rejects_empty_tradition() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store.assert_triple(
        "jyotish", "surya", "karaka", "soul", true,
        "", "source-bphs", "vidya:shabda", 1.0,
    );
    assert!(matches!(result, Err(VidyaError::InvalidArgument(_))));
}

#[test]
fn assert_triple_rejects_empty_source() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store.assert_triple(
        "jyotish", "surya", "karaka", "soul", true,
        "tradition-bphs", "", "vidya:shabda", 1.0,
    );
    assert!(matches!(result, Err(VidyaError::InvalidArgument(_))));
}

#[test]
fn assert_triple_rejects_empty_pramana() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store.assert_triple(
        "jyotish", "surya", "karaka", "soul", true,
        "tradition-bphs", "source-bphs", "", 1.0,
    );
    assert!(matches!(result, Err(VidyaError::InvalidArgument(_))));
}

#[test]
fn assert_triple_default_confidence() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    store
        .assert_triple(
            "jyotish",
            "surya",
            "karaka",
            "atman",
            true,
            "tradition-bphs",
            "source-bphs",
            "vidya:shabda",
            1.0,
        )
        .unwrap();

    let result = store
        .provenance("jyotish", "surya", "karaka", "atman", &ProvenanceFilter::default())
        .unwrap();

    assert_eq!(result.assertions.len(), 1);
    assert!(result.assertions[0].confidence.contains("1"));
}

#[test]
fn provenance_handles_invalid_object_iri_gracefully() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let result = store
        .provenance("jyotish", "surya", "exaltedIn", "mesha> <http://evil", &ProvenanceFilter::default())
        .unwrap();
    assert!(result.assertions.is_empty());
}

#[test]
fn search_rejects_invalid_provenance_filter_iri() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let filter = ProvenanceFilter {
        tradition: Some("http://example.org/bad>iri".into()),
        pramana: None,
    };
    let result = store.search("jyotish", "Graha", &[], &filter);
    assert!(matches!(result, Err(VidyaError::InvalidArgument(_))));
}

#[test]
fn describe_with_mismatching_filter_excludes_unmatched_facts() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let filter = ProvenanceFilter {
        tradition: Some(vidya_core::ontology::resolve_iri("tradition-jaimini", "jyotish")),
        pramana: None,
    };
    let result = store.describe("jyotish", "surya", &filter).unwrap();

    assert!(result.label.is_some(), "identity info should still be present");
    assert!(result.properties.is_empty(), "filtered describe should exclude unmatched properties");
    assert!(result.annotated_triples.is_empty(), "filtered describe should exclude unmatched annotated triples");
}

// ═══════════════════════════════════════════════════════════════════
// Ayurveda seed tests
// ═══════════════════════════════════════════════════════════════════

fn load_ayurveda(store: &KnowledgeStore) {
    let seed_path = project_root().join("seeds/ayurveda.ttl");
    store.load_domain_from_file("ayurveda", &seed_path).unwrap();
}

#[test]
fn load_ayurveda_seed() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_ayurveda(&store);
    assert!(store.triple_count().unwrap() > 100);
}

#[test]
fn search_all_dravyas() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_ayurveda(&store);

    let result = store
        .search("ayurveda", "Dravya", &[], &ProvenanceFilter::default())
        .unwrap();
    assert_eq!(result.entities.len(), 15);
}

#[test]
fn describe_ashwagandha() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_ayurveda(&store);

    let result = store
        .describe("ayurveda", "ashwagandha", &ProvenanceFilter::default())
        .unwrap();

    assert_eq!(result.label.as_deref(), Some("ashwagandha"));
    assert!(result.types.iter().any(|t| t.contains("Dravya")));

    let rasa_props: Vec<_> = result
        .properties
        .iter()
        .filter(|p| p.predicate.contains("hasRasa"))
        .collect();
    assert!(!rasa_props.is_empty(), "ashwagandha should have rasa properties");

    assert!(!result.annotated_triples.is_empty());
    let veerya = result
        .annotated_triples
        .iter()
        .find(|at| at.predicate.contains("hasVeerya"))
        .expect("should have hasVeerya annotated triple");
    assert!(veerya.object.contains("ushna"));
    assert!(!veerya.provenance.is_empty());
}

#[test]
fn describe_multi_provenance() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_ayurveda(&store);

    let result = store
        .describe("ayurveda", "ashwagandha", &ProvenanceFilter::default())
        .unwrap();

    let veerya = result
        .annotated_triples
        .iter()
        .find(|at| at.predicate.contains("hasVeerya") && at.object.contains("ushna"))
        .expect("should have hasVeerya ushna");

    assert!(
        veerya.provenance.len() >= 2,
        "ashwagandha hasVeerya ushna should have provenance from both Charaka and Bhavaprakasha, got {}",
        veerya.provenance.len()
    );

    let sources: Vec<&str> = veerya.provenance.iter().map(|p| p.source.as_str()).collect();
    assert!(sources.iter().any(|s| s.contains("charaka")), "should have Charaka source");
    assert!(sources.iter().any(|s| s.contains("bhavaprakasha")), "should have Bhavaprakasha source");
}

#[test]
fn search_dravya_by_dosha_entity_filter() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_ayurveda(&store);

    let result = store
        .search(
            "ayurveda",
            "Dravya",
            &[("pacifiesDosha".into(), "vata".into())],
            &ProvenanceFilter::default(),
        )
        .unwrap();

    let names: Vec<&str> = result.entities.iter().map(|e| e.label.as_str()).collect();
    assert!(names.contains(&"ashwagandha"), "ashwagandha pacifies vata");
    assert!(names.contains(&"haritaki"), "haritaki pacifies vata");
    assert!(result.entities.len() >= 5, "many dravyas pacify vata");
}

#[test]
fn search_rasa_aggravates_kapha() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_ayurveda(&store);

    let result = store
        .search(
            "ayurveda",
            "Rasa",
            &[("aggravatesDosha".into(), "kapha".into())],
            &ProvenanceFilter::default(),
        )
        .unwrap();

    let mut names: Vec<&str> = result.entities.iter().map(|e| e.label.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["amla", "lavana", "madhura"]);
}

#[test]
fn provenance_veerya_disagreement() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_ayurveda(&store);

    let result = store
        .describe("ayurveda", "pippali", &ProvenanceFilter::default())
        .unwrap();

    let veerya_triples: Vec<_> = result
        .annotated_triples
        .iter()
        .filter(|at| at.predicate.contains("hasVeerya"))
        .collect();

    assert!(
        veerya_triples.len() >= 2,
        "pippali should have at least 2 veerya triples (sheeta from Charaka, ushna from Sushruta), got {}",
        veerya_triples.len()
    );

    let sheeta = veerya_triples.iter().find(|at| at.object.contains("sheeta"));
    let ushna = veerya_triples.iter().find(|at| at.object.contains("ushna"));
    assert!(sheeta.is_some(), "should have sheeta veerya from Charaka");
    assert!(ushna.is_some(), "should have ushna veerya from Sushruta/Bhavaprakasha");

    let sheeta_src = &sheeta.unwrap().provenance[0].source;
    assert!(sheeta_src.contains("charaka"), "sheeta veerya should be from Charaka");

    let ushna_sources: Vec<&str> = ushna.unwrap().provenance.iter().map(|p| p.source.as_str()).collect();
    assert!(ushna_sources.iter().any(|s| s.contains("sushruta")), "ushna veerya should include Sushruta");
}

#[test]
fn search_literal_filter_with_spaces() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_ayurveda(&store);

    let result = store
        .search(
            "ayurveda",
            "Dravya",
            &[("commonName".into(), "black pepper".into())],
            &ProvenanceFilter::default(),
        )
        .unwrap();

    assert_eq!(result.entities.len(), 1);
    assert!(result.entities[0].label.contains("maricha"));
}

#[test]
fn search_contested_veerya_finds_pippali() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_ayurveda(&store);

    let sheeta_result = store
        .search(
            "ayurveda",
            "Dravya",
            &[("hasVeerya".into(), "sheeta".into())],
            &ProvenanceFilter::default(),
        )
        .unwrap();

    let names: Vec<&str> = sheeta_result.entities.iter().map(|e| e.label.as_str()).collect();
    assert!(names.contains(&"pippali"), "pippali should appear in sheeta veerya search (Charaka classification)");
}

#[test]
fn provenance_coverage_jyotish() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let cov = store.provenance_coverage("jyotish").unwrap();
    assert!(cov.total > 0, "jyotish domain should have triples");
    assert!(cov.annotated > 0, "jyotish domain should have annotated triples");
    assert!(cov.coverage > 0.0 && cov.coverage <= 1.0, "coverage={}", cov.coverage);
}

#[test]
fn provenance_coverage_empty_domain() {
    let store = KnowledgeStore::new_memory().unwrap();

    let cov = store.provenance_coverage("nonexistent").unwrap();
    assert_eq!(cov.total, 0);
    assert_eq!(cov.annotated, 0);
    assert_eq!(cov.coverage, 0.0);
}

#[test]
fn type_summary_jyotish() {
    let store = KnowledgeStore::new_memory().unwrap();
    load_jyotish(&store);

    let types = store.type_summary("jyotish").unwrap();
    assert!(!types.is_empty(), "jyotish should have entity types");

    let graha = types.iter().find(|t| t.name == "Graha");
    assert!(graha.is_some(), "should contain Graha type");
    assert_eq!(graha.unwrap().count, 9);

    let rashi = types.iter().find(|t| t.name == "Rashi");
    assert!(rashi.is_some(), "should contain Rashi type");
    assert_eq!(rashi.unwrap().count, 12);
}

#[test]
fn type_summary_empty_domain() {
    let store = KnowledgeStore::new_memory().unwrap();

    let types = store.type_summary("nonexistent").unwrap();
    assert!(types.is_empty());
}
