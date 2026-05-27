//! Application services that orchestrate P0 method-content command flows.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use method_library_contracts::{
    ActorContext, ArtifactRef, ContentPublishedPayload, ContentRefView,
    CreateMethodContentDraftCommand, CreateMethodContentDraftResponse, DefinitionEventEnvelope,
    DefinitionEventPayload, DefinitionEventType, DefinitionSnapshot, EventTraceContext,
    MethodContentView, PublishMethodContentCommand, PublishMethodContentResponse, RequestMeta,
    SnapshotPayload, SnapshotRef, SubmitMethodContentForReviewCommand,
    SubmitMethodContentForReviewResponse, UpdateMethodContentDraftCommand,
    UpdateMethodContentDraftResponse,
};
use method_library_domain::content::{
    ApprovedGateRef, CanonicalFingerprint, ContentRef, ContentVersion, FingerprintAlgorithm,
    LifecycleState, MethodContent, PublishedContentRef, ReferenceState, SnapshotId,
};
use method_library_domain::definitions::MethodContentPayload;
use method_library_domain::policies::{
    DefinitionUseBoundaryGuard, FingerprintPolicy, PublishPolicy,
};
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ports::{
    AuditRecord, AuditRepository, AuditTargetRef, Clock, DefinitionSnapshotRepository,
    FailureReason, FingerprintHasher, GateValidationResult, GovernancePort, IdGenerator,
    IdempotencyBeginResult, IdempotencyRepository, IdempotencyScope, LifecycleHistoryEntry,
    LifecycleHistoryRepository, MethodContentReferenceRepository, MethodContentRepository,
    MethodContentVersionRecord, MethodContentVersionRepository, ObjectStoragePort, OutboxEvent,
    OutboxRepository, ResultRef, UnitOfWork,
};

const CREATE_DRAFT_SCOPE: &str = "command:create_draft";
const UPDATE_DRAFT_SCOPE: &str = "command:update_draft";
const SUBMIT_REVIEW_SCOPE: &str = "command:submit_for_review";
const PUBLISH_SCOPE: &str = "command:publish";
const CANONICAL_SCHEMA_VERSION: &str = "1.0";
const SNAPSHOT_SCHEMA_VERSION: &str = "1.0";
const EVENT_SCHEMA_VERSION: &str = "1.0";
const OUTBOX_PRODUCER: &str = "L3-method-library";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PublishResultRef {
    content_id: String,
    version: ContentVersion,
    snapshot_id: SnapshotId,
    outbox_event_id: String,
}

