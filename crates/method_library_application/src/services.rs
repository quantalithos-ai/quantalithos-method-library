//! Application services that orchestrate P0 method-content command flows.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use method_library_contracts::{
    ActorContext, ArtifactRef, CreateMethodContentDraftCommand, CreateMethodContentDraftResponse,
    RequestMeta, SubmitMethodContentForReviewCommand, SubmitMethodContentForReviewResponse,
    UpdateMethodContentDraftCommand, UpdateMethodContentDraftResponse,
};
use method_library_domain::content::{ContentRef, LifecycleState, MethodContent};
use method_library_domain::definitions::MethodContentPayload;
use method_library_domain::policies::DefinitionUseBoundaryGuard;
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};

use crate::ports::{
    AuditRecord, AuditRepository, AuditTargetRef, Clock, FailureReason, IdGenerator,
    IdempotencyBeginResult, IdempotencyRepository, IdempotencyScope, LifecycleHistoryEntry,
    LifecycleHistoryRepository, MethodContentReferenceRepository, MethodContentRepository,
    ResultRef, UnitOfWork,
};

const CREATE_DRAFT_SCOPE: &str = "command:create_draft";
const UPDATE_DRAFT_SCOPE: &str = "command:update_draft";
const SUBMIT_REVIEW_SCOPE: &str = "command:submit_for_review";

