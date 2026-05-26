pub mod assemble;
pub mod intent;
pub mod matcher;
pub mod rank;
pub mod vocab;

pub use assemble::{AlternativeParse, AssembleError, ProvenanceScope, QueryMode, ResolvedQuery, ResolutionReport};
pub use intent::IntentResult;
pub use matcher::{MatchConfidence, ResolvedToken};
pub use vocab::{SchemaVocab, SynonymTable};

use crate::store::KnowledgeStore;
use crate::vsa::{EntityIndex, Hrr};

#[derive(Debug, thiserror::Error)]
pub enum IntentError {
    #[error("no intent pattern matched — try a specific command like 'describe', 'search', or 'traverse'")]
    NoIntentDetected,
    #[error("{0}")]
    Assemble(#[from] AssembleError),
}

pub fn resolve(
    mode: QueryMode,
    input: &str,
    vocab: &SchemaVocab,
    vsa: Option<&EntityIndex<Hrr>>,
    domain: &str,
) -> std::result::Result<ResolutionReport, AssembleError> {
    let tokens = matcher::tokenize(input);
    let matched = matcher::match_tokens(&tokens, vocab, vsa, domain);
    assemble::assemble(mode, &matched, vocab)
}

pub fn resolve_nl(
    raw_input: &str,
    vocab: &SchemaVocab,
    vsa: Option<&EntityIndex<Hrr>>,
    domain: &str,
) -> std::result::Result<ResolutionReport, IntentError> {
    let intents = intent::detect_all_intents(raw_input);
    if intents.is_empty() {
        return Err(IntentError::NoIntentDetected);
    }

    let attempts: Vec<rank::ParseAttempt> = intents
        .into_iter()
        .map(|intent| {
            let tokens = matcher::tokenize(&intent.slot_text);
            let matched = matcher::match_tokens(&tokens, vocab, vsa, domain);
            match assemble::assemble(intent.mode, &matched, vocab) {
                Ok(report) => rank::ParseAttempt::Ok {
                    intent,
                    tokens: matched,
                    report,
                },
                Err(error) => rank::ParseAttempt::Err {
                    _intent: intent,
                    _tokens: matched,
                    _error: error,
                },
            }
        })
        .collect();

    let mut ranked = rank::rank(attempts);

    if ranked.is_empty() {
        let intent = intent::detect_intent(raw_input).unwrap();
        let tokens = matcher::tokenize(&intent.slot_text);
        let matched = matcher::match_tokens(&tokens, vocab, vsa, domain);
        return Err(IntentError::Assemble(
            assemble::assemble(intent.mode, &matched, vocab).unwrap_err(),
        ));
    }

    let winner = ranked.remove(0);
    let mut report = winner.report;

    report.alternatives = ranked
        .iter()
        .map(|c| assemble::AlternativeParse {
            query: c.report.query.clone(),
            pattern_name: c.pattern_name.to_string(),
            score: c.total_score,
            score_breakdown: c.signals.iter().map(|s| (s.name.to_string(), s.value)).collect(),
        })
        .collect();

    report.resolution_details.insert(
        0,
        format!(
            "intent: {:?} (pattern: {}, score: {:.2})",
            query_mode_of(&report.query),
            winner.pattern_name,
            winner.total_score,
        ),
    );
    for signal in &winner.signals {
        report
            .resolution_details
            .push(format!("  {}: {:.3}", signal.name, signal.value));
    }

    if let Some(ref hint) = winner.scope_hint {
        let scope = vocab.resolve_provenance(hint);
        if scope.is_empty() {
            report
                .resolution_details
                .push(format!("scope: \"{hint}\" (unresolved)"));
        } else {
            if let Some(ref t) = scope.tradition {
                report
                    .resolution_details
                    .push(format!("scope tradition: {}", assemble::short_name(t)));
            }
            if let Some(ref s) = scope.source {
                report
                    .resolution_details
                    .push(format!("scope source: {}", assemble::short_name(s)));
            }
            if let Some(ref p) = scope.pramana {
                report
                    .resolution_details
                    .push(format!("scope pramana: {}", assemble::short_name(p)));
            }
        }
        report.scope = scope;
    }

    Ok(report)
}

fn query_mode_of(q: &ResolvedQuery) -> QueryMode {
    match q {
        ResolvedQuery::Describe { .. } => QueryMode::Describe,
        ResolvedQuery::Search { .. } => QueryMode::Search,
        ResolvedQuery::Traverse { .. } => QueryMode::Traverse,
        ResolvedQuery::Provenance { .. } => QueryMode::Provenance,
        ResolvedQuery::Similar { .. } => QueryMode::Similar,
        ResolvedQuery::Unbind { .. } => QueryMode::Unbind,
    }
}

pub fn build_vocab(store: &KnowledgeStore, domain: &str) -> SchemaVocab {
    SchemaVocab::build(store.inner(), domain)
}

pub fn build_vsa(store: &KnowledgeStore, domain: &str) -> EntityIndex<Hrr> {
    let hrr = Hrr::new(1024);
    EntityIndex::build(hrr, store.inner(), domain)
}