/// Application service for method-content command flows.
#[derive(Clone)]
pub struct MethodContentCommandService {
    unit_of_work: Arc<dyn UnitOfWork>,
    method_content_repository: Arc<dyn MethodContentRepository>,
    method_content_reference_repository: Arc<dyn MethodContentReferenceRepository>,
    method_content_version_repository: Arc<dyn MethodContentVersionRepository>,
    definition_snapshot_repository: Arc<dyn DefinitionSnapshotRepository>,
    outbox_repository: Arc<dyn OutboxRepository>,
    lifecycle_history_repository: Arc<dyn LifecycleHistoryRepository>,
    audit_repository: Arc<dyn AuditRepository>,
    idempotency_repository: Arc<dyn IdempotencyRepository>,
    governance_port: Arc<dyn GovernancePort>,
    object_storage_port: Arc<dyn ObjectStoragePort>,
    fingerprint_hasher: Arc<dyn FingerprintHasher>,
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
        method_content_version_repository: Arc<dyn MethodContentVersionRepository>,
        definition_snapshot_repository: Arc<dyn DefinitionSnapshotRepository>,
        outbox_repository: Arc<dyn OutboxRepository>,
        lifecycle_history_repository: Arc<dyn LifecycleHistoryRepository>,
        audit_repository: Arc<dyn AuditRepository>,
        idempotency_repository: Arc<dyn IdempotencyRepository>,
        governance_port: Arc<dyn GovernancePort>,
        object_storage_port: Arc<dyn ObjectStoragePort>,
        fingerprint_hasher: Arc<dyn FingerprintHasher>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Self {
        Self {
            unit_of_work,
            method_content_repository,
            method_content_reference_repository,
            method_content_version_repository,
            definition_snapshot_repository,
            outbox_repository,
            lifecycle_history_repository,
            audit_repository,
            idempotency_repository,
            governance_port,
            object_storage_port,
            fingerprint_hasher,
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

    /// Publishes an in-review method-content aggregate.
    pub async fn publish(
        &self,
        command: PublishMethodContentCommand,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<PublishMethodContentResponse, MethodLibraryError> {
        validate_publish_command(&command)?;
        let idempotency_key = meta.require_idempotency_key()?.clone();
        let mut tx = self.unit_of_work.begin(meta.clone()).await?;
        let now = self.clock.now();
        let scope: IdempotencyScope = PUBLISH_SCOPE.to_string();

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
                    let version_record_id = self.id_generator.new_content_version_record_id();
                    let snapshot_id = self.id_generator.new_snapshot_id();
                    let outbox_event_id = self.id_generator.new_outbox_event_id();
                    let PublishMethodContentCommand {
                        content_id,
                        expected_revision,
                        version,
                        approved_gate_ref,
                        publish_reason,
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
                    let gate_validation = self
                        .governance_port
                        .validate_approved_gate(
                            approved_gate_ref.clone(),
                            content.content_id.clone(),
                            actor.clone(),
                            meta.clone(),
                        )
                        .await?;

                    let resolved_refs =
                        resolve_published_refs(&self.method_content_repository, &mut tx, &content)
                            .await?;
                    PublishPolicy::validate_publish(&content, &approved_gate_ref, &resolved_refs)?;
                    let canonical_bytes = FingerprintPolicy::canonicalize(
                        &content,
                        CANONICAL_SCHEMA_VERSION.to_string(),
                    )?;
                    let fingerprint = self.fingerprint_hasher.hash_canonical_bytes(
                        canonical_bytes,
                        FingerprintAlgorithm::Sha256,
                        CANONICAL_SCHEMA_VERSION.to_string(),
                    )?;

                    content.publish(
                        approved_gate_ref.clone(),
                        version.clone(),
                        fingerprint.clone(),
                        actor.actor_id.clone(),
                        now,
                    )?;

                    let snapshot_payload = build_snapshot_payload(&content, &resolved_refs, now);
                    let blob_ref = self
                        .object_storage_port
                        .put_snapshot_payload(
                            snapshot_payload.clone(),
                            snapshot_object_key(&snapshot_id),
                            meta.clone(),
                        )
                        .await?;
                    let snapshot_ref = SnapshotRef {
                        snapshot_id: snapshot_id.clone(),
                        schema_version: SNAPSHOT_SCHEMA_VERSION.to_string(),
                        blob_ref: blob_ref.clone(),
                    };
                    let snapshot = DefinitionSnapshot {
                        snapshot_id: snapshot_id.clone(),
                        content_id: content.content_id.clone(),
                        version: version.clone(),
                        fingerprint: fingerprint.clone(),
                        schema_version: SNAPSHOT_SCHEMA_VERSION.to_string(),
                        blob_ref: blob_ref.clone(),
                        created_at: now,
                        content_ref: published_content_ref(&content)?,
                        references: resolved_refs.clone(),
                    };
                    let outbox_event = build_publish_outbox_event(
                        &content,
                        &approved_gate_ref,
                        &snapshot,
                        &fingerprint,
                        outbox_event_id.clone(),
                        meta.clone(),
                        now,
                    )?;
                    let response = build_publish_response(
                        &content,
                        snapshot_ref.clone(),
                        outbox_event_id.clone(),
                    )?;
                    let result_ref = serde_json::to_string(&PublishResultRef {
                        content_id: response.content_id.clone(),
                        version: response.version.clone(),
                        snapshot_id: snapshot.snapshot_id.clone(),
                        outbox_event_id: response.outbox_event_id.clone(),
                    })
                    .map_err(|error| {
                        MethodLibraryError::retryable(
                            MethodLibraryErrorCode::PersistenceUnavailable,
                            format!("failed to encode publish idempotency result: {error}"),
                        )
                    })?;

                    self.method_content_reference_repository
                        .replace_published_refs(
                            &mut tx,
                            response.content_id.clone(),
                            resolved_refs.clone(),
                        )
                        .await?;
                    let content_family_id = content.content_family_id.clone();
                    self.method_content_repository
                        .save(&mut tx, content, expected_revision)
                        .await?;
                    self.method_content_version_repository
                        .insert(
                            &mut tx,
                            MethodContentVersionRecord {
                                content_version_id: version_record_id,
                                content_id: response.content_id.clone(),
                                content_family_id,
                                version: response.version.clone(),
                                fingerprint: response.fingerprint.clone(),
                                snapshot_id: snapshot.snapshot_id.clone(),
                                published_at: now,
                            },
                        )
                        .await?;
                    self.definition_snapshot_repository
                        .insert(&mut tx, snapshot.clone())
                        .await?;
                    self.outbox_repository.append(&mut tx, outbox_event).await?;
                    self.lifecycle_history_repository
                        .append(
                            &mut tx,
                            build_lifecycle_history_entry(
                                history_entry_id,
                                &response.content_id,
                                LifecycleState::InReview,
                                response.lifecycle_state,
                                &actor.actor_id,
                                Some(publish_reason.clone()),
                                now,
                            ),
                        )
                        .await?;
                    self.audit_repository
                        .append(
                            &mut tx,
                            build_audit_record(
                                &response.content_id,
                                &actor,
                                &meta,
                                "publish",
                                "succeeded",
                                build_publish_details(
                                    &response,
                                    expected_revision,
                                    gate_validation,
                                    publish_reason,
                                    snapshot_ref.clone(),
                                    blob_ref,
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
                            result_ref,
                            now,
                        )
                        .await?;

                    Ok(response)
                }
                IdempotencyBeginResult::Succeeded(result_ref) => {
                    load_existing_publish_response(
                        &self.method_content_repository,
                        &self.definition_snapshot_repository,
                        &mut tx,
                        result_ref,
                    )
                    .await
                }
                IdempotencyBeginResult::Failed(reason) => Err(failure_reason_to_error(reason)),
                IdempotencyBeginResult::Processing => Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::IdempotencyStatusConflict,
                    "publish request is still processing",
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

fn build_publish_details(
    response: &PublishMethodContentResponse,
    expected_revision: i64,
    gate_validation: GateValidationResult,
    publish_reason: String,
    snapshot_ref: SnapshotRef,
    blob_ref: String,
) -> BTreeMap<String, String> {
    let GateValidationResult {
        approved: _,
        gate_ref,
        validated_at,
    } = gate_validation;
    let mut details = BTreeMap::new();
    details.insert("kind".to_string(), response.kind.as_str().to_string());
    details.insert(
        "expected_revision".to_string(),
        expected_revision.to_string(),
    );
    details.insert("revision".to_string(), response.revision.to_string());
    details.insert("version".to_string(), response.version.raw.clone());
    details.insert(
        "fingerprint".to_string(),
        response.fingerprint.value.clone(),
    );
    details.insert("gate_id".to_string(), gate_ref.gate_id);
    details.insert("gate_decision_id".to_string(), gate_ref.gate_decision_id);
    details.insert("gate_validated_at".to_string(), validated_at.to_string());
    details.insert("publish_reason".to_string(), publish_reason);
    details.insert("snapshot_id".to_string(), snapshot_ref.snapshot_id);
    details.insert("snapshot_blob_ref".to_string(), blob_ref);
    details.insert(
        "outbox_event_id".to_string(),
        response.outbox_event_id.clone(),
    );
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

fn build_publish_response(
    content: &MethodContent,
    snapshot_ref: SnapshotRef,
    outbox_event_id: String,
) -> Result<PublishMethodContentResponse, MethodLibraryError> {
    Ok(PublishMethodContentResponse {
        content_id: content.content_id.clone(),
        kind: content.kind,
        lifecycle_state: content.lifecycle.state,
        version: content.version.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "published content version is missing",
            )
        })?,
        fingerprint: content.fingerprint.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "published content fingerprint is missing",
            )
        })?,
        snapshot_ref,
        outbox_event_id,
        revision: content.revision,
    })
}

fn build_publish_outbox_event(
    content: &MethodContent,
    gate_ref: &ApprovedGateRef,
    snapshot: &DefinitionSnapshot,
    fingerprint: &CanonicalFingerprint,
    outbox_event_id: String,
    meta: RequestMeta,
    now: method_library_domain::content::Timestamp,
) -> Result<OutboxEvent, MethodLibraryError> {
    let content_ref = published_content_ref(content)?;
    let request_id = meta.request_id.clone();
    let trace_id = meta.trace_id.clone();
    let idempotency_key = meta.idempotency_key.clone();
    let payload = DefinitionEventPayload::ContentPublished(ContentPublishedPayload {
        gate_ref: gate_ref.clone(),
        version: content.version.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "published content version is missing",
            )
        })?,
        fingerprint: fingerprint.clone(),
    });
    let envelope = DefinitionEventEnvelope {
        event_id: outbox_event_id.clone(),
        event_type: DefinitionEventType::ContentPublished,
        schema_version: EVENT_SCHEMA_VERSION.to_string(),
        occurred_at: now,
        producer: OUTBOX_PRODUCER.to_string(),
        content_ref,
        snapshot_ref: Some(snapshot.snapshot_ref()),
        trace: EventTraceContext {
            request_id,
            trace_id,
        },
        payload,
    };
    let payload_hash = hash_canonical_json(&envelope)?;
    OutboxEvent::new_pending(
        outbox_event_id,
        content.content_id.clone(),
        envelope,
        payload_hash,
        idempotency_key,
    )
}

