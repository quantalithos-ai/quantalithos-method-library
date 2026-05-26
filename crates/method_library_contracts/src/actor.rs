//! Actor and gateway context contracts.

use serde::{Deserialize, Serialize};

use method_library_domain::content::{ActorId, ActorKind, IdempotencyKey, RequestId, TraceId};
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};

/// Gateway headers trusted by the inbound adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayHeaders {
    /// Gateway request identifier.
    pub request_id: RequestId,
    /// Distributed trace identifier.
    pub trace_id: TraceId,
    /// Optional idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
    /// Trusted actor identifier.
    pub actor_id: ActorId,
    /// Trusted actor kind.
    pub actor_kind: ActorKind,
    /// Identity of the trusted gateway.
    pub trusted_by: String,
}

/// Stable actor reference used by audit and trace records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorRef {
    /// Stable actor identifier.
    pub actor_id: ActorId,
    /// Stable actor kind.
    pub actor_kind: ActorKind,
}

/// Request-scoped actor context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorContext {
    /// Stable actor identifier.
    pub actor_id: ActorId,
    /// Stable actor kind.
    pub actor_kind: ActorKind,
    /// Actor reference used by append-only records.
    pub actor_ref: ActorRef,
}

/// External artifact reference carried by commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    /// External artifact identifier.
    pub artifact_id: String,
    /// External artifact kind.
    pub artifact_kind: String,
}

impl GatewayHeaders {
    /// Validates that the trusted gateway headers are populated.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        if self.request_id.trim().is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::GatewayContextMissing,
                "gateway request id is required",
            ));
        }
        if self.trace_id.trim().is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::GatewayContextMissing,
                "gateway trace id is required",
            ));
        }
        if self.actor_id.trim().is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::GatewayContextMissing,
                "gateway actor id is required",
            ));
        }
        if self.trusted_by.trim().is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::GatewayContextInvalid,
                "gateway trust source is required",
            ));
        }

        Ok(())
    }
}

impl ActorContext {
    /// Builds the request actor context from gateway headers.
    pub fn from_gateway_headers(headers: &GatewayHeaders) -> Result<Self, MethodLibraryError> {
        headers.validate()?;

        Ok(Self {
            actor_id: headers.actor_id.clone(),
            actor_kind: headers.actor_kind,
            actor_ref: ActorRef {
                actor_id: headers.actor_id.clone(),
                actor_kind: headers.actor_kind,
            },
        })
    }
}

impl ArtifactRef {
    /// Validates the artifact reference.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        if self.artifact_id.trim().is_empty() || self.artifact_kind.trim().is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::ReferenceInvalid,
                "artifact references must carry both id and kind",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ActorContext, GatewayHeaders};
    use method_library_domain::content::ActorKind;

    #[test]
    fn builds_actor_context_from_gateway_headers() {
        let headers = GatewayHeaders {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            idempotency_key: Some("idem-1".to_string()),
            actor_id: "actor-1".to_string(),
            actor_kind: ActorKind::Human,
            trusted_by: "gateway".to_string(),
        };

        let actor = ActorContext::from_gateway_headers(&headers).expect("headers should validate");

        assert_eq!(actor.actor_ref.actor_id, "actor-1");
    }
}
