//! Shared request and trace metadata contracts.

use serde::{Deserialize, Serialize};

use method_library_domain::content::{IdempotencyKey, RequestHash, RequestId, Timestamp, TraceId};
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};

use crate::actor::GatewayHeaders;

/// Shared request metadata propagated across commands, queries, and jobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestMeta {
    /// Gateway request identifier.
    pub request_id: RequestId,
    /// Distributed trace identifier.
    pub trace_id: TraceId,
    /// Optional idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
    /// Canonical request hash used by idempotency records.
    pub request_hash: RequestHash,
    /// Inbound receipt timestamp.
    pub received_at: Timestamp,
}

/// Alias retained for command-service specific metadata usage.
pub type CommandMetadata = RequestMeta;

impl RequestMeta {
    /// Builds request metadata from gateway headers and resolved request context.
    pub fn from_gateway_headers(
        headers: &GatewayHeaders,
        request_hash: RequestHash,
        received_at: Timestamp,
    ) -> Result<Self, MethodLibraryError> {
        headers.validate()?;
        if request_hash.trim().is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::GatewayContextInvalid,
                "request hash is required",
            ));
        }

        Ok(Self {
            request_id: headers.request_id.clone(),
            trace_id: headers.trace_id.clone(),
            idempotency_key: headers.idempotency_key.clone(),
            request_hash,
            received_at,
        })
    }

    /// Returns the required idempotency key for write paths.
    pub fn require_idempotency_key(&self) -> Result<&IdempotencyKey, MethodLibraryError> {
        self.idempotency_key.as_ref().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::IdempotencyKeyRequired,
                "write requests require an idempotency key",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::RequestMeta;
    use crate::actor::GatewayHeaders;
    use method_library_domain::content::ActorKind;

    #[test]
    fn requires_idempotency_key_for_write_paths() {
        let headers = GatewayHeaders {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            idempotency_key: None,
            actor_id: "actor-1".to_string(),
            actor_kind: ActorKind::Human,
            trusted_by: "gateway".to_string(),
        };

        let meta = RequestMeta::from_gateway_headers(
            &headers,
            "hash-1".to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("metadata should build");

        let error = meta
            .require_idempotency_key()
            .expect_err("missing idempotency key should fail");

        assert_eq!(
            error.code,
            method_library_domain::MethodLibraryErrorCode::IdempotencyKeyRequired
        );
    }
}