fn build_snapshot_payload(
    content: &MethodContent,
    references: &[PublishedContentRef],
    now: method_library_domain::content::Timestamp,
) -> SnapshotPayload {
    SnapshotPayload {
        content: build_method_content_view(content, references),
        references: references.to_vec(),
        generated_at: now,
        schema_version: SNAPSHOT_SCHEMA_VERSION.to_string(),
    }
}

fn build_method_content_view(
    content: &MethodContent,
    references: &[PublishedContentRef],
) -> MethodContentView {
    MethodContentView {
        content_id: content.content_id.clone(),
        content_family_id: content.content_family_id.clone(),
        kind: content.kind,
        name: content.name.clone(),
        description: content.description.clone(),
        lifecycle_state: content.lifecycle.state,
        version: content.version.clone(),
        fingerprint: content.fingerprint.clone(),
        payload: Some(content.payload.clone()),
        references: references
            .iter()
            .map(build_content_ref_view)
            .collect::<Vec<_>>(),
        revision: content.revision,
    }
}

fn build_content_ref_view(reference: &PublishedContentRef) -> ContentRefView {
    ContentRefView {
        target_content_id: reference.content_id.clone(),
        target_kind: reference.kind,
        target_version: Some(reference.version.clone()),
        target_fingerprint: Some(reference.fingerprint.clone()),
    }
}

