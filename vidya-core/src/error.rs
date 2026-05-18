use oxigraph::sparql::QueryEvaluationError;
use oxigraph::store::{LoaderError, StorageError};

#[derive(Debug, thiserror::Error)]
pub enum VidyaError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("loader error: {0}")]
    Loader(#[from] LoaderError),

    #[error("query error: {0}")]
    Query(#[from] QueryEvaluationError),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, VidyaError>;
