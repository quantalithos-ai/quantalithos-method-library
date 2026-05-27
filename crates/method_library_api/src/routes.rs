//! HTTP routes and bootstrap state for the method-library service.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Json, OriginalUri, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use method_library_application::MethodContentCommandService;
use method_library_application::ports::Clock;
use method_library_application::ports::fakes::{
    DeterministicClock, DeterministicIdGenerator, FakeUnitOfWork, InMemoryAuditRepository,
    InMemoryIdempotencyRepository, InMemoryLifecycleHistoryRepository,
    InMemoryMethodContentReferenceRepository, InMemoryMethodContentRepository,
};
use method_library_contracts::{
    CreateMethodContentDraftCommand, CreateMethodContentDraftResponse, ErrorResponse,
    PlaceholderContract, SubmitMethodContentForReviewCommand, SubmitMethodContentForReviewResponse,
    UpdateMethodContentDraftCommand, UpdateMethodContentDraftResponse,
};
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};
use serde::Serialize;
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
    /// Builds shared router state from explicit dependencies.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>, command_service: Arc<MethodContentCommandService>) -> Self {
        Self {
            clock,
            command_service,
        }
    }

    /// Builds a bootstrap state backed by in-memory ports.
    #[must_use]
    pub fn bootstrap() -> Self {
        let clock: Arc<dyn Clock> =
            Arc::new(DeterministicClock::new(datetime!(2026-05-26 08:00:00 UTC)));
        let command_service = Arc::new(MethodContentCommandService::new(
            Arc::new(FakeUnitOfWork),
            Arc::new(InMemoryMethodContentRepository::default()),
            Arc::new(InMemoryMethodContentReferenceRepository::default()),
            Arc::new(InMemoryLifecycleHistoryRepository::default()),
            Arc::new(InMemoryAuditRepository::default()),
            Arc::new(InMemoryIdempotencyRepository::default()),
            clock.clone(),
            Arc::new(DeterministicIdGenerator::default()),
        ));

        Self::new(clock, command_service)
    }

    fn received_at(&self) -> method_library_domain::content::Timestamp {
        self.clock.now()
    }
}

/// Builds the HTTP router used by the binary entrypoint and tests.
#[must_use]
pub fn router() -> Router {
    router_with_state(AppState::bootstrap())
}

/// Builds the HTTP router with explicit shared application state.
#[must_use]
pub fn router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthcheck))
        .route("/contents", post(create_method_content_draft))
        .route(
            "/contents/{content_id}/draft",
            patch(update_method_content_draft),
        )
        .fallback(submit_method_content_for_review)
        .with_state(state)
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

