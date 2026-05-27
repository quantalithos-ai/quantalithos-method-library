//! HTTP routes and bootstrap state for the method-library service.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use method_library_application::MethodContentCommandService;
use method_library_application::ports::Clock;
use method_library_application::ports::fakes::{
    DeterministicClock, DeterministicFingerprintHasher, DeterministicIdGenerator, FakeUnitOfWork,
    InMemoryAuditRepository, InMemoryDefinitionSnapshotRepository, InMemoryIdempotencyRepository,
    InMemoryLifecycleHistoryRepository, InMemoryMethodContentReferenceRepository,
    InMemoryMethodContentRepository, InMemoryMethodContentVersionRepository, InMemoryObjectStorage,
    InMemoryOutboxRepository, InMemorySupersedeLinkRepository, StaticGovernancePort,
};
use method_library_contracts::{
    CreateMethodContentDraftCommand, CreateMethodContentDraftResponse,
    DeprecateMethodContentCommand, DeprecateMethodContentResponse, ErrorResponse,
    PlaceholderContract, PublishMethodContentCommand, PublishMethodContentResponse,
    RetireMethodContentCommand, RetireMethodContentResponse, SubmitMethodContentForReviewCommand,
    SubmitMethodContentForReviewResponse, SupersedeMethodContentCommand,
    SupersedeMethodContentResponse, UpdateMethodContentDraftCommand,
    UpdateMethodContentDraftResponse,
};
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use time::macros::datetime;
use tracing::{error, info, warn};

use crate::extractors::GatewayContextExtractor;

const API_BASE_PATH: &str = "/api/v1/method-library";

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
            Arc::new(InMemoryMethodContentVersionRepository::default()),
            Arc::new(InMemoryDefinitionSnapshotRepository::default()),
            Arc::new(InMemorySupersedeLinkRepository::default()),
            Arc::new(InMemoryOutboxRepository::default()),
            Arc::new(InMemoryLifecycleHistoryRepository::default()),
            Arc::new(InMemoryAuditRepository::default()),
            Arc::new(InMemoryIdempotencyRepository::default()),
            Arc::new(StaticGovernancePort::new(
                true,
                datetime!(2026-05-26 08:00:00 UTC),
            )),
            Arc::new(InMemoryObjectStorage::default()),
            Arc::new(DeterministicFingerprintHasher::default()),
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
        .nest(
            API_BASE_PATH,
            Router::new()
                .route("/healthz", get(healthcheck))
                .route("/contents", post(create_method_content_draft))
                .route(
                    "/contents/{content_id}/draft",
                    patch(update_method_content_draft),
                )
                .route(
                    "/contents/{content_action}",
                    post(dispatch_method_content_action),
                ),
        )
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
    log_api_request(&gateway, "create_draft", "POST", "/contents", None);
    let request_hash = canonical_request_hash(&command).map_err(|error| {
        log_api_error(
            &gateway,
            "create_draft",
            "POST",
            "/contents",
            StatusCode::INTERNAL_SERVER_ERROR,
            None,
            &error,
        );
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
            log_api_error(
                &gateway,
                "create_draft",
                "POST",
                "/contents",
                StatusCode::BAD_REQUEST,
                None,
                &error,
            );
            error_response(
                StatusCode::BAD_REQUEST,
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error,
            )
        })?;

    match state
        .command_service
        .create_draft(command, gateway.actor.clone(), meta)
        .await
    {
        Ok(response) => {
            log_api_success(
                &gateway,
                "create_draft",
                "POST",
                "/contents",
                StatusCode::CREATED,
                Some(&response.content_id),
            );
            Ok((StatusCode::CREATED, Json(response)))
        }
        Err(error) => {
            let mapped = map_error_response(
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error.clone(),
            );
            log_api_error(
                &gateway,
                "create_draft",
                "POST",
                "/contents",
                mapped.0,
                None,
                &error,
            );
            Err(mapped)
        }
    }
}