/// Application service for method-content command flows.
#[derive(Clone)]
pub struct MethodContentCommandService {
    unit_of_work: Arc<dyn UnitOfWork>,
    method_content_repository: Arc<dyn MethodContentRepository>,
    method_content_reference_repository: Arc<dyn MethodContentReferenceRepository>,
    lifecycle_history_repository: Arc<dyn LifecycleHistoryRepository>,
    audit_repository: Arc<dyn AuditRepository>,
    idempotency_repository: Arc<dyn IdempotencyRepository>,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl MethodContentCommandService {
    /// Creates a command service from port implementations.
    #[must_use]
    pub fn new(
        unit_of_work: Arc<dyn UnitOfWork>,
        method_content_repository: Arc<dyn MethodContentRepository>,
        method_content_reference_repository: Arc<dyn MethodContentReferenceRepository>,
        lifecycle_history_repository: Arc<dyn LifecycleHistoryRepository>,
        audit_repository: Arc<dyn AuditRepository>,
        idempotency_repository: Arc<dyn IdempotencyRepository>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Self {
        Self {
            unit_of_work,
            method_content_repository,
            method_content_reference_repository,
            lifecycle_history_repository,
            audit_repository,
            idempotency_repository,
            clock,
            id_generator,
        }
    }

    /// Creates a new draft method-content aggregate.
    pub async fn create_draft(
        &self,
        command: CreateMethodContentDraftCommand,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<CreateMethodContentDraftResponse, MethodLibraryError> {
        validate_create_draft_command(&command)?;
        let idempotency_key = meta.require_idempotency_key()?.clone();
        let mut tx = self.unit_of_work.begin(meta.clone()).await?;
        let now = self.clock.now();
        let scope: IdempotencyScope = CREATE_DRAFT_SCOPE.to_string();

        let operation = async {
            match self
                .idempotency_repository
                .try_begin(
                    &mut tx,
                    idempotency_key.clone(),
                    scope.clone(),
                    meta.request_hash.clone(),
                    now,
                )
                .await?
            {
                IdempotencyBeginResult::Started => {
                    let audit_id = self.id_generator.new_audit_id();
                    let CreateMethodContentDraftCommand {
                        kind,
                        name,
                        description,
                        payload,
                        references,
                        source_refs,
                    } = command;
                    let content_id = self.id_generator.new_content_id();
                    let content_family_id = self.id_generator.new_content_family_id();
                    let mut content = MethodContent::create_draft(
                        content_id.clone(),
                        content_family_id.clone(),
                        kind,
                        name,
                        description,
                        payload,
                        actor.actor_id.clone(),
                        now,
                    )?;

                    let merged_refs =
                        merge_content_refs(content.references.clone(), references.clone());
                    let reference_count = merged_refs.len();
                    content.replace_references(merged_refs.clone(), actor.actor_id.clone(), now)?;

                    let response = build_create_response(&content);
                    self.method_content_repository
                        .insert(&mut tx, content)
                        .await?;
                    self.method_content_reference_repository
                        .replace_refs(&mut tx, response.content_id.clone(), merged_refs)
                        .await?;
                    self.audit_repository
                        .append(
                            &mut tx,
                            build_audit_record(
                                &response.content_id,
                                &actor,
                                &meta,
                                "create_draft",
                                "succeeded",
                                build_create_details(&response, reference_count, source_refs.len()),
                                audit_id,
                            ),
                        )
                        .await?;
                    self.idempotency_repository
                        .mark_completed(
                            &mut tx,
                            idempotency_key.clone(),
                            scope.clone(),
                            response.content_id.clone(),
                            now,
                        )
                        .await?;

                    Ok(response)
                }
                IdempotencyBeginResult::Succeeded(result_ref) => {
                    load_existing_create_response(
                        &self.method_content_repository,
                        &mut tx,
                        result_ref,
                    )
                    .await
                }
                IdempotencyBeginResult::Failed(reason) => Err(failure_reason_to_error(reason)),
                IdempotencyBeginResult::Processing => Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::IdempotencyStatusConflict,
                    "create draft request is still processing",
                )),
            }
        }
        .await;

        match operation {
            Ok(response) => {
                tx.commit().await?;
                Ok(response)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    /// Updates an existing draft method-content aggregate.
    pub async fn update_draft(
        &self,
        command: UpdateMethodContentDraftCommand,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<UpdateMethodContentDraftResponse, MethodLibraryError> {
        validate_update_draft_command(&command)?;
        let idempotency_key = meta.require_idempotency_key()?.clone();
        let mut tx = self.unit_of_work.begin(meta.clone()).await?;
        let now = self.clock.now();
        let scope: IdempotencyScope = UPDATE_DRAFT_SCOPE.to_string();

        let operation = async {
            match self
                .idempotency_repository
                .try_begin(
                    &mut tx,
                    idempotency_key.clone(),
                    scope.clone(),
                    meta.request_hash.clone(),
                    now,
                )
                .await?
            {
                IdempotencyBeginResult::Started => {
                    let audit_id = self.id_generator.new_audit_id();
                    let UpdateMethodContentDraftCommand {
                        content_id,
                        expected_revision,
                        name,
                        description,
                        payload,
                        references,
                    } = command;

                    let mut content = self
                        .method_content_repository
                        .get_for_update(&mut tx, content_id.clone())
                        .await?
                        .ok_or_else(|| {
                            MethodLibraryError::validation(
                                MethodLibraryErrorCode::MethodContentNotFound,
                                "method content does not exist",
                            )
                        })?;

                    content.ensure_revision(expected_revision)?;
                    DefinitionUseBoundaryGuard::ensure_definition_only(&payload)?;
                    content.update_draft(
                        name,
                        description,
                        payload,
                        actor.actor_id.clone(),
                        now,
                    )?;

                    let merged_refs = merge_content_refs(content.references.clone(), references);
                    let reference_count = merged_refs.len();
                    content.replace_references(merged_refs.clone(), actor.actor_id.clone(), now)?;

                    let response = build_update_response(&content);
                    self.method_content_reference_repository
                        .replace_refs(&mut tx, response.content_id.clone(), merged_refs)
                        .await?;
                    self.method_content_repository
                        .save(&mut tx, content, expected_revision)
                        .await?;
                    self.audit_repository
                        .append(
                            &mut tx,
                            build_audit_record(
                                &response.content_id,
                                &actor,
                                &meta,
                                "update_draft",
                                "succeeded",
                                build_update_details(&response, expected_revision, reference_count),
                                audit_id,
                            ),
                        )
                        .await?;
                    self.idempotency_repository
                        .mark_completed(
                            &mut tx,
                            idempotency_key.clone(),
                            scope.clone(),
                            response.content_id.clone(),
                            now,
                        )
                        .await?;

                    Ok(response)
                }
                IdempotencyBeginResult::Succeeded(result_ref) => {
                    load_existing_update_response(
                        &self.method_content_repository,
                        &mut tx,
                        result_ref,
                    )
                    .await
                }
                IdempotencyBeginResult::Failed(reason) => Err(failure_reason_to_error(reason)),
                IdempotencyBeginResult::Processing => Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::IdempotencyStatusConflict,
                    "update draft request is still processing",
                )),
            }
        }
        .await;

        match operation {
            Ok(response) => {
                tx.commit().await?;
                Ok(response)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    /// Submits a draft method-content aggregate for review.
    pub async fn submit_for_review(
        &self,
        command: SubmitMethodContentForReviewCommand,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<SubmitMethodContentForReviewResponse, MethodLibraryError> {
        validate_submit_for_review_command(&command)?;
        let idempotency_key = meta.require_idempotency_key()?.clone();
        let mut tx = self.unit_of_work.begin(meta.clone()).await?;
        let now = self.clock.now();
        let scope: IdempotencyScope = SUBMIT_REVIEW_SCOPE.to_string();

        let operation = async {
            match self
                .idempotency_repository
                .try_begin(
                    &mut tx,
                    idempotency_key.clone(),
                    scope.clone(),
                    meta.request_hash.clone(),
                    now,
                )
                .await?
            {
                IdempotencyBeginResult::Started => {
                    let audit_id = self.id_generator.new_audit_id();
                    let history_entry_id = self.id_generator.new_history_entry_id();
                    let SubmitMethodContentForReviewCommand {
                        content_id,
                        expected_revision,
                        review_reason,
                        review_evidence_refs,
                    } = command;

                    let mut content = self
                        .method_content_repository
                        .get_for_update(&mut tx, content_id.clone())
                        .await?
                        .ok_or_else(|| {
                            MethodLibraryError::validation(
                                MethodLibraryErrorCode::MethodContentNotFound,
                                "method content does not exist",
                            )
                        })?;

                    content.ensure_revision(expected_revision)?;
                    let from_state = content.lifecycle.state;
                    content.submit_for_review(actor.actor_id.clone(), now)?;

                    let response = build_submit_response(&content);
                    self.lifecycle_history_repository
                        .append(
                            &mut tx,
                            build_lifecycle_history_entry(
                                history_entry_id,
                                &content.content_id,
                                from_state,
                                response.lifecycle_state,
                                &actor.actor_id,
                                review_reason.clone(),
                                now,
                            ),
                        )
                        .await?;
                    self.method_content_repository
                        .save(&mut tx, content, expected_revision)
                        .await?;
                    self.audit_repository
                        .append(
                            &mut tx,
                            build_audit_record(
                                &response.content_id,
                                &actor,
                                &meta,
                                "submit_for_review",
                                "succeeded",
                                build_submit_details(
                                    &response,
                                    expected_revision,
                                    review_reason,
                                    review_evidence_refs.len(),
                                ),
                                audit_id,
                            ),
                        )
                        .await?;
                    self.idempotency_repository
                        .mark_completed(
                            &mut tx,
                            idempotency_key.clone(),
                            scope.clone(),
                            response.content_id.clone(),
                            now,
                        )
                        .await?;

                    Ok(response)
                }
                IdempotencyBeginResult::Succeeded(result_ref) => {
                    load_existing_submit_response(
                        &self.method_content_repository,
                        &mut tx,
                        result_ref,
                    )
                    .await
                }
                IdempotencyBeginResult::Failed(reason) => Err(failure_reason_to_error(reason)),
                IdempotencyBeginResult::Processing => Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::IdempotencyStatusConflict,
                    "submit review request is still processing",
                )),
            }
        }
        .await;

        match operation {
            Ok(response) => {
                tx.commit().await?;
                Ok(response)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }
}

fn build_create_response(content: &MethodContent) -> CreateMethodContentDraftResponse {
    CreateMethodContentDraftResponse {
        content_id: content.content_id.clone(),
        content_family_id: content.content_family_id.clone(),
        kind: content.kind,
        lifecycle_state: content.lifecycle.state,
        revision: content.revision,
    }
}

fn build_update_response(content: &MethodContent) -> UpdateMethodContentDraftResponse {
    UpdateMethodContentDraftResponse {
        content_id: content.content_id.clone(),
        kind: content.kind,
        lifecycle_state: content.lifecycle.state,
        revision: content.revision,
    }
}

fn build_submit_response(content: &MethodContent) -> SubmitMethodContentForReviewResponse {
    SubmitMethodContentForReviewResponse {
        content_id: content.content_id.clone(),
        kind: content.kind,
        lifecycle_state: content.lifecycle.state,
        revision: content.revision,
    }
}

fn build_audit_record(
    target_id: &str,
    actor: &ActorContext,
    meta: &RequestMeta,
    action: &str,
    result: &str,
    details: BTreeMap<String, String>,
    audit_id: String,
) -> AuditRecord {
    AuditRecord {
        audit_id,
        request_id: meta.request_id.clone(),
        trace_id: meta.trace_id.clone(),
        actor_context: actor.clone(),
        target_ref: AuditTargetRef {
            target_type: "method_content".to_string(),
            target_id: target_id.to_string(),
        },
        action: action.to_string(),
        result: result.to_string(),
        details,
        occurred_at: meta.received_at,
    }
}

fn build_create_details(
    response: &CreateMethodContentDraftResponse,
    reference_count: usize,
    source_ref_count: usize,
) -> BTreeMap<String, String> {
    let mut details = BTreeMap::new();
    details.insert("kind".to_string(), response.kind.as_str().to_string());
    details.insert(
        "content_family_id".to_string(),
        response.content_family_id.clone(),
    );
    details.insert("revision".to_string(), response.revision.to_string());
    details.insert("reference_count".to_string(), reference_count.to_string());
    details.insert("source_ref_count".to_string(), source_ref_count.to_string());
    details
}

fn build_update_details(
    response: &UpdateMethodContentDraftResponse,
    expected_revision: i64,
    reference_count: usize,
) -> BTreeMap<String, String> {
    let mut details = BTreeMap::new();
    details.insert("kind".to_string(), response.kind.as_str().to_string());
    details.insert(
        "expected_revision".to_string(),
        expected_revision.to_string(),
    );
    details.insert("revision".to_string(), response.revision.to_string());
    details.insert("reference_count".to_string(), reference_count.to_string());
    details
}

fn build_submit_details(
    response: &SubmitMethodContentForReviewResponse,
    expected_revision: i64,
    review_reason: Option<String>,
    review_evidence_ref_count: usize,
) -> BTreeMap<String, String> {
    let mut details = BTreeMap::new();
    details.insert("kind".to_string(), response.kind.as_str().to_string());
    details.insert(
        "expected_revision".to_string(),
        expected_revision.to_string(),
    );
    details.insert("revision".to_string(), response.revision.to_string());
    details.insert(
        "review_evidence_ref_count".to_string(),
        review_evidence_ref_count.to_string(),
    );
    if let Some(reason) = review_reason {
        details.insert("review_reason".to_string(), reason);
    }
    details
}

fn build_lifecycle_history_entry(
    history_entry_id: String,
    content_id: &str,
    from_state: LifecycleState,
    to_state: LifecycleState,
    actor_id: &str,
    reason: Option<String>,
    now: method_library_domain::content::Timestamp,
) -> LifecycleHistoryEntry {
    LifecycleHistoryEntry {
        history_entry_id,
        content_id: content_id.to_string(),
        from_state: Some(from_state),
        to_state,
        actor_id: actor_id.to_string(),
        reason,
        created_at: now,
    }
}

fn failure_reason_to_error(reason: FailureReason) -> MethodLibraryError {
    MethodLibraryError::new(reason.code, reason.message, reason.retryable)
}

async fn load_existing_create_response(
    method_content_repository: &Arc<dyn MethodContentRepository>,
    tx: &mut crate::ports::UnitOfWorkTx,
    result_ref: ResultRef,
) -> Result<CreateMethodContentDraftResponse, MethodLibraryError> {
    let content = method_content_repository
        .get_for_update(tx, result_ref)
        .await?
        .ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::MethodContentNotFound,
                "idempotent create draft result could not be reloaded",
            )
        })?;

    Ok(build_create_response(&content))
}

async fn load_existing_update_response(
    method_content_repository: &Arc<dyn MethodContentRepository>,
    tx: &mut crate::ports::UnitOfWorkTx,
    result_ref: ResultRef,
) -> Result<UpdateMethodContentDraftResponse, MethodLibraryError> {
    let content = method_content_repository
        .get_for_update(tx, result_ref)
        .await?
        .ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::MethodContentNotFound,
                "idempotent update draft result could not be reloaded",
            )
        })?;

    Ok(build_update_response(&content))
}

async fn load_existing_submit_response(
    method_content_repository: &Arc<dyn MethodContentRepository>,
    tx: &mut crate::ports::UnitOfWorkTx,
    result_ref: ResultRef,
) -> Result<SubmitMethodContentForReviewResponse, MethodLibraryError> {
    let content = method_content_repository
        .get_for_update(tx, result_ref)
        .await?
        .ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::MethodContentNotFound,
                "idempotent submit review result could not be reloaded",
            )
        })?;

    Ok(build_submit_response(&content))
}