fn published_content_ref(
    content: &MethodContent,
) -> Result<PublishedContentRef, MethodLibraryError> {
    Ok(PublishedContentRef {
        content_id: content.content_id.clone(),
        kind: content.kind,
        version: content.version.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "published content version is missing",
            )
        })?,
        fingerprint: content.fingerprint.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "published content fingerprint is missing",
            )
        })?,
    })
}

fn snapshot_object_key(snapshot_id: &str) -> String {
    format!("method-library/snapshots/{snapshot_id}.json")
}

fn hash_canonical_json<T: Serialize>(value: &T) -> Result<String, MethodLibraryError> {
    let canonical = serde_json::to_string(value).map_err(|error| {
        MethodLibraryError::validation(
            MethodLibraryErrorCode::FingerprintBuildFailed,
            format!("failed to canonicalize publish payload: {error}"),
        )
    })?;

    Ok(format!("sha256:{:x}", Sha256::digest(canonical.as_bytes())))
}

async fn resolve_published_refs(
    method_content_repository: &Arc<dyn MethodContentRepository>,
    tx: &mut crate::ports::UnitOfWorkTx,
    content: &MethodContent,
) -> Result<Vec<PublishedContentRef>, MethodLibraryError> {
    let mut resolved_refs = Vec::with_capacity(content.references.len());

    for reference in &content.references {
        if !reference.target_kind.is_definition_kind() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::ReferenceInvalid,
                "reference target kind is outside the definition boundary",
            )
            .with_detail("target_kind", reference.target_kind.as_str()));
        }

        let target = method_content_repository
            .get_for_update(tx, reference.target_content_id.clone())
            .await?
            .ok_or_else(|| {
                MethodLibraryError::validation(
                    MethodLibraryErrorCode::ReferenceNotPublished,
                    "reference target is not available as a published definition",
                )
                .with_detail("target_content_id", reference.target_content_id.clone())
                .with_detail("target_kind", reference.target_kind.as_str())
            })?;

        if target.kind != reference.target_kind {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::ReferenceNotPublished,
                "reference target does not match the required definition kind",
            )
            .with_detail("target_content_id", reference.target_content_id.clone())
            .with_detail("target_kind", reference.target_kind.as_str()));
        }

        let lifecycle_ok = match reference.required_state {
            ReferenceState::Published => target.lifecycle.state == LifecycleState::Published,
            ReferenceState::PublishedLike => target.is_published_like(),
        };
        if !lifecycle_ok {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::ReferenceNotPublished,
                "reference target is not available as a published definition",
            )
            .with_detail("target_content_id", reference.target_content_id.clone())
            .with_detail("target_kind", reference.target_kind.as_str()));
        }

        let version = target.version.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::ReferenceNotPublished,
                "published reference target is missing a version",
            )
            .with_detail("target_content_id", reference.target_content_id.clone())
        })?;
        let fingerprint = target.fingerprint.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::ReferenceNotPublished,
                "published reference target is missing a fingerprint",
            )
            .with_detail("target_content_id", reference.target_content_id.clone())
        })?;

        resolved_refs.push(PublishedContentRef {
            content_id: target.content_id.clone(),
            kind: target.kind,
            version,
            fingerprint,
        });
    }

    Ok(resolved_refs)
}

