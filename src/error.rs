use rmcp::model::ErrorData;

#[derive(Debug, thiserror::Error)]
pub enum VidyaError {
    #[error("invalid argument: {constraint} (got: {received})")]
    InvalidArgument {
        tool: String,
        argument: String,
        constraint: String,
        received: String,
    },

    #[error("{kind} not found")]
    NotFound {
        tool: String,
        kind: String,
    },

    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("internal error: {0}")]
    Internal(String),
}

pub fn to_error_data(e: VidyaError) -> ErrorData {
    match &e {
        VidyaError::InvalidArgument { .. } => {
            ErrorData::invalid_params(e.to_string(), None)
        }
        _ => ErrorData::internal_error(e.to_string(), None),
    }
}

pub type Result<T> = std::result::Result<T, VidyaError>;
