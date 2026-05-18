use rmcp::model::ErrorData;
use vidya_core::VidyaError;

pub fn to_error_data(e: VidyaError) -> ErrorData {
    match &e {
        VidyaError::InvalidArgument(_) | VidyaError::NotFound(_) => {
            ErrorData::invalid_params(e.to_string(), None)
        }
        _ => ErrorData::internal_error(e.to_string(), None),
    }
}