async fn load_existing_publish_response(
    method_content_repository: &Arc<dyn MethodContentRepository>,
    snapshot_repository: &Arc<dyn DefinitionSnapshotRepository>,
    tx: &mut crate::ports::UnitOfWorkTx,
    result_ref: ResultRef,
) -> Result<PublishMethodContentResponse, MethodLibraryError> {
    let publish_result: PublishResultRef = serde_json::from_str(&result_ref).map_err(|error| {
        MethodLibraryError::validation(
            MethodLibraryErrorCode::PersistenceUnavailable,
            format!("publish idempotency result could not be decoded: {error}"),
        )
    })?;

    let content = method_content_repository
        .get_for_update(tx, publish_result.content_id.clone())
        .await?
        .ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::MethodContentNotFound,
                "idempotent publish result could not be reloaded",
            )
        })?;
    let snapshot = snapshot_repository
        .get(publish_result.snapshot_id.clone())
        .await?
        .ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::SnapshotNotFound,
                "idempotent publish snapshot could not be reloaded",
            )
        })?;

    if content.version.as_ref() != Some(&publish_result.version) {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::MethodContentNotFound,
            "published content version did not match the idempotent publish result",
        ));
    }

    if snapshot.content_id != content.content_id || snapshot.version != publish_result.version {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::SnapshotNotFound,
            "published snapshot did not match the idempotent publish result",
        ));
    }

    build_publish_response(
        &content,
        snapshot.snapshot_ref(),
        publish_result.outbox_event_id,
    )
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