async fn update_method_content_draft(
    State(state): State<AppState>,
    gateway: GatewayContextExtractor,
    Path(path_content_id): Path<String>,
    Json(command): Json<UpdateMethodContentDraftCommand>,
) -> Result<(StatusCode, Json<UpdateMethodContentDraftResponse>), (StatusCode, Json<ErrorResponse>)>
{
    let target_content_id = command.content_id.clone();
    log_api_request(
        &gateway,
        "update_draft",
        "PATCH",
        "/contents/{content_id}/draft",
        Some(&target_content_id),
    );
    validate_path_content_id(&path_content_id, &command.content_id).map_err(|error| {
        log_api_error(
            &gateway,
            "update_draft",
            "PATCH",
            "/contents/{content_id}/draft",
            StatusCode::BAD_REQUEST,
            Some(&target_content_id),
            &error,
        );
        error_response(
            StatusCode::BAD_REQUEST,
            gateway.headers.request_id.clone(),
            gateway.headers.trace_id.clone(),
            error,
        )
    })?;
    let request_hash = canonical_request_hash(&command).map_err(|error| {
        log_api_error(
            &gateway,
            "update_draft",
            "PATCH",
            "/contents/{content_id}/draft",
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(&target_content_id),
            &error,
        );
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
            log_api_error(
                &gateway,
                "update_draft",
                "PATCH",
                "/contents/{content_id}/draft",
                StatusCode::BAD_REQUEST,
                Some(&target_content_id),
                &error,
            );
            error_response(
                StatusCode::BAD_REQUEST,
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error,
            )
        })?;

    match state
        .command_service
        .update_draft(command, gateway.actor.clone(), meta)
        .await
    {
        Ok(response) => {
            log_api_success(
                &gateway,
                "update_draft",
                "PATCH",
                "/contents/{content_id}/draft",
                StatusCode::OK,
                Some(&response.content_id),
            );
            Ok((StatusCode::OK, Json(response)))
        }
        Err(error) => {
            let mapped = map_error_response(
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error.clone(),
            );
            log_api_error(
                &gateway,
                "update_draft",
                "PATCH",
                "/contents/{content_id}/draft",
                mapped.0,
                Some(&target_content_id),
                &error,
            );
            Err(mapped)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MethodContentAction {
    SubmitReview,
    Publish,
    Deprecate,
    Retire,
    Supersede,
}

async fn dispatch_method_content_action(
    State(state): State<AppState>,
    gateway: GatewayContextExtractor,
    Path(content_action): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let (path_content_id, action) =
        parse_method_content_action(&content_action).map_err(|error| {
            error_response(
                StatusCode::NOT_FOUND,
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error,
            )
        })?;

    match action {
        MethodContentAction::SubmitReview => {
            let command: SubmitMethodContentForReviewCommand = decode_action_command(
                payload,
                &gateway.headers.request_id,
                &gateway.headers.trace_id,
            )?;
            submit_method_content_for_review(&state, &gateway, path_content_id, command)
                .await
                .map(IntoResponse::into_response)
        }
        MethodContentAction::Publish => {
            let command: PublishMethodContentCommand = decode_action_command(
                payload,
                &gateway.headers.request_id,
                &gateway.headers.trace_id,
            )?;
            publish_method_content(&state, &gateway, path_content_id, command)
                .await
                .map(IntoResponse::into_response)
        }
        MethodContentAction::Deprecate => {
            let command: DeprecateMethodContentCommand = decode_action_command(
                payload,
                &gateway.headers.request_id,
                &gateway.headers.trace_id,
            )?;
            deprecate_method_content(&state, &gateway, path_content_id, command)
                .await
                .map(IntoResponse::into_response)
        }
        MethodContentAction::Retire => {
            let command: RetireMethodContentCommand = decode_action_command(
                payload,
                &gateway.headers.request_id,
                &gateway.headers.trace_id,
            )?;
            retire_method_content(&state, &gateway, path_content_id, command)
                .await
                .map(IntoResponse::into_response)
        }
        MethodContentAction::Supersede => {
            let command: SupersedeMethodContentCommand = decode_action_command(
                payload,
                &gateway.headers.request_id,
                &gateway.headers.trace_id,
            )?;
            supersede_method_content(&state, &gateway, path_content_id, command)
                .await
                .map(IntoResponse::into_response)
        }
    }
}

async fn submit_method_content_for_review(
    state: &AppState,
    gateway: &GatewayContextExtractor,
    path_content_id: &str,
    command: SubmitMethodContentForReviewCommand,
) -> Result<
    (StatusCode, Json<SubmitMethodContentForReviewResponse>),
    (StatusCode, Json<ErrorResponse>),
> {
    let target_content_id = command.content_id.clone();
    log_api_request(
        gateway,
        "submit_for_review",
        "POST",
        "/contents/{content_id}:submit-review",
        Some(&target_content_id),
    );
    validate_path_content_id(path_content_id, &command.content_id).map_err(|error| {
        log_api_error(
            gateway,
            "submit_for_review",
            "POST",
            "/contents/{content_id}:submit-review",
            StatusCode::BAD_REQUEST,
            Some(&target_content_id),
            &error,
        );
        error_response(
            StatusCode::BAD_REQUEST,
            gateway.headers.request_id.clone(),
            gateway.headers.trace_id.clone(),
            error,
        )
    })?;
    let request_hash = canonical_request_hash(&command).map_err(|error| {
        log_api_error(
            gateway,
            "submit_for_review",
            "POST",
            "/contents/{content_id}:submit-review",
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(&target_content_id),
            &error,
        );
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
            log_api_error(
                gateway,
                "submit_for_review",
                "POST",
                "/contents/{content_id}:submit-review",
                StatusCode::BAD_REQUEST,
                Some(&target_content_id),
                &error,
            );
            error_response(
                StatusCode::BAD_REQUEST,
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error,
            )
        })?;

    match state
        .command_service
        .submit_for_review(command, gateway.actor.clone(), meta)
        .await
    {
        Ok(response) => {
            log_api_success(
                gateway,
                "submit_for_review",
                "POST",
                "/contents/{content_id}:submit-review",
                StatusCode::OK,
                Some(&response.content_id),
            );
            Ok((StatusCode::OK, Json(response)))
        }
        Err(error) => {
            let mapped = map_error_response(
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error.clone(),
            );
            log_api_error(
                gateway,
                "submit_for_review",
                "POST",
                "/contents/{content_id}:submit-review",
                mapped.0,
                Some(&target_content_id),
                &error,
            );
            Err(mapped)
        }
    }
}

async fn publish_method_content(
    state: &AppState,
    gateway: &GatewayContextExtractor,
    path_content_id: &str,
    command: PublishMethodContentCommand,
) -> Result<(StatusCode, Json<PublishMethodContentResponse>), (StatusCode, Json<ErrorResponse>)> {
    let target_content_id = command.content_id.clone();
    log_api_request(
        gateway,
        "publish",
        "POST",
        "/contents/{content_id}:publish",
        Some(&target_content_id),
    );
    validate_path_content_id(path_content_id, &command.content_id).map_err(|error| {
        log_api_error(
            gateway,
            "publish",
            "POST",
            "/contents/{content_id}:publish",
            StatusCode::BAD_REQUEST,
            Some(&target_content_id),
            &error,
        );
        error_response(
            StatusCode::BAD_REQUEST,
            gateway.headers.request_id.clone(),
            gateway.headers.trace_id.clone(),
            error,
        )
    })?;
    let request_hash = canonical_request_hash(&command).map_err(|error| {
        log_api_error(
            gateway,
            "publish",
            "POST",
            "/contents/{content_id}:publish",
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(&target_content_id),
            &error,
        );
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
            log_api_error(
                gateway,
                "publish",
                "POST",
                "/contents/{content_id}:publish",
                StatusCode::BAD_REQUEST,
                Some(&target_content_id),
                &error,
            );
            error_response(
                StatusCode::BAD_REQUEST,
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error,
            )
        })?;

    match state
        .command_service
        .publish(command, gateway.actor.clone(), meta)
        .await
    {
        Ok(response) => {
            log_api_success(
                gateway,
                "publish",
                "POST",
                "/contents/{content_id}:publish",
                StatusCode::OK,
                Some(&response.content_id),
            );
            Ok((StatusCode::OK, Json(response)))
        }
        Err(error) => {
            let mapped = map_error_response(
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error.clone(),
            );
            log_api_error(
                gateway,
                "publish",
                "POST",
                "/contents/{content_id}:publish",
                mapped.0,
                Some(&target_content_id),
                &error,
            );
            Err(mapped)
        }
    }
}

async fn deprecate_method_content(
    state: &AppState,
    gateway: &GatewayContextExtractor,
    path_content_id: &str,
    command: DeprecateMethodContentCommand,
) -> Result<(StatusCode, Json<DeprecateMethodContentResponse>), (StatusCode, Json<ErrorResponse>)> {
    let target_content_id = command.content_id.clone();
    log_api_request(
        gateway,
        "deprecate",
        "POST",
        "/contents/{content_id}:deprecate",
        Some(&target_content_id),
    );
    validate_path_content_id(path_content_id, &command.content_id).map_err(|error| {
        log_api_error(
            gateway,
            "deprecate",
            "POST",
            "/contents/{content_id}:deprecate",
            StatusCode::BAD_REQUEST,
            Some(&target_content_id),
            &error,
        );
        error_response(
            StatusCode::BAD_REQUEST,
            gateway.headers.request_id.clone(),
            gateway.headers.trace_id.clone(),
            error,
        )
    })?;
    let request_hash = canonical_request_hash(&command).map_err(|error| {
        log_api_error(
            gateway,
            "deprecate",
            "POST",
            "/contents/{content_id}:deprecate",
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(&target_content_id),
            &error,
        );
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
            log_api_error(
                gateway,
                "deprecate",
                "POST",
                "/contents/{content_id}:deprecate",
                StatusCode::BAD_REQUEST,
                Some(&target_content_id),
                &error,
            );
            error_response(
                StatusCode::BAD_REQUEST,
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error,
            )
        })?;

    match state
        .command_service
        .deprecate(command, gateway.actor.clone(), meta)
        .await
    {
        Ok(response) => {
            log_api_success(
                gateway,
                "deprecate",
                "POST",
                "/contents/{content_id}:deprecate",
                StatusCode::OK,
                Some(&response.content_id),
            );
            Ok((StatusCode::OK, Json(response)))
        }
        Err(error) => {
            let mapped = map_error_response(
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error.clone(),
            );
            log_api_error(
                gateway,
                "deprecate",
                "POST",
                "/contents/{content_id}:deprecate",
                mapped.0,
                Some(&target_content_id),
                &error,
            );
            Err(mapped)
        }
    }
}

async fn retire_method_content(
    state: &AppState,
    gateway: &GatewayContextExtractor,
    path_content_id: &str,
    command: RetireMethodContentCommand,
) -> Result<(StatusCode, Json<RetireMethodContentResponse>), (StatusCode, Json<ErrorResponse>)> {
    let target_content_id = command.content_id.clone();
    log_api_request(
        gateway,
        "retire",
        "POST",
        "/contents/{content_id}:retire",
        Some(&target_content_id),
    );
    validate_path_content_id(path_content_id, &command.content_id).map_err(|error| {
        log_api_error(
            gateway,
            "retire",
            "POST",
            "/contents/{content_id}:retire",
            StatusCode::BAD_REQUEST,
            Some(&target_content_id),
            &error,
        );
        error_response(
            StatusCode::BAD_REQUEST,
            gateway.headers.request_id.clone(),
            gateway.headers.trace_id.clone(),
            error,
        )
    })?;
    let request_hash = canonical_request_hash(&command).map_err(|error| {
        log_api_error(
            gateway,
            "retire",
            "POST",
            "/contents/{content_id}:retire",
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(&target_content_id),
            &error,
        );
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
            log_api_error(
                gateway,
                "retire",
                "POST",
                "/contents/{content_id}:retire",
                StatusCode::BAD_REQUEST,
                Some(&target_content_id),
                &error,
            );
            error_response(
                StatusCode::BAD_REQUEST,
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error,
            )
        })?;

    match state
        .command_service
        .retire(command, gateway.actor.clone(), meta)
        .await
    {
        Ok(response) => {
            log_api_success(
                gateway,
                "retire",
                "POST",
                "/contents/{content_id}:retire",
                StatusCode::OK,
                Some(&response.content_id),
            );
            Ok((StatusCode::OK, Json(response)))
        }
        Err(error) => {
            let mapped = map_error_response(
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error.clone(),
            );
            log_api_error(
                gateway,
                "retire",
                "POST",
                "/contents/{content_id}:retire",
                mapped.0,
                Some(&target_content_id),
                &error,
            );
            Err(mapped)
        }
    }
}

async fn supersede_method_content(
    state: &AppState,
    gateway: &GatewayContextExtractor,
    path_old_content_id: &str,
    command: SupersedeMethodContentCommand,
) -> Result<(StatusCode, Json<SupersedeMethodContentResponse>), (StatusCode, Json<ErrorResponse>)> {
    let target_content_id = command.old_content_id.clone();
    log_api_request(
        gateway,
        "supersede",
        "POST",
        "/contents/{content_id}:supersede",
        Some(&target_content_id),
    );
    validate_path_content_id(path_old_content_id, &command.old_content_id).map_err(|error| {
        log_api_error(
            gateway,
            "supersede",
            "POST",
            "/contents/{content_id}:supersede",
            StatusCode::BAD_REQUEST,
            Some(&target_content_id),
            &error,
        );
        error_response(
            StatusCode::BAD_REQUEST,
            gateway.headers.request_id.clone(),
            gateway.headers.trace_id.clone(),
            error,
        )
    })?;
    let request_hash = canonical_request_hash(&command).map_err(|error| {
        log_api_error(
            gateway,
            "supersede",
            "POST",
            "/contents/{content_id}:supersede",
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(&target_content_id),
            &error,
        );
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
            log_api_error(
                gateway,
                "supersede",
                "POST",
                "/contents/{content_id}:supersede",
                StatusCode::BAD_REQUEST,
                Some(&target_content_id),
                &error,
            );
            error_response(
                StatusCode::BAD_REQUEST,
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error,
            )
        })?;

    match state
        .command_service
        .supersede(command, gateway.actor.clone(), meta)
        .await
    {
        Ok(response) => {
            log_api_success(
                gateway,
                "supersede",
                "POST",
                "/contents/{content_id}:supersede",
                StatusCode::OK,
                Some(&response.old_content_id),
            );
            Ok((StatusCode::OK, Json(response)))
        }
        Err(error) => {
            let mapped = map_error_response(
                gateway.headers.request_id.clone(),
                gateway.headers.trace_id.clone(),
                error.clone(),
            );
            log_api_error(
                gateway,
                "supersede",
                "POST",
                "/contents/{content_id}:supersede",
                mapped.0,
                Some(&target_content_id),
                &error,
            );
            Err(mapped)
        }
    }
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

fn decode_action_command<T: DeserializeOwned>(
    payload: serde_json::Value,
    request_id: &str,
    trace_id: &str,
) -> Result<T, (StatusCode, Json<ErrorResponse>)> {
    serde_json::from_value(payload).map_err(|error| {
        error_response(
            StatusCode::BAD_REQUEST,
            request_id.to_string(),
            trace_id.to_string(),
            MethodLibraryError::validation(
                MethodLibraryErrorCode::BoundaryViolation,
                format!("invalid action request body: {error}"),
            ),
        )
    })
}

fn parse_method_content_action(
    content_action: &str,
) -> Result<(&str, MethodContentAction), MethodLibraryError> {
    let Some((content_id, action)) = content_action.rsplit_once(':') else {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "unsupported content action path",
        ));
    };

    if content_id.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "content action path must include a content id",
        ));
    }

    let action = match action {
        "submit-review" => MethodContentAction::SubmitReview,
        "publish" => MethodContentAction::Publish,
        "deprecate" => MethodContentAction::Deprecate,
        "retire" => MethodContentAction::Retire,
        "supersede" => MethodContentAction::Supersede,
        _ => {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::BoundaryViolation,
                "unsupported content action path",
            ));
        }
    };

    Ok((content_id, action))
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
        | MethodLibraryErrorCode::PayloadKindMismatch
        | MethodLibraryErrorCode::ReferenceNotPublished
        | MethodLibraryErrorCode::SupersedeKindMismatch
        | MethodLibraryErrorCode::SupersedeTargetRequired => StatusCode::UNPROCESSABLE_ENTITY,
        MethodLibraryErrorCode::PublishGateRequired
        | MethodLibraryErrorCode::PublishGateInvalid => StatusCode::FAILED_DEPENDENCY,
        MethodLibraryErrorCode::IdempotencyConflict
        | MethodLibraryErrorCode::IdempotencyStatusConflict
        | MethodLibraryErrorCode::LifecycleTransitionNotAllowed
        | MethodLibraryErrorCode::PublishedContentImmutable
        | MethodLibraryErrorCode::SupersedeConflict
        | MethodLibraryErrorCode::ContentVersionConflict => StatusCode::CONFLICT,
        MethodLibraryErrorCode::MethodContentNotFound => StatusCode::NOT_FOUND,
        MethodLibraryErrorCode::RevisionConflict => StatusCode::PRECONDITION_FAILED,
        MethodLibraryErrorCode::GovernanceUnavailable
        | MethodLibraryErrorCode::ObjectStorageUnavailable
        | MethodLibraryErrorCode::PersistenceUnavailable
        | MethodLibraryErrorCode::TransactionCommitFailed => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };

    error_response(status, request_id, trace_id, error)
}

