//! HTTP routes and bootstrap state for the method-library service.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Json, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use method_library_application::MethodContentCommandService;
use method_library_application::ports::Clock;
use method_library_application::ports::fakes::{
    DeterministicClock, DeterministicIdGenerator, FakeUnitOfWork, InMemoryAuditRepository,
    InMemoryIdempotencyRepository, InMemoryMethodContentReferenceRepository,
    InMemoryMethodContentRepository,
};
use method_library_contracts::{
    CreateMethodContentDraftCommand, CreateMethodContentDraftResponse, ErrorResponse,
    PlaceholderContract,
};
use method_library_domain::MethodLibraryError;
use sha2::{Digest, Sha256};
use time::macros::datetime;

use crate::extractors::GatewayContextExtractor;

/// Shared application state used by the HTTP router.
#[derive(Clone)]
pub struct AppState {
    clock: Arc<dyn Clock>,
    command_service: Arc<MethodContentCommandService>,
}

impl AppState {
    /// Builds a bootstrap state backed by in-memory ports.
    #[must_use]
    pub fn bootstrap() -> Self {
        let clock: Arc<dyn Clock> =
            Arc::new(DeterministicClock::new(datetime!(2026-05-26 08:00:00 UTC)));
        let command_service = Arc::new(MethodContentCommandService::new(
            Arc::new(FakeUnitOfWork),
            Arc::new(InMemoryMethodContentRepository::default()),
            Arc::new(InMemoryMethodContentReferenceRepository::default()),
            Arc::new(InMemoryAuditRepository::default()),
            Arc::new(InMemoryIdempotencyRepository::default()),
            clock.clone(),
            Arc::new(DeterministicIdGenerator::default()),
        ));

        Self {
            clock,
            command_service,
        }
    }

    fn received_at(&self) -> method_library_domain::content::Timestamp {
        self.clock.now()
    }
}

/// Builds the HTTP router used by the binary entrypoint and tests.
#[must_use]
pub fn router() -> Router {
    Router::new()
        .route("/healthz", get(healthcheck))
        .route("/contents", post(create_method_content_draft))
        .with_state(AppState::bootstrap())
}

async fn healthcheck() -> Json<PlaceholderContract> {
    Json(PlaceholderContract::default())
}

async fn create_method_content_draft(
    State(state): State<AppState>,
    gateway: GatewayContextExtractor,
    Json(command): Json<CreateMethodContentDraftCommand>,
) -> Result<(StatusCode, Json<CreateMethodContentDraftResponse>), (StatusCode, Json<ErrorResponse>)>
{
    let request_hash = canonical_request_hash(&command).map_err(|error| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            String::new(),
            String::new(),
            error,
        )
    })?;
    let meta = gateway
        .request_meta(request_hash, state.received_at())
        .map_err(|error| {
            error_response(
                StatusCode::BAD_REQUEST,
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error,
            )
        })?;

    state
        .command_service
        .create_draft(command, gateway.actor.clone(), meta)
        .await
        .map(|response| (StatusCode::CREATED, Json(response)))
        .map_err(|error| {
            map_error_response(
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error,
            )
        })
}

fn canonical_request_hash(
    command: &CreateMethodContentDraftCommand,
) -> Result<String, MethodLibraryError> {
    let canonical = serde_json::to_string(command).map_err(|error| {
        MethodLibraryError::retryable(
            method_library_domain::MethodLibraryErrorCode::PersistenceUnavailable,
            format!("failed to canonicalize request body: {error}"),
        )
    })?;
    Ok(format!("{:x}", Sha256::digest(canonical.as_bytes())))
}

fn error_response(
    status: StatusCode,
    request_id: String,
    trace_id: String,
    error: MethodLibraryError,
) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse::from_domain_error(
            request_id, trace_id, error,
        )),
    )
}

fn map_error_response(
    request_id: String,
    trace_id: String,
    error: MethodLibraryError,
) -> (StatusCode, Json<ErrorResponse>) {
    let status = match error.code {
        method_library_domain::MethodLibraryErrorCode::GatewayContextMissing
        | method_library_domain::MethodLibraryErrorCode::GatewayContextInvalid
        | method_library_domain::MethodLibraryErrorCode::IdempotencyKeyRequired => {
            StatusCode::BAD_REQUEST
        }
        method_library_domain::MethodLibraryErrorCode::BoundaryViolation
        | method_library_domain::MethodLibraryErrorCode::ReferenceInvalid
        | method_library_domain::MethodLibraryErrorCode::PayloadKindMismatch => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
        method_library_domain::MethodLibraryErrorCode::IdempotencyConflict
        | method_library_domain::MethodLibraryErrorCode::IdempotencyStatusConflict => {
            StatusCode::CONFLICT
        }
        method_library_domain::MethodLibraryErrorCode::MethodContentNotFound => {
            StatusCode::NOT_FOUND
        }
        method_library_domain::MethodLibraryErrorCode::RevisionConflict => {
            StatusCode::PRECONDITION_FAILED
        }
        method_library_domain::MethodLibraryErrorCode::PersistenceUnavailable
        | method_library_domain::MethodLibraryErrorCode::TransactionCommitFailed => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };

    error_response(status, request_id, trace_id, error)
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::StatusCode;
    use http::Request;
    use tower::ServiceExt;

    use super::router;

    #[tokio::test]
    async fn responds_to_healthcheck() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn creates_draft_via_http() {
        let request = Request::builder()
            .method("POST")
            .uri("/contents")
            .header("content-type", "application/json")
            .header("x-request-id", "req-1")
            .header("x-trace-id", "trace-1")
            .header("x-idempotency-key", "idem-1")
            .header("x-actor-id", "actor-1")
            .header("x-actor-kind", "human")
            .header("x-gateway-trusted-by", "gateway")
            .body(Body::from(
                serde_json::json!({
                    "kind": "qualification",
                    "name": "Quality",
                    "description": null,
                    "payload": {
                        "qualification": {
                            "qualification_key": "quality-1",
                            "name": "Quality",
                            "description": null,
                            "level_model": {
                                "levels": [
                                    {
                                        "level_key": "basic",
                                        "name": "Basic",
                                        "order": 1,
                                        "description": null
                                    }
                                ],
                                "default_level_key": "basic"
                            },
                            "evidence_rules": [
                                {
                                    "evidence_kind": "document",
                                    "required": true,
                                    "description": "Proof"
                                }
                            ]
                        }
                    },
                    "references": [],
                    "source_refs": []
                })
                .to_string(),
            ))
            .expect("request should build");

        let response = router()
            .oneshot(request)
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::CREATED);
    }
}
