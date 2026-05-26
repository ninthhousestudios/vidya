pub mod assemble;
pub mod matcher;
pub mod vocab;

pub use assemble::{AssembleError, QueryMode, ResolvedQuery, ResolutionReport};
pub use matcher::{MatchConfidence, ResolvedToken};
pub use vocab::{SchemaVocab, SynonymTable};

use crate::store::KnowledgeStore;
use crate::vsa::{EntityIndex, Hrr};

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

pub fn build_vocab(store: &KnowledgeStore, domain: &str) -> SchemaVocab {
    SchemaVocab::build(store.inner(), domain)
}

pub fn build_vsa(store: &KnowledgeStore, domain: &str) -> EntityIndex<Hrr> {
    let hrr = Hrr::new(1024);
    EntityIndex::build(hrr, store.inner(), domain)
}
