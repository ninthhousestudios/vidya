pub mod error;
pub mod ontology;
pub mod query;
pub mod resolve;
pub mod store;
pub mod vsa;

pub use error::{Result, VidyaError};
pub use query::{CoverageResult, DescribeResult, ProvenanceFilter, ProvenanceResult, SearchResult, SimilarityResult, SimilarityMatch, TraverseResult, TypeSummary, VocabResult};
pub use resolve::{AlternativeParse, IntentError, ProvenanceScope, QueryMode, ResolvedQuery, ResolutionReport};
pub use store::{KnowledgeStore, ResolveContext};
