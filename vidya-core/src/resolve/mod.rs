pub mod assemble;
pub mod intent;
pub mod matcher;
pub mod vocab;

pub use assemble::{AssembleError, QueryMode, ResolvedQuery, ResolutionReport};
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
    let intent = intent::detect_intent(raw_input).ok_or(IntentError::NoIntentDetected)?;

    let tokens = matcher::tokenize(&intent.slot_text);
    let matched = matcher::match_tokens(&tokens, vocab, vsa, domain);
    let mut report = assemble::assemble(intent.mode, &matched, vocab)?;

    report.resolution_details.insert(
        0,
        format!("intent: {:?} (pattern: {})", intent.mode, intent.pattern_name),
    );
    if let Some(tradition) = &intent.tradition {
        report
            .resolution_details
            .push(format!("tradition: {tradition}"));
    }

    Ok(report)
}

pub fn build_vocab(store: &KnowledgeStore, domain: &str) -> SchemaVocab {
    SchemaVocab::build(store.inner(), domain)
}

pub fn build_vsa(store: &KnowledgeStore, domain: &str) -> EntityIndex<Hrr> {
    let hrr = Hrr::new(1024);
    EntityIndex::build(hrr, store.inner(), domain)
}