fn validate_publish_command(
    command: &PublishMethodContentCommand,
) -> Result<(), MethodLibraryError> {
    if command.content_id.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "publish content id must not be empty",
        ));
    }

    if command.publish_reason.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "publish reason must not be empty",
        ));
    }

    if !command.approved_gate_ref.is_complete() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::PublishGateRequired,
            "publish requires a complete approved gate reference",
        ));
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
        DeterministicClock, DeterministicFingerprintHasher, DeterministicIdGenerator,
        FakeUnitOfWork, InMemoryAuditRepository, InMemoryDefinitionSnapshotRepository,
        InMemoryIdempotencyRepository, InMemoryLifecycleHistoryRepository,
        InMemoryMethodContentReferenceRepository, InMemoryMethodContentRepository,
        InMemoryMethodContentVersionRepository, InMemoryObjectStorage, InMemoryOutboxRepository,
        StaticGovernancePort,
    };
    use crate::ports::{MethodContentRepository, UnitOfWork};
    use method_library_contracts::{
        ActorContext, ArtifactRef, CreateMethodContentDraftCommand, DefinitionEventType,
        PublishMethodContentCommand, RequestMeta, SubmitMethodContentForReviewCommand,
        UpdateMethodContentDraftCommand,
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
        version_repository: Arc<InMemoryMethodContentVersionRepository>,
        snapshot_repository: Arc<InMemoryDefinitionSnapshotRepository>,
        outbox_repository: Arc<InMemoryOutboxRepository>,
        object_storage: Arc<InMemoryObjectStorage>,
        lifecycle_history_repository: Arc<InMemoryLifecycleHistoryRepository>,
    }

    fn sample_harness() -> TestHarness {
        let content_repository = Arc::new(InMemoryMethodContentRepository::default());
        let reference_repository = Arc::new(InMemoryMethodContentReferenceRepository::default());
        let version_repository = Arc::new(InMemoryMethodContentVersionRepository::default());
        let snapshot_repository = Arc::new(InMemoryDefinitionSnapshotRepository::default());
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
        let clock = Arc::new(DeterministicClock::new(datetime!(2026-05-26 08:00:00 UTC)));
        let id_generator = Arc::new(DeterministicIdGenerator::default());

        let service = MethodContentCommandService::new(
            Arc::new(FakeUnitOfWork),
            content_repository.clone(),
            reference_repository,
            version_repository.clone(),
            snapshot_repository.clone(),
            outbox_repository.clone(),
            lifecycle_history_repository.clone(),
            audit_repository,
            idempotency_repository,
            governance_port,
            object_storage.clone(),
            fingerprint_hasher,
            clock,
            id_generator,
        );

        TestHarness {
            service,
            content_repository,
            version_repository,
            snapshot_repository,
            outbox_repository,
            object_storage,
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
                target_kind: MethodContentKind::Qualification,
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
                target_kind: MethodContentKind::Qualification,
                required_state: ReferenceState::Published,
            }],
        }
    }

    fn sample_publish_command(
        content_id: impl Into<String>,
        expected_revision: i64,
        gate_id: &str,
        gate_decision_id: &str,
    ) -> PublishMethodContentCommand {
        PublishMethodContentCommand {
            content_id: content_id.into(),
            expected_revision,
            version: ContentVersion::new("1.0.0").expect("version should be valid"),
            approved_gate_ref: ApprovedGateRef {
                gate_id: gate_id.to_string(),
                gate_decision_id: gate_decision_id.to_string(),
                approved_at: datetime!(2026-05-26 08:10:00 UTC),
            },
            publish_reason: "Initial release".to_string(),
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
        content_id: &str,
        actor_id: &str,
    ) -> String {
        let mut content = MethodContent::create_draft(
            content_id.to_string(),
            format!("family-{content_id}"),
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

        content_id.to_string()
    }

    async fn insert_draft_content(
        repository: &Arc<InMemoryMethodContentRepository>,
        content_id: &str,
        actor_id: &str,
    ) -> String {
        let content = MethodContent::create_draft(
            content_id.to_string(),
            format!("family-{content_id}"),
            MethodContentKind::Qualification,
            "Draft reference".to_string(),
            None,
            sample_command().payload,
            actor_id.to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("draft fixture should build");

        let mut tx = FakeUnitOfWork
            .begin(sample_meta_with("idem-seed-draft", "hash-seed-draft"))
            .await
            .expect("fixture transaction should open");
        repository
            .insert(&mut tx, content)
            .await
            .expect("draft fixture content should insert");
        tx.commit()
            .await
            .expect("fixture transaction should commit");

        content_id.to_string()
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
    async fn publishes_draft_and_reuses_idempotent_result() {
        let harness = sample_harness();
        let actor = sample_actor();
        let target_content_id = insert_published_content(
            &harness.content_repository,
            "content-ref-1",
            &actor.actor_id,
        )
        .await;
        assert_eq!(target_content_id, "content-ref-1");

        let create_response = harness
            .service
            .create_draft(sample_command(), actor.clone(), sample_meta())
            .await
            .expect("draft should be created");
        let submit_meta = sample_meta_with("idem-3", "hash-3");
        let submit_response = harness
            .service
            .submit_for_review(
                sample_submit_command(create_response.content_id.clone(), 1),
                actor.clone(),
                submit_meta,
            )
            .await
            .expect("draft should enter review");

        let publish_meta = sample_meta_with("idem-4", "hash-4");
        let command = sample_publish_command(
            submit_response.content_id.clone(),
            submit_response.revision,
            "gate-1",
            "decision-1",
        );
        let response = harness
            .service
            .publish(command.clone(), actor.clone(), publish_meta.clone())
            .await
            .expect("content should publish");

        assert_eq!(response.content_id, submit_response.content_id);
        assert_eq!(response.lifecycle_state, LifecycleState::Published);
        assert_eq!(response.version.raw, "1.0.0");
        assert_eq!(response.snapshot_ref.schema_version, "1.0");

        let replayed = harness
            .service
            .publish(command, actor, publish_meta)
            .await
            .expect("idempotent publish should reuse the response");
        assert_eq!(replayed, response);

        let contents = harness
            .content_repository
            .contents()
            .expect("contents should be inspectable");
        let stored = contents
            .get(&response.content_id)
            .expect("published content should be stored");
        assert_eq!(stored.lifecycle.state, LifecycleState::Published);
        assert!(stored.version.is_some());
        assert!(stored.fingerprint.is_some());

        let versions = harness
            .version_repository
            .records()
            .expect("version history should be inspectable");
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].content_id, response.content_id);

        let snapshots = harness
            .snapshot_repository
            .snapshots()
            .expect("snapshots should be inspectable");
        assert_eq!(snapshots.len(), 1);
        let snapshot = snapshots
            .get(&response.snapshot_ref.snapshot_id)
            .expect("snapshot should be stored");
        assert_eq!(snapshot.content_id, response.content_id);
        assert_eq!(snapshot.version, response.version);

        let blobs = harness
            .object_storage
            .blobs()
            .expect("blobs should be inspectable");
        assert!(blobs.contains_key(&response.snapshot_ref.blob_ref));

        let events = harness
            .outbox_repository
            .events()
            .expect("outbox should be inspectable");
        assert_eq!(events.len(), 1);
        let event = events
            .get(&response.outbox_event_id)
            .expect("publish event should exist");
        assert_eq!(
            event.envelope.event_type,
            DefinitionEventType::ContentPublished
        );

        let entries = harness
            .lifecycle_history_repository
            .entries()
            .expect("history should be inspectable");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].from_state, Some(LifecycleState::InReview));
        assert_eq!(entries[1].to_state, LifecycleState::Published);
    }

    #[tokio::test]
    async fn rejects_publish_without_gate() {
        let harness = sample_harness();
        let actor = sample_actor();
        let create_response = harness
            .service
            .create_draft(sample_command(), actor.clone(), sample_meta())
            .await
            .expect("draft should be created");
        harness
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
            .publish(
                sample_publish_command(create_response.content_id.clone(), 2, "", ""),
                actor,
                sample_meta_with("idem-4", "hash-4"),
            )
            .await
            .expect_err("publish without gate should fail");

        assert_eq!(error.code, MethodLibraryErrorCode::PublishGateRequired);
        let versions = harness
            .version_repository
            .records()
            .expect("version history should be inspectable");
        assert!(versions.is_empty());
        let snapshots = harness
            .snapshot_repository
            .snapshots()
            .expect("snapshots should be inspectable");
        assert!(snapshots.is_empty());
        let events = harness
            .outbox_repository
            .events()
            .expect("outbox should be inspectable");
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn rejects_publish_when_reference_target_is_not_published() {
        let harness = sample_harness();
        let actor = sample_actor();
        let target_content_id = insert_draft_content(
            &harness.content_repository,
            "content-ref-1",
            &actor.actor_id,
        )
        .await;
        assert_eq!(target_content_id, "content-ref-1");

        let create_response = harness
            .service
            .create_draft(sample_command(), actor.clone(), sample_meta())
            .await
            .expect("draft should be created");
        harness
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
            .publish(
                sample_publish_command(
                    create_response.content_id.clone(),
                    2,
                    "gate-1",
                    "decision-1",
                ),
                actor,
                sample_meta_with("idem-4", "hash-4"),
            )
            .await
            .expect_err("publish should reject unpublished references");

        assert_eq!(error.code, MethodLibraryErrorCode::ReferenceNotPublished);
        let contents = harness
            .content_repository
            .contents()
            .expect("contents should be inspectable");
        let stored = contents
            .get(&create_response.content_id)
            .expect("draft should still exist");
        assert_eq!(stored.lifecycle.state, LifecycleState::InReview);
        let versions = harness
            .version_repository
            .records()
            .expect("version history should be inspectable");
        assert!(versions.is_empty());
        let snapshots = harness
            .snapshot_repository
            .snapshots()
            .expect("snapshots should be inspectable");
        assert!(snapshots.is_empty());
        let events = harness
            .outbox_repository
            .events()
            .expect("outbox should be inspectable");
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn rejects_updating_published_content() {
        let harness = sample_harness();
        let actor = sample_actor();
        let content_id = insert_published_content(
            &harness.content_repository,
            "content-published",
            &actor.actor_id,
        )
        .await;

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