fn validate_create_draft_command(
    command: &CreateMethodContentDraftCommand,
) -> Result<(), MethodLibraryError> {
    if command.name.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "draft name must not be empty",
        ));
    }

    for reference in &command.references {
        validate_content_ref(reference)?;
    }

    for source_ref in &command.source_refs {
        validate_artifact_ref(source_ref)?;
    }

    match &command.payload {
        MethodContentPayload::Qualification(_)
        | MethodContentPayload::RoleDefinition(_)
        | MethodContentPayload::TaskDefinition(_)
        | MethodContentPayload::WorkProductDefinition(_)
        | MethodContentPayload::ProcessTemplateDef(_)
        | MethodContentPayload::ViewProfile(_)
        | MethodContentPayload::AIPolicyDef(_) => Ok(()),
    }
}

fn validate_content_ref(reference: &ContentRef) -> Result<(), MethodLibraryError> {
    if reference.target_content_id.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::ReferenceInvalid,
            "draft reference target content id must not be empty",
        ));
    }

    Ok(())
}

fn validate_update_draft_command(
    command: &UpdateMethodContentDraftCommand,
) -> Result<(), MethodLibraryError> {
    if command.content_id.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "draft content id must not be empty",
        ));
    }

    if command.name.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "draft name must not be empty",
        ));
    }

    for reference in &command.references {
        validate_content_ref(reference)?;
    }

    Ok(())
}