fn log_api_request(
    gateway: &GatewayContextExtractor,
    operation: &str,
    method: &str,
    path_template: &str,
    target_content_id: Option<&str>,
) {
    info!(
        request_id = %gateway.headers.request_id,
        trace_id = %gateway.headers.trace_id,
        operation,
        method,
        path_template,
        actor_id = %gateway.actor.actor_id,
        actor_kind = ?gateway.actor.actor_kind,
        idempotency_key_present = gateway.headers.idempotency_key.is_some(),
        target_content_id = target_content_id.unwrap_or(""),
        "received api request"
    );
}

fn log_api_success(
    gateway: &GatewayContextExtractor,
    operation: &str,
    method: &str,
    path_template: &str,
    status: StatusCode,
    target_content_id: Option<&str>,
) {
    info!(
        request_id = %gateway.headers.request_id,
        trace_id = %gateway.headers.trace_id,
        operation,
        method,
        path_template,
        status = status.as_u16(),
        target_content_id = target_content_id.unwrap_or(""),
        "completed api request"
    );
}

fn log_api_error(
    gateway: &GatewayContextExtractor,
    operation: &str,
    method: &str,
    path_template: &str,
    status: StatusCode,
    target_content_id: Option<&str>,
    error_value: &MethodLibraryError,
) {
    let dependency = dependency_for_error(error_value.code).unwrap_or("");
    if status.is_server_error() {
        error!(
            request_id = %gateway.headers.request_id,
            trace_id = %gateway.headers.trace_id,
            operation,
            method,
            path_template,
            status = status.as_u16(),
            target_content_id = target_content_id.unwrap_or(""),
            error_code = error_value.code.as_str(),
            retryable = error_value.retryable,
            dependency,
            "api request failed"
        );
    } else {
        warn!(
            request_id = %gateway.headers.request_id,
            trace_id = %gateway.headers.trace_id,
            operation,
            method,
            path_template,
            status = status.as_u16(),
            target_content_id = target_content_id.unwrap_or(""),
            error_code = error_value.code.as_str(),
            retryable = error_value.retryable,
            dependency,
            "api request failed"
        );
    }
}

