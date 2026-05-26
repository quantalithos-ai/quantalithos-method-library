//! Error response contracts shared by API and worker surfaces.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use method_library_domain::content::{RequestId, TraceId};
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};

/// Structured error details safe to expose to callers.
pub type ErrorDetails = BTreeMap<String, String>;

/// Outer error envelope returned to callers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Request identifier.
    pub request_id: RequestId,
    /// Error payload.
    pub error: ErrorBody,
}

/// Inner error payload returned to callers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Stable machine-readable error code.
    pub code: MethodLibraryErrorCode,
    /// Human-readable error message.
    pub message: String,
    /// Whether the caller may retry without changing input.
    pub retryable: bool,
    /// Structured error details.
    pub details: ErrorDetails,
    /// Distributed trace identifier.
    pub trace_id: TraceId,
}

impl ErrorResponse {
    /// Builds an error response from a domain error.
    #[must_use]
    pub fn from_domain_error(
        request_id: RequestId,
        trace_id: TraceId,
        error: MethodLibraryError,
    ) -> Self {
        Self {
            request_id,
            error: ErrorBody {
                code: error.code,
                message: error.message,
                retryable: error.retryable,
                details: error.details,
                trace_id,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ErrorResponse;

    #[test]
    fn maps_domain_errors_to_responses() {
        let response = ErrorResponse::from_domain_error(
            "req-1".to_string(),
            "trace-1".to_string(),
            method_library_domain::MethodLibraryError::validation(
                method_library_domain::MethodLibraryErrorCode::RevisionConflict,
                "expected revision does not match",
            ),
        );

        assert_eq!(
            response.error.code,
            method_library_domain::MethodLibraryErrorCode::RevisionConflict
        );
    }
}