fn validate_submit_for_review_command(
    command: &SubmitMethodContentForReviewCommand,
) -> Result<(), MethodLibraryError> {
    if command.content_id.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "review content id must not be empty",
        ));
    }

    for reference in &command.review_evidence_refs {
        validate_artifact_ref(reference)?;
    }

    Ok(())
}

fn validate_artifact_ref(artifact_ref: &ArtifactRef) -> Result<(), MethodLibraryError> {
    if artifact_ref.artifact_id.trim().is_empty() || artifact_ref.artifact_kind.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::ReferenceInvalid,
            "source artifact references must not be empty",
        ));
    }

    Ok(())
}

fn merge_content_refs(mut base: Vec<ContentRef>, extras: Vec<ContentRef>) -> Vec<ContentRef> {
    let mut seen = HashSet::new();
    base.retain(|reference| seen.insert(reference.clone()));
    for reference in extras {
        if seen.insert(reference.clone()) {
            base.push(reference);
        }
    }

    base
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use time::macros::datetime;

    use super::MethodContentCommandService;
    use crate::ports::fakes::{
        DeterministicClock, DeterministicIdGenerator, FakeUnitOfWork, InMemoryAuditRepository,
        InMemoryIdempotencyRepository, InMemoryLifecycleHistoryRepository,
        InMemoryMethodContentReferenceRepository, InMemoryMethodContentRepository,
    };
    use crate::ports::{MethodContentRepository, UnitOfWork};
    use method_library_contracts::{
        ActorContext, ArtifactRef, CreateMethodContentDraftCommand, RequestMeta,
        SubmitMethodContentForReviewCommand, UpdateMethodContentDraftCommand,
    };
    use method_library_domain::MethodLibraryErrorCode;
    use method_library_domain::content::ActorKind;
    use method_library_domain::content::{
        ApprovedGateRef, CanonicalFingerprint, ContentRef, ContentVersion, FingerprintAlgorithm,
        LifecycleState, MethodContent, MethodContentKind, ReferenceState,
    };
    use method_library_domain::definitions::{
        EvidenceKind, EvidenceRule, MethodContentPayload, Qualification, QualificationLevel,
        QualificationLevelModel,
    };

    struct TestHarness {
        service: MethodContentCommandService,
        content_repository: Arc<InMemoryMethodContentRepository>,
        lifecycle_history_repository: Arc<InMemoryLifecycleHistoryRepository>,
    }

    fn sample_harness() -> TestHarness {
        let content_repository = Arc::new(InMemoryMethodContentRepository::default());
        let reference_repository = Arc::new(InMemoryMethodContentReferenceRepository::default());
        let lifecycle_history_repository = Arc::new(InMemoryLifecycleHistoryRepository::default());
        let audit_repository = Arc::new(InMemoryAuditRepository::default());
        let idempotency_repository = Arc::new(InMemoryIdempotencyRepository::default());
        let clock = Arc::new(DeterministicClock::new(datetime!(2026-05-26 08:00:00 UTC)));
        let id_generator = Arc::new(DeterministicIdGenerator::default());

        let service = MethodContentCommandService::new(
            Arc::new(FakeUnitOfWork),
            content_repository.clone(),
            reference_repository,
            lifecycle_history_repository.clone(),
            audit_repository,
            idempotency_repository,
            clock,
            id_generator,
        );

        TestHarness {
            service,
            content_repository,
            lifecycle_history_repository,
        }
    }

    fn sample_actor() -> ActorContext {
        ActorContext {
            actor_id: "actor-1".to_string(),
            actor_kind: ActorKind::Human,
            actor_ref: method_library_contracts::ActorRef {
                actor_id: "actor-1".to_string(),
                actor_kind: ActorKind::Human,
            },
        }
    }

    fn sample_meta() -> RequestMeta {
        sample_meta_with("idem-1", "hash-1")
    }

    fn sample_meta_with(idempotency_key: &str, request_hash: &str) -> RequestMeta {
        RequestMeta {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            idempotency_key: Some(idempotency_key.to_string()),
            request_hash: request_hash.to_string(),
            received_at: datetime!(2026-05-26 08:00:00 UTC),
        }
    }

    fn sample_command() -> CreateMethodContentDraftCommand {
        CreateMethodContentDraftCommand {
            kind: MethodContentKind::Qualification,
            name: "Quality".to_string(),
            description: None,
            payload: MethodContentPayload::Qualification(Qualification {
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
            references: vec![ContentRef {
                target_content_id: "content-ref-1".to_string(),
                target_kind: MethodContentKind::ViewProfile,
                required_state: ReferenceState::Published,
            }],
            source_refs: vec![ArtifactRef {
                artifact_id: "artifact-1".to_string(),
                artifact_kind: "evidence".to_string(),
            }],
        }
    }

    fn sample_update_command(
        content_id: impl Into<String>,
        expected_revision: i64,
    ) -> UpdateMethodContentDraftCommand {
        UpdateMethodContentDraftCommand {
            content_id: content_id.into(),
            expected_revision,
            name: "Quality Updated".to_string(),
            description: Some("Updated definition".to_string()),
            payload: MethodContentPayload::Qualification(Qualification {
                qualification_key: "quality-1".to_string(),
                name: "Quality Updated".to_string(),
                description: Some("Updated definition".to_string()),
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
                    description: "Updated proof".to_string(),
                }],
            }),
            references: vec![ContentRef {
                target_content_id: "content-ref-2".to_string(),
                target_kind: MethodContentKind::ViewProfile,
                required_state: ReferenceState::Published,
            }],
        }
    }

    fn sample_submit_command(
        content_id: impl Into<String>,
        expected_revision: i64,
    ) -> SubmitMethodContentForReviewCommand {
        SubmitMethodContentForReviewCommand {
            content_id: content_id.into(),
            expected_revision,
            review_reason: Some("Ready for review".to_string()),
            review_evidence_refs: vec![ArtifactRef {
                artifact_id: "artifact-2".to_string(),
                artifact_kind: "review-note".to_string(),
            }],
        }
    }

    async fn insert_published_content(
        repository: &Arc<InMemoryMethodContentRepository>,
        actor_id: &str,
    ) -> String {
        let mut content = MethodContent::create_draft(
            "content-published".to_string(),
            "family-published".to_string(),
            MethodContentKind::Qualification,
            "Published".to_string(),
            None,
            sample_command().payload,
            actor_id.to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("published fixture draft should build");
        content
            .submit_for_review(actor_id.to_string(), datetime!(2026-05-26 08:05:00 UTC))
            .expect("fixture should enter review");
        content
            .publish(
                ApprovedGateRef {
                    gate_id: "gate-1".to_string(),
                    gate_decision_id: "decision-1".to_string(),
                    approved_at: datetime!(2026-05-26 08:10:00 UTC),
                },
                ContentVersion::new("1.0.0").expect("fixture version should build"),
                CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "abc123", "1.0")
                    .expect("fixture fingerprint should build"),
                actor_id.to_string(),
                datetime!(2026-05-26 08:10:00 UTC),
            )
            .expect("fixture should publish");

        let mut tx = FakeUnitOfWork
            .begin(sample_meta_with("idem-seed", "hash-seed"))
            .await
            .expect("fixture transaction should open");
        repository
            .insert(&mut tx, content)
            .await
            .expect("fixture content should insert");
        tx.commit()
            .await
            .expect("fixture transaction should commit");

        "content-published".to_string()
    }

    #[tokio::test]
    async fn creates_draft_and_reuses_idempotent_result() {
        let harness = sample_harness();
        let actor = sample_actor();
        let meta = sample_meta();
        let command = sample_command();

        let response = harness
            .service
            .create_draft(command.clone(), actor.clone(), meta.clone())
            .await
            .expect("draft should be created");

        assert_eq!(response.content_id, "content-1");
        assert_eq!(response.lifecycle_state, LifecycleState::Draft);
        assert_eq!(response.revision, 1);

        let replayed = harness
            .service
            .create_draft(command, actor, meta)
            .await
            .expect("idempotent draft should reuse the response");

        assert_eq!(replayed, response);
    }

    #[tokio::test]
    async fn updates_draft_and_reuses_idempotent_result() {
        let harness = sample_harness();
        let actor = sample_actor();
        let create_response = harness
            .service
            .create_draft(sample_command(), actor.clone(), sample_meta())
            .await
            .expect("draft should be created");
        let update_meta = sample_meta_with("idem-2", "hash-2");
        let command = sample_update_command(create_response.content_id.clone(), 1);

        let response = harness
            .service
            .update_draft(command.clone(), actor.clone(), update_meta.clone())
            .await
            .expect("draft should be updated");

        assert_eq!(response.content_id, create_response.content_id);
        assert_eq!(response.lifecycle_state, LifecycleState::Draft);
        assert_eq!(response.revision, 2);

        let replayed = harness
            .service
            .update_draft(command, actor, update_meta)
            .await
            .expect("idempotent update should reuse the response");

        assert_eq!(replayed, response);

        let contents = harness
            .content_repository
            .contents()
            .expect("contents should be inspectable");
        let stored = contents
            .get(&response.content_id)
            .expect("updated content should be stored");
        assert_eq!(stored.name, "Quality Updated");
        assert_eq!(stored.references.len(), 1);
        assert_eq!(stored.references[0].target_content_id, "content-ref-2");
    }

    #[tokio::test]
    async fn submits_draft_for_review_and_reuses_idempotent_result() {
        let harness = sample_harness();
        let actor = sample_actor();
        let create_response = harness
            .service
            .create_draft(sample_command(), actor.clone(), sample_meta())
            .await
            .expect("draft should be created");
        let submit_meta = sample_meta_with("idem-3", "hash-3");
        let command = sample_submit_command(create_response.content_id.clone(), 1);

        let response = harness
            .service
            .submit_for_review(command.clone(), actor.clone(), submit_meta.clone())
            .await
            .expect("draft should enter review");

        assert_eq!(response.content_id, create_response.content_id);
        assert_eq!(response.lifecycle_state, LifecycleState::InReview);
        assert_eq!(response.revision, 2);

        let replayed = harness
            .service
            .submit_for_review(command, actor, submit_meta)
            .await
            .expect("idempotent submit should reuse the response");

        assert_eq!(replayed, response);

        let contents = harness
            .content_repository
            .contents()
            .expect("contents should be inspectable");
        let stored = contents
            .get(&response.content_id)
            .expect("submitted content should be stored");
        assert_eq!(stored.lifecycle.state, LifecycleState::InReview);

        let entries = harness
            .lifecycle_history_repository
            .entries()
            .expect("history should be inspectable");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].from_state, Some(LifecycleState::Draft));
        assert_eq!(entries[0].to_state, LifecycleState::InReview);
    }

    #[tokio::test]
    async fn rejects_updating_published_content() {
        let harness = sample_harness();
        let actor = sample_actor();
        let content_id =
            insert_published_content(&harness.content_repository, &actor.actor_id).await;

        let error = harness
            .service
            .update_draft(
                sample_update_command(content_id, 3),
                actor,
                sample_meta_with("idem-3", "hash-3"),
            )
            .await
            .expect_err("published content should reject draft updates");

        assert_eq!(
            error.code,
            MethodLibraryErrorCode::PublishedContentImmutable
        );
    }

    #[tokio::test]
    async fn rejects_repeated_submit_with_new_idempotency_key() {
        let harness = sample_harness();
        let actor = sample_actor();
        let create_response = harness
            .service
            .create_draft(sample_command(), actor.clone(), sample_meta())
            .await
            .expect("draft should be created");
        let first_submit = harness
            .service
            .submit_for_review(
                sample_submit_command(create_response.content_id.clone(), 1),
                actor.clone(),
                sample_meta_with("idem-3", "hash-3"),
            )
            .await
            .expect("draft should enter review");

        let error = harness
            .service
            .submit_for_review(
                sample_submit_command(first_submit.content_id.clone(), 2),
                actor,
                sample_meta_with("idem-4", "hash-4"),
            )
            .await
            .expect_err("review submission should reject in-review content");

        assert_eq!(
            error.code,
            MethodLibraryErrorCode::LifecycleTransitionNotAllowed
        );
    }
}