async fn update_method_content_draft(
    State(state): State<AppState>,
    gateway: GatewayContextExtractor,
    Path(path_content_id): Path<String>,
    Json(command): Json<UpdateMethodContentDraftCommand>,
) -> Result<(StatusCode, Json<UpdateMethodContentDraftResponse>), (StatusCode, Json<ErrorResponse>)>
{
    validate_path_content_id(&path_content_id, &command.content_id).map_err(|error| {
        error_response(
            StatusCode::BAD_REQUEST,
            gateway.headers.request_id.clone(),
            gateway.headers.trace_id.clone(),
            error,
        )
    })?;
    let request_hash = canonical_request_hash(&command).map_err(|error| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            gateway.headers.request_id.clone(),
            gateway.headers.trace_id.clone(),
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
        .update_draft(command, gateway.actor.clone(), meta)
        .await
        .map(|response| (StatusCode::OK, Json(response)))
        .map_err(|error| {
            map_error_response(
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error,
            )
        })
}

async fn submit_method_content_for_review(
    State(state): State<AppState>,
    gateway: GatewayContextExtractor,
    OriginalUri(original_uri): OriginalUri,
    Json(command): Json<SubmitMethodContentForReviewCommand>,
) -> Result<
    (StatusCode, Json<SubmitMethodContentForReviewResponse>),
    (StatusCode, Json<ErrorResponse>),
> {
    let Some(path_content_id) = original_uri
        .path()
        .strip_prefix("/contents/")
        .and_then(|rest| rest.split_once(':'))
        .and_then(|(content_id, action)| (action == "submit-review").then_some(content_id))
    else {
        return Err(error_response(
            StatusCode::NOT_FOUND,
            gateway.headers.request_id.clone(),
            gateway.headers.trace_id.clone(),
            MethodLibraryError::validation(
                MethodLibraryErrorCode::MethodContentNotFound,
                "submit review route was not found",
            ),
        ));
    };
    validate_path_content_id(&path_content_id, &command.content_id).map_err(|error| {
        error_response(
            StatusCode::BAD_REQUEST,
            gateway.headers.request_id.clone(),
            gateway.headers.trace_id.clone(),
            error,
        )
    })?;
    let request_hash = canonical_request_hash(&command).map_err(|error| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            gateway.headers.request_id.clone(),
            gateway.headers.trace_id.clone(),
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
        .submit_for_review(command, gateway.actor.clone(), meta)
        .await
        .map(|response| (StatusCode::OK, Json(response)))
        .map_err(|error| {
            map_error_response(
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error,
            )
        })
}

fn canonical_request_hash(command: &impl Serialize) -> Result<String, MethodLibraryError> {
    let canonical = serde_json::to_string(command).map_err(|error| {
        MethodLibraryError::retryable(
            MethodLibraryErrorCode::PersistenceUnavailable,
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
        MethodLibraryErrorCode::GatewayContextMissing
        | MethodLibraryErrorCode::GatewayContextInvalid
        | MethodLibraryErrorCode::IdempotencyKeyRequired
        | MethodLibraryErrorCode::PathBodyMismatch => StatusCode::BAD_REQUEST,
        MethodLibraryErrorCode::BoundaryViolation
        | MethodLibraryErrorCode::ReferenceInvalid
        | MethodLibraryErrorCode::PayloadKindMismatch => StatusCode::UNPROCESSABLE_ENTITY,
        MethodLibraryErrorCode::IdempotencyConflict
        | MethodLibraryErrorCode::IdempotencyStatusConflict
        | MethodLibraryErrorCode::LifecycleTransitionNotAllowed
        | MethodLibraryErrorCode::PublishedContentImmutable => StatusCode::CONFLICT,
        MethodLibraryErrorCode::MethodContentNotFound => StatusCode::NOT_FOUND,
        MethodLibraryErrorCode::RevisionConflict => StatusCode::PRECONDITION_FAILED,
        MethodLibraryErrorCode::PersistenceUnavailable
        | MethodLibraryErrorCode::TransactionCommitFailed => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };

    error_response(status, request_id, trace_id, error)
}

fn validate_path_content_id(
    path_content_id: &str,
    body_content_id: &str,
) -> Result<(), MethodLibraryError> {
    if path_content_id == body_content_id {
        return Ok(());
    }

    Err(MethodLibraryError::validation(
        MethodLibraryErrorCode::PathBodyMismatch,
        "path content id does not match the request body",
    )
    .with_detail("path_content_id", path_content_id.to_string())
    .with_detail("body_content_id", body_content_id.to_string()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use http::Request;
    use time::macros::datetime;
    use tower::ServiceExt;

    use super::{AppState, router, router_with_state};
    use method_library_application::ports::Clock;
    use method_library_application::ports::fakes::{
        DeterministicClock, DeterministicIdGenerator, FakeUnitOfWork, InMemoryAuditRepository,
        InMemoryIdempotencyRepository, InMemoryLifecycleHistoryRepository,
        InMemoryMethodContentReferenceRepository, InMemoryMethodContentRepository,
    };
    use method_library_application::{
        MethodContentCommandService, MethodContentRepository, UnitOfWork,
    };
    use method_library_contracts::{ErrorResponse, RequestMeta};
    use method_library_domain::MethodLibraryErrorCode;
    use method_library_domain::content::{ContentVersion, MethodContent, MethodContentKind};
    use method_library_domain::definitions::{
        EvidenceKind, EvidenceRule, MethodContentPayload, Qualification, QualificationLevel,
        QualificationLevelModel,
    };

    fn test_state() -> (
        AppState,
        Arc<InMemoryMethodContentRepository>,
        Arc<InMemoryLifecycleHistoryRepository>,
    ) {
        let clock: Arc<dyn Clock> =
            Arc::new(DeterministicClock::new(datetime!(2026-05-26 08:00:00 UTC)));
        let content_repository = Arc::new(InMemoryMethodContentRepository::default());
        let reference_repository = Arc::new(InMemoryMethodContentReferenceRepository::default());
        let lifecycle_history_repository = Arc::new(InMemoryLifecycleHistoryRepository::default());
        let audit_repository = Arc::new(InMemoryAuditRepository::default());
        let idempotency_repository = Arc::new(InMemoryIdempotencyRepository::default());
        let command_service = Arc::new(MethodContentCommandService::new(
            Arc::new(FakeUnitOfWork),
            content_repository.clone(),
            reference_repository,
            lifecycle_history_repository.clone(),
            audit_repository,
            idempotency_repository,
            clock.clone(),
            Arc::new(DeterministicIdGenerator::default()),
        ));

        (
            AppState::new(clock, command_service),
            content_repository,
            lifecycle_history_repository,
        )
    }

    fn sample_submit_body(content_id: &str, expected_revision: i64) -> serde_json::Value {
        serde_json::json!({
            "content_id": content_id,
            "expected_revision": expected_revision,
            "review_reason": "Ready for review",
            "review_evidence_refs": [
                {
                    "artifact_id": "artifact-2",
                    "artifact_kind": "review-note"
                }
            ]
        })
    }

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

    #[tokio::test]
    async fn updates_draft_via_http() {
        let app = router();

        let create_response = app
            .clone()
            .oneshot(build_gateway_request(
                "POST",
                "/contents",
                Some("idem-1"),
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
                }),
            ))
            .await
            .expect("create route should respond");

        assert_eq!(create_response.status(), StatusCode::CREATED);

        let update_response = app
            .oneshot(build_gateway_request(
                "PATCH",
                "/contents/content-1/draft",
                Some("idem-2"),
                serde_json::json!({
                    "content_id": "content-1",
                    "expected_revision": 1,
                    "name": "Quality Updated",
                    "description": "Updated definition",
                    "payload": {
                        "qualification": {
                            "qualification_key": "quality-1",
                            "name": "Quality Updated",
                            "description": "Updated definition",
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
                                    "description": "Updated proof"
                                }
                            ]
                        }
                    },
                    "references": []
                }),
            ))
            .await
            .expect("update route should respond");

        assert_eq!(update_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn submits_draft_for_review_via_http() {
        let app = router();

        let create_response = app
            .clone()
            .oneshot(build_gateway_request(
                "POST",
                "/contents",
                Some("idem-1"),
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
                }),
            ))
            .await
            .expect("create route should respond");

        assert_eq!(create_response.status(), StatusCode::CREATED);

        let submit_response = app
            .oneshot(build_gateway_request(
                "POST",
                "/contents/content-1:submit-review",
                Some("idem-2"),
                sample_submit_body("content-1", 1),
            ))
            .await
            .expect("submit route should respond");

        assert_eq!(submit_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_missing_idempotency_key_via_http() {
        let response = router()
            .oneshot(build_gateway_request(
                "POST",
                "/contents",
                None,
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
                }),
            ))
            .await
            .expect("router should respond");

        assert_error_code(
            response,
            StatusCode::BAD_REQUEST,
            MethodLibraryErrorCode::IdempotencyKeyRequired,
        )
        .await;
    }

    #[tokio::test]
    async fn rejects_path_body_mismatch_via_http() {
        let app = router();

        let create_response = app
            .clone()
            .oneshot(build_gateway_request(
                "POST",
                "/contents",
                Some("idem-1"),
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
                }),
            ))
            .await
            .expect("create route should respond");

        assert_eq!(create_response.status(), StatusCode::CREATED);

        let response = app
            .oneshot(build_gateway_request(
                "PATCH",
                "/contents/content-1/draft",
                Some("idem-2"),
                serde_json::json!({
                    "content_id": "content-2",
                    "expected_revision": 1,
                    "name": "Quality Updated",
                    "description": "Updated definition",
                    "payload": {
                        "qualification": {
                            "qualification_key": "quality-1",
                            "name": "Quality Updated",
                            "description": "Updated definition",
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
                                    "description": "Updated proof"
                                }
                            ]
                        }
                    },
                    "references": []
                }),
            ))
            .await
            .expect("router should respond");

        assert_error_code(
            response,
            StatusCode::BAD_REQUEST,
            MethodLibraryErrorCode::PathBodyMismatch,
        )
        .await;
    }

    #[tokio::test]
    async fn maps_published_update_error_via_http() {
        let (state, content_repository, _) = test_state();
        seed_published_content(&content_repository).await;
        let app = router_with_state(state);

        let response = app
            .oneshot(build_gateway_request(
                "PATCH",
                "/contents/content-published/draft",
                Some("idem-2"),
                serde_json::json!({
                    "content_id": "content-published",
                    "expected_revision": 3,
                    "name": "Quality Updated",
                    "description": "Updated definition",
                    "payload": {
                        "qualification": {
                            "qualification_key": "quality-1",
                            "name": "Quality Updated",
                            "description": "Updated definition",
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
                                    "description": "Updated proof"
                                }
                            ]
                        }
                    },
                    "references": []
                }),
            ))
            .await
            .expect("router should respond");

        assert_error_code(
            response,
            StatusCode::CONFLICT,
            MethodLibraryErrorCode::PublishedContentImmutable,
        )
        .await;
    }

    fn build_gateway_request(
        method: &str,
        uri: &str,
        idempotency_key: Option<&str>,
        body: serde_json::Value,
    ) -> Request<Body> {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .header("x-request-id", "req-1")
            .header("x-trace-id", "trace-1")
            .header("x-actor-id", "actor-1")
            .header("x-actor-kind", "human")
            .header("x-gateway-trusted-by", "gateway");
        if let Some(idempotency_key) = idempotency_key {
            builder = builder.header("x-idempotency-key", idempotency_key);
        }
        builder
            .body(Body::from(body.to_string()))
            .expect("request should build")
    }

    async fn assert_error_code(
        response: axum::response::Response,
        expected_status: StatusCode,
        expected_code: MethodLibraryErrorCode,
    ) {
        assert_eq!(response.status(), expected_status);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let response: ErrorResponse =
            serde_json::from_slice(&bytes).expect("error body should parse");
        assert_eq!(response.error.code, expected_code);
    }

    async fn seed_published_content(repository: &Arc<InMemoryMethodContentRepository>) -> String {
        let actor_id = "actor-1".to_string();
        let mut content = MethodContent::create_draft(
            "content-published".to_string(),
            "family-published".to_string(),
            MethodContentKind::Qualification,
            "Quality".to_string(),
            None,
            MethodContentPayload::Qualification(Qualification {
                qualification_key: "quality-1".to_string(),
                name: "Quality".to_string(),
                description: None,
                level_model: QualificationLevelModel {
                    levels: vec![QualificationLevel {
                        level_key: "basic".to_string(),
                        name: "Basic".to_string(),
                        order: 1,
                        description: None,
                    }],
                    default_level_key: Some("basic".to_string()),
                },
                evidence_rules: vec![EvidenceRule {
                    evidence_kind: EvidenceKind::Document,
                    required: true,
                    description: "Proof".to_string(),
                }],
            }),
            actor_id.clone(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("draft should build");
        content
            .submit_for_review(actor_id.clone(), datetime!(2026-05-26 08:05:00 UTC))
            .expect("review submission should work");
        content
            .publish(
                method_library_domain::content::ApprovedGateRef {
                    gate_id: "gate-1".to_string(),
                    gate_decision_id: "decision-1".to_string(),
                    approved_at: datetime!(2026-05-26 08:10:00 UTC),
                },
                ContentVersion::new("1.0.0").expect("version should build"),
                method_library_domain::content::CanonicalFingerprint::new(
                    method_library_domain::content::FingerprintAlgorithm::Sha256,
                    "abc123",
                    "1.0",
                )
                .expect("fingerprint should build"),
                actor_id,
                datetime!(2026-05-26 08:10:00 UTC),
            )
            .expect("publish should work");

        let clock = Arc::new(DeterministicClock::new(datetime!(2026-05-26 08:00:00 UTC)));
        let meta = RequestMeta {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            idempotency_key: Some("idem-seed".to_string()),
            request_hash: "hash-seed".to_string(),
            received_at: clock.now(),
        };
        let mut tx = FakeUnitOfWork
            .begin(meta)
            .await
            .expect("transaction should open");
        repository
            .insert(&mut tx, content)
            .await
            .expect("content should insert");
        tx.commit().await.expect("transaction should commit");

        "content-published".to_string()
    }
}