fn dependency_for_error(error_code: MethodLibraryErrorCode) -> Option<&'static str> {
    match error_code {
        MethodLibraryErrorCode::PublishGateInvalid
        | MethodLibraryErrorCode::GovernanceUnavailable => Some("governance"),
        MethodLibraryErrorCode::ObjectStorageUnavailable => Some("object_storage"),
        MethodLibraryErrorCode::PersistenceUnavailable
        | MethodLibraryErrorCode::TransactionCommitFailed => Some("database"),
        _ => None,
    }
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

    use super::{API_BASE_PATH, AppState, router, router_with_state};
    use method_library_application::ports::Clock;
    use method_library_application::ports::fakes::{
        DeterministicClock, DeterministicFingerprintHasher, DeterministicIdGenerator,
        FakeUnitOfWork, InMemoryAuditRepository, InMemoryDefinitionSnapshotRepository,
        InMemoryIdempotencyRepository, InMemoryLifecycleHistoryRepository,
        InMemoryMethodContentReferenceRepository, InMemoryMethodContentRepository,
        InMemoryMethodContentVersionRepository, InMemoryObjectStorage, InMemoryOutboxRepository,
        InMemorySupersedeLinkRepository, StaticGovernancePort,
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
        Arc<InMemoryMethodContentVersionRepository>,
        Arc<InMemoryDefinitionSnapshotRepository>,
        Arc<InMemoryOutboxRepository>,
        Arc<InMemoryObjectStorage>,
        Arc<InMemoryLifecycleHistoryRepository>,
    ) {
        let clock: Arc<dyn Clock> =
            Arc::new(DeterministicClock::new(datetime!(2026-05-26 08:00:00 UTC)));
        let content_repository = Arc::new(InMemoryMethodContentRepository::default());
        let reference_repository = Arc::new(InMemoryMethodContentReferenceRepository::default());
        let version_repository = Arc::new(InMemoryMethodContentVersionRepository::default());
        let snapshot_repository = Arc::new(InMemoryDefinitionSnapshotRepository::default());
        let supersede_link_repository = Arc::new(InMemorySupersedeLinkRepository::default());
        let outbox_repository = Arc::new(InMemoryOutboxRepository::default());
        let object_storage = Arc::new(InMemoryObjectStorage::default());
        let lifecycle_history_repository = Arc::new(InMemoryLifecycleHistoryRepository::default());
        let audit_repository = Arc::new(InMemoryAuditRepository::default());
        let idempotency_repository = Arc::new(InMemoryIdempotencyRepository::default());
        let governance_port = Arc::new(StaticGovernancePort::new(
            true,
            datetime!(2026-05-26 08:00:00 UTC),
        ));
        let fingerprint_hasher = Arc::new(DeterministicFingerprintHasher::default());
        let command_service = Arc::new(MethodContentCommandService::new(
            Arc::new(FakeUnitOfWork),
            content_repository.clone(),
            reference_repository,
            version_repository.clone(),
            snapshot_repository.clone(),
            supersede_link_repository,
            outbox_repository.clone(),
            lifecycle_history_repository.clone(),
            audit_repository,
            idempotency_repository,
            governance_port,
            object_storage.clone(),
            fingerprint_hasher,
            clock.clone(),
            Arc::new(DeterministicIdGenerator::default()),
        ));

        (
            AppState::new(clock, command_service),
            content_repository,
            version_repository,
            snapshot_repository,
            outbox_repository,
            object_storage,
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

    fn sample_publish_body(
        content_id: &str,
        expected_revision: i64,
        gate_id: &str,
        gate_decision_id: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "content_id": content_id,
            "expected_revision": expected_revision,
            "version": "1.0.0",
            "approved_gate_ref": {
                "gate_id": gate_id,
                "gate_decision_id": gate_decision_id,
                "approved_at": "2026-05-26T08:10:00Z"
            },
            "publish_reason": "Initial release"
        })
    }

    fn sample_deprecate_body(content_id: &str, expected_revision: i64) -> serde_json::Value {
        serde_json::json!({
            "content_id": content_id,
            "expected_revision": expected_revision,
            "reason": "Superseded by newer guidance",
            "effective_at": "2026-05-26T08:20:00Z"
        })
    }

    fn sample_retire_body(content_id: &str, expected_revision: i64) -> serde_json::Value {
        serde_json::json!({
            "content_id": content_id,
            "expected_revision": expected_revision,
            "reason": "Retired for trace-only retention",
            "retire_policy": "stop_new_usage"
        })
    }

    fn sample_supersede_body(
        old_content_id: &str,
        old_expected_revision: i64,
        new_content_id: &str,
        new_expected_revision: i64,
        gate_id: &str,
        gate_decision_id: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "old_content_id": old_content_id,
            "old_expected_revision": old_expected_revision,
            "new_content_id": new_content_id,
            "new_expected_revision": new_expected_revision,
            "new_version": "2.0.0",
            "approved_gate_ref": {
                "gate_id": gate_id,
                "gate_decision_id": gate_decision_id,
                "approved_at": "2026-05-26T08:15:00Z"
            },
            "reason": "Replaced by a newer definition"
        })
    }

    fn api_path(path: &str) -> String {
        format!("{API_BASE_PATH}{path}")
    }

    #[tokio::test]
    async fn responds_to_healthcheck() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri(api_path("/healthz"))
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
            .uri(api_path("/contents"))
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
                api_path("/contents"),
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
                api_path("/contents/content-1/draft"),
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
                api_path("/contents"),
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
                api_path("/contents/content-1:submit-review"),
                Some("idem-2"),
                sample_submit_body("content-1", 1),
            ))
            .await
            .expect("submit route should respond");

        assert_eq!(submit_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn publishes_draft_via_http() {
        let (state, content_repository, _, _, _, _, _) = test_state();
        seed_published_content(&content_repository, "content-ref-1").await;
        let app = router_with_state(state);

        let create_response = app
            .clone()
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents"),
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
                    "references": [
                        {
                            "target_content_id": "content-ref-1",
                            "target_kind": "qualification",
                            "required_state": "published"
                        }
                    ],
                    "source_refs": []
                }),
            ))
            .await
            .expect("create route should respond");
        assert_eq!(create_response.status(), StatusCode::CREATED);

        let submit_response = app
            .clone()
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents/content-1:submit-review"),
                Some("idem-2"),
                sample_submit_body("content-1", 1),
            ))
            .await
            .expect("submit route should respond");
        assert_eq!(submit_response.status(), StatusCode::OK);

        let publish_response = app
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents/content-1:publish"),
                Some("idem-3"),
                sample_publish_body("content-1", 2, "gate-1", "decision-1"),
            ))
            .await
            .expect("publish route should respond");

        assert_eq!(publish_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_publish_without_gate_via_http() {
        let (state, content_repository, _, _, _, _, _) = test_state();
        seed_published_content(&content_repository, "content-ref-1").await;
        let app = router_with_state(state);

        let create_response = app
            .clone()
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents"),
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
                    "references": [
                        {
                            "target_content_id": "content-ref-1",
                            "target_kind": "qualification",
                            "required_state": "published"
                        }
                    ],
                    "source_refs": []
                }),
            ))
            .await
            .expect("create route should respond");
        assert_eq!(create_response.status(), StatusCode::CREATED);

        let submit_response = app
            .clone()
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents/content-1:submit-review"),
                Some("idem-2"),
                sample_submit_body("content-1", 1),
            ))
            .await
            .expect("submit route should respond");
        assert_eq!(submit_response.status(), StatusCode::OK);

        let response = app
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents/content-1:publish"),
                Some("idem-3"),
                serde_json::json!({
                    "content_id": "content-1",
                    "expected_revision": 2,
                    "version": "1.0.0",
                    "approved_gate_ref": {
                        "gate_id": "",
                        "gate_decision_id": "",
                        "approved_at": "2026-05-26T08:10:00Z"
                    },
                    "publish_reason": "Initial release"
                }),
            ))
            .await
            .expect("publish route should respond");

        assert_error_code(
            response,
            StatusCode::FAILED_DEPENDENCY,
            MethodLibraryErrorCode::PublishGateRequired,
        )
        .await;
    }

    #[tokio::test]
    async fn rejects_publish_with_unpublished_reference_via_http() {
        let (state, content_repository, _, _, _, _, _) = test_state();
        seed_draft_content(&content_repository, "content-ref-1").await;
        let app = router_with_state(state);

        let create_response = app
            .clone()
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents"),
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
                    "references": [
                        {
                            "target_content_id": "content-ref-1",
                            "target_kind": "qualification",
                            "required_state": "published"
                        }
                    ],
                    "source_refs": []
                }),
            ))
            .await
            .expect("create route should respond");
        assert_eq!(create_response.status(), StatusCode::CREATED);

        let submit_response = app
            .clone()
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents/content-1:submit-review"),
                Some("idem-2"),
                sample_submit_body("content-1", 1),
            ))
            .await
            .expect("submit route should respond");
        assert_eq!(submit_response.status(), StatusCode::OK);

        let response = app
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents/content-1:publish"),
                Some("idem-3"),
                sample_publish_body("content-1", 2, "gate-1", "decision-1"),
            ))
            .await
            .expect("publish route should respond");

        assert_error_code(
            response,
            StatusCode::UNPROCESSABLE_ENTITY,
            MethodLibraryErrorCode::ReferenceNotPublished,
        )
        .await;
    }

    #[tokio::test]
    async fn deprecates_published_content_via_http() {
        let (state, content_repository, _, _, _, _, _) = test_state();
        seed_published_content(&content_repository, "content-published").await;
        let app = router_with_state(state);

        let response = app
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents/content-published:deprecate"),
                Some("idem-4"),
                sample_deprecate_body("content-published", 3),
            ))
            .await
            .expect("deprecate route should respond");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn retires_published_content_via_http() {
        let (state, content_repository, _, _, _, _, _) = test_state();
        seed_published_content(&content_repository, "content-published").await;
        let app = router_with_state(state);

        let response = app
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents/content-published:retire"),
                Some("idem-4"),
                sample_retire_body("content-published", 3),
            ))
            .await
            .expect("retire route should respond");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn supersedes_content_via_http() {
        let (state, content_repository, _, _, _, _, _) = test_state();
        seed_published_content(&content_repository, "content-old").await;
        seed_in_review_content(&content_repository, "content-new").await;
        let app = router_with_state(state);

        let response = app
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents/content-old:supersede"),
                Some("idem-4"),
                sample_supersede_body("content-old", 3, "content-new", 2, "gate-1", "decision-1"),
            ))
            .await
            .expect("supersede route should respond");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_supersede_path_body_mismatch_via_http() {
        let response = router()
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents/content-a:supersede"),
                Some("idem-4"),
                sample_supersede_body("content-b", 3, "content-new", 2, "gate-1", "decision-1"),
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
    async fn rejects_missing_idempotency_key_via_http() {
        let response = router()
            .oneshot(build_gateway_request(
                "POST",
                api_path("/contents"),
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
                api_path("/contents"),
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
                api_path("/contents/content-1/draft"),
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
        let (state, content_repository, _, _, _, _, _) = test_state();
        seed_published_content(&content_repository, "content-published").await;
        let app = router_with_state(state);

        let response = app
            .oneshot(build_gateway_request(
                "PATCH",
                api_path("/contents/content-published/draft"),
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
        uri: impl AsRef<str>,
        idempotency_key: Option<&str>,
        body: serde_json::Value,
    ) -> Request<Body> {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri.as_ref())
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

    async fn seed_published_content(
        repository: &Arc<InMemoryMethodContentRepository>,
        content_id: &str,
    ) -> String {
        let actor_id = "actor-1".to_string();
        let mut content = MethodContent::create_draft(
            content_id.to_string(),
            format!("family-{content_id}"),
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

        content_id.to_string()
    }

    async fn seed_in_review_content(
        repository: &Arc<InMemoryMethodContentRepository>,
        content_id: &str,
    ) -> String {
        let actor_id = "actor-1".to_string();
        let mut content = MethodContent::create_draft(
            content_id.to_string(),
            format!("family-{content_id}"),
            MethodContentKind::Qualification,
            "Quality Replacement".to_string(),
            None,
            MethodContentPayload::Qualification(Qualification {
                qualification_key: "quality-2".to_string(),
                name: "Quality Replacement".to_string(),
                description: None,
                level_model: QualificationLevelModel {
                    levels: vec![QualificationLevel {
                        level_key: "advanced".to_string(),
                        name: "Advanced".to_string(),
                        order: 1,
                        description: None,
                    }],
                    default_level_key: Some("advanced".to_string()),
                },
                evidence_rules: vec![EvidenceRule {
                    evidence_kind: EvidenceKind::Document,
                    required: true,
                    description: "Replacement proof".to_string(),
                }],
            }),
            actor_id.clone(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("draft should build");
        content
            .submit_for_review(actor_id, datetime!(2026-05-26 08:05:00 UTC))
            .expect("review submission should work");

        let meta = RequestMeta {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            idempotency_key: Some("idem-seed-review".to_string()),
            request_hash: "hash-seed-review".to_string(),
            received_at: datetime!(2026-05-26 08:00:00 UTC),
        };
        let mut tx = FakeUnitOfWork
            .begin(meta)
            .await
            .expect("transaction should open");
        repository
            .insert(&mut tx, content)
            .await
            .expect("in-review content should insert");
        tx.commit().await.expect("transaction should commit");

        content_id.to_string()
    }

    async fn seed_draft_content(
        repository: &Arc<InMemoryMethodContentRepository>,
        content_id: &str,
    ) -> String {
        let actor_id = "actor-1".to_string();
        let content = MethodContent::create_draft(
            content_id.to_string(),
            format!("family-{content_id}"),
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

        let meta = RequestMeta {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            idempotency_key: Some("idem-seed-draft".to_string()),
            request_hash: "hash-seed-draft".to_string(),
            received_at: datetime!(2026-05-26 08:00:00 UTC),
        };
        let mut tx = FakeUnitOfWork
            .begin(meta)
            .await
            .expect("transaction should open");
        repository
            .insert(&mut tx, content)
            .await
            .expect("draft content should insert");
        tx.commit().await.expect("transaction should commit");

        content_id.to_string()
    }
}
