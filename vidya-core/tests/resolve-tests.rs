use std::path::PathBuf;
use vidya_core::resolve::{self, QueryMode, ResolvedQuery};
use vidya_core::KnowledgeStore;

fn load_jyotish() -> KnowledgeStore {
    let store = KnowledgeStore::new_memory().unwrap();
    let seed = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("seeds")
        .join("jyotish.ttl");
    store
        .load_domain_from_file("jyotish", &seed)
        .unwrap();
    store
}

fn iri_ends_with(iri: &str, suffix: &str) -> bool {
    iri.ends_with(&format!("/{suffix}"))
}

// ── Vocabulary index tests ──

#[test]
fn vocab_has_entities() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");
    assert!(
        vocab.entity_names.contains_key("mars")
            || vocab.entity_names.contains_key("sun")
            || vocab.entity_names.contains_key("surya"),
        "vocab should contain known entities"
    );
}

#[test]
fn vocab_has_types() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");
    assert!(
        vocab.type_names.contains_key("graha"),
        "vocab should contain Graha type"
    );
}

#[test]
fn vocab_has_predicates() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");
    assert!(
        vocab.predicate_names.contains_key("exaltedin")
            || vocab.predicate_names.contains_key("rules"),
        "vocab should contain known predicates"
    );
}

#[test]
fn vocab_has_property_values() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");
    assert!(
        vocab.value_index.contains_key("fire"),
        "vocab should contain 'fire' as a property value"
    );
}

// ── Describe mode ──

#[test]
fn describe_mars_resolves_to_mangala() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");
    let vsa = resolve::build_vsa(&store, "jyotish");

    let report = resolve::resolve(QueryMode::Describe, "mars", &vocab, Some(&vsa), "jyotish")
        .expect("should resolve");

    match &report.query {
        ResolvedQuery::Describe { subject_iri } => {
            assert!(
                iri_ends_with(subject_iri, "mangala"),
                "expected mangala, got {subject_iri}"
            );
        }
        other => panic!("expected Describe, got {other:?}"),
    }
}

#[test]
fn describe_sun_resolves_to_surya() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");

    let report = resolve::resolve(QueryMode::Describe, "Sun", &vocab, None, "jyotish")
        .expect("should resolve");

    match &report.query {
        ResolvedQuery::Describe { subject_iri } => {
            assert!(
                iri_ends_with(subject_iri, "surya"),
                "expected surya, got {subject_iri}"
            );
        }
        other => panic!("expected Describe, got {other:?}"),
    }
}

#[test]
fn describe_with_label() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");

    let report = resolve::resolve(QueryMode::Describe, "budha", &vocab, None, "jyotish")
        .expect("should resolve");

    match &report.query {
        ResolvedQuery::Describe { subject_iri } => {
            assert!(
                iri_ends_with(subject_iri, "budha"),
                "expected budha, got {subject_iri}"
            );
        }
        other => panic!("expected Describe, got {other:?}"),
    }
}

// ── Search mode ──

#[test]
fn search_fire_planets() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");

    let report =
        resolve::resolve(QueryMode::Search, "fire graha", &vocab, None, "jyotish")
            .expect("should resolve");

    match &report.query {
        ResolvedQuery::Search { type_iri, filters } => {
            assert!(
                iri_ends_with(type_iri, "Graha"),
                "expected Graha type, got {type_iri}"
            );
            assert!(
                filters.iter().any(|(k, v)| k == "element" && v == "fire"),
                "expected element=fire filter, got {filters:?}"
            );
        }
        other => panic!("expected Search, got {other:?}"),
    }
}

#[test]
fn search_cruel_grahas() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");

    let report =
        resolve::resolve(QueryMode::Search, "malefic graha", &vocab, None, "jyotish")
            .expect("should resolve");

    match &report.query {
        ResolvedQuery::Search { type_iri, filters } => {
            assert!(iri_ends_with(type_iri, "Graha"));
            assert!(
                filters.iter().any(|(k, v)| k == "nature" && v == "malefic"),
                "expected nature=malefic filter, got {filters:?}"
            );
        }
        other => panic!("expected Search, got {other:?}"),
    }
}

// ── Traverse mode ──

#[test]
fn traverse_mars_exalted() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");

    let report =
        resolve::resolve(QueryMode::Traverse, "mars exaltedIn", &vocab, None, "jyotish")
            .expect("should resolve");

    match &report.query {
        ResolvedQuery::Traverse {
            subject_iri,
            predicate_iri,
        } => {
            assert!(
                iri_ends_with(subject_iri, "mangala"),
                "expected mangala, got {subject_iri}"
            );
            assert!(
                iri_ends_with(predicate_iri, "exaltedIn"),
                "expected exaltedIn, got {predicate_iri}"
            );
        }
        other => panic!("expected Traverse, got {other:?}"),
    }
}

#[test]
fn traverse_mars_rules() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");

    let report =
        resolve::resolve(QueryMode::Traverse, "mars rules", &vocab, None, "jyotish")
            .expect("should resolve");

    match &report.query {
        ResolvedQuery::Traverse {
            subject_iri,
            predicate_iri,
        } => {
            assert!(iri_ends_with(subject_iri, "mangala"));
            assert!(iri_ends_with(predicate_iri, "rules"));
        }
        other => panic!("expected Traverse, got {other:?}"),
    }
}

// ── Error handling ──

#[test]
fn unknown_input_reports_error() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");

    let result = resolve::resolve(QueryMode::Describe, "xyzzy frobnicator", &vocab, None, "jyotish");
    assert!(result.is_err(), "expected error for unrecognized input");
}

#[test]
fn search_without_type_errors() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");

    let result = resolve::resolve(QueryMode::Search, "fire", &vocab, None, "jyotish");
    // "fire" resolves to a property value but no type — should error
    assert!(result.is_err(), "expected NoType error");
}

#[test]
fn unknown_tokens_reported() {
    let store = load_jyotish();
    let vocab = resolve::build_vocab(&store, "jyotish");

    let report =
        resolve::resolve(QueryMode::Search, "fire graha xyzzy", &vocab, None, "jyotish")
            .expect("should still resolve");

    assert!(
        report.unknown_tokens.contains(&"xyzzy".to_string()),
        "unknown token 'xyzzy' should be reported, got {:?}",
        report.unknown_tokens
    );
}

// ── VSA fallback ──

#[test]
fn vsa_index_builds_without_panic() {
    let store = load_jyotish();
    let vsa = resolve::build_vsa(&store, "jyotish");
    assert!(vsa.entity_count() > 0, "VSA index should have entities");
}

// ── Tokenizer ──

#[test]
fn tokenizer_handles_mixed_case_and_stopwords() {
    let tokens = vidya_core::resolve::matcher::tokenize("Tell me about the Fire Planets!");
    assert!(tokens.contains(&"fire".to_string()));
    assert!(tokens.contains(&"planets".to_string()));
    assert!(!tokens.contains(&"tell".to_string()));
    assert!(!tokens.contains(&"the".to_string()));
}
