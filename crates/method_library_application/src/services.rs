//! Application services that orchestrate P0 method-content command flows.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use method_library_contracts::{
    ActorContext, ArtifactRef, ContentDeprecatedPayload, ContentPublishedPayload, ContentRefView,
    ContentRetiredPayload, CreateMethodContentDraftCommand, CreateMethodContentDraftResponse,
    DefinitionEventEnvelope, DefinitionEventPayload, DefinitionSnapshot,
    DeprecateMethodContentCommand, DeprecateMethodContentResponse, EventTraceContext,
    FingerprintChangedPayload, MethodContentView, PublishMethodContentCommand,
    PublishMethodContentResponse, RequestMeta, RetireMethodContentCommand,
    RetireMethodContentResponse, SnapshotPayload, SnapshotRef, SubmitMethodContentForReviewCommand,
    SubmitMethodContentForReviewResponse, SupersedeMethodContentCommand,
    SupersedeMethodContentResponse, UpdateMethodContentDraftCommand,
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
    OutboxRepository, ResultRef, SupersedeLink, SupersedeLinkRepository, UnitOfWork,
};

const CREATE_DRAFT_SCOPE: &str = "command:create_draft";
const UPDATE_DRAFT_SCOPE: &str = "command:update_draft";
const SUBMIT_REVIEW_SCOPE: &str = "command:submit_for_review";
const PUBLISH_SCOPE: &str = "command:publish";
const DEPRECATE_SCOPE: &str = "command:deprecate";
const RETIRE_SCOPE: &str = "command:retire";
const SUPERSEDE_SCOPE: &str = "command:supersede";
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LifecycleCommandResultRef {
    content_id: String,
    outbox_event_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SupersedeResultRef {
    old_content_id: String,
    new_content_id: String,
    snapshot_id: SnapshotId,
    supersede_link_id: String,
    outbox_event_ids: Vec<String>,
}

/// Application service for method-content command flows.
#[derive(Clone)]
pub struct MethodContentCommandService {
    unit_of_work: Arc<dyn UnitOfWork>,
    method_content_repository: Arc<dyn MethodContentRepository>,
    method_content_reference_repository: Arc<dyn MethodContentReferenceRepository>,
    method_content_version_repository: Arc<dyn MethodContentVersionRepository>,
    definition_snapshot_repository: Arc<dyn DefinitionSnapshotRepository>,
    supersede_link_repository: Arc<dyn SupersedeLinkRepository>,
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
        supersede_link_repository: Arc<dyn SupersedeLinkRepository>,
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
            supersede_link_repository,
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

    /// Deprecates a published method-content aggregate.
    pub async fn deprecate(
        &self,
        command: DeprecateMethodContentCommand,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<DeprecateMethodContentResponse, MethodLibraryError> {
        validate_deprecate_command(&command)?;
        let idempotency_key = meta.require_idempotency_key()?.clone();
        let mut tx = self.unit_of_work.begin(meta.clone()).await?;
        let now = self.clock.now();
        let scope: IdempotencyScope = DEPRECATE_SCOPE.to_string();

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
                    let outbox_event_id = self.id_generator.new_outbox_event_id();
                    let DeprecateMethodContentCommand {
                        content_id,
                        expected_revision,
                        reason,
                        effective_at,
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
                    let effective_at = effective_at.unwrap_or(now);
                    content.deprecate(reason.clone(), actor.actor_id.clone(), now)?;

                    let response = build_deprecate_response(&content, outbox_event_id.clone())?;
                    let outbox_event = build_deprecate_outbox_event(
                        &content,
                        reason.clone(),
                        effective_at,
                        outbox_event_id.clone(),
                        meta.clone(),
                        now,
                    )?;
                    let result_ref = encode_json_result_ref(
                        &LifecycleCommandResultRef {
                            content_id: response.content_id.clone(),
                            outbox_event_id: response.outbox_event_id.clone(),
                        },
                        "deprecate idempotency result",
                    )?;

                    self.method_content_repository
                        .save(&mut tx, content, expected_revision)
                        .await?;
                    self.outbox_repository.append(&mut tx, outbox_event).await?;
                    self.lifecycle_history_repository
                        .append(
                            &mut tx,
                            build_lifecycle_history_entry(
                                history_entry_id,
                                &response.content_id,
                                from_state,
                                response.lifecycle_state,
                                &actor.actor_id,
                                Some(reason.clone()),
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
                                "deprecate",
                                "succeeded",
                                build_deprecate_details(
                                    &response,
                                    expected_revision,
                                    reason,
                                    effective_at,
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
                    load_existing_deprecate_response(
                        &self.method_content_repository,
                        &mut tx,
                        result_ref,
                    )
                    .await
                }
                IdempotencyBeginResult::Failed(reason) => Err(failure_reason_to_error(reason)),
                IdempotencyBeginResult::Processing => Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::IdempotencyStatusConflict,
                    "deprecate request is still processing",
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

    /// Retires a published-like method-content aggregate.
    pub async fn retire(
        &self,
        command: RetireMethodContentCommand,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<RetireMethodContentResponse, MethodLibraryError> {
        validate_retire_command(&command)?;
        let idempotency_key = meta.require_idempotency_key()?.clone();
        let mut tx = self.unit_of_work.begin(meta.clone()).await?;
        let now = self.clock.now();
        let scope: IdempotencyScope = RETIRE_SCOPE.to_string();

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
                    let outbox_event_id = self.id_generator.new_outbox_event_id();
                    let RetireMethodContentCommand {
                        content_id,
                        expected_revision,
                        reason,
                        retire_policy,
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
                    content.retire(reason.clone(), actor.actor_id.clone(), now)?;

                    let response = build_retire_response(&content, outbox_event_id.clone())?;
                    let outbox_event = build_retire_outbox_event(
                        &content,
                        reason.clone(),
                        retire_policy.clone(),
                        outbox_event_id.clone(),
                        meta.clone(),
                        now,
                    )?;
                    let result_ref = encode_json_result_ref(
                        &LifecycleCommandResultRef {
                            content_id: response.content_id.clone(),
                            outbox_event_id: response.outbox_event_id.clone(),
                        },
                        "retire idempotency result",
                    )?;

                    self.method_content_repository
                        .save(&mut tx, content, expected_revision)
                        .await?;
                    self.outbox_repository.append(&mut tx, outbox_event).await?;
                    self.lifecycle_history_repository
                        .append(
                            &mut tx,
                            build_lifecycle_history_entry(
                                history_entry_id,
                                &response.content_id,
                                from_state,
                                response.lifecycle_state,
                                &actor.actor_id,
                                Some(reason.clone()),
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
                                "retire",
                                "succeeded",
                                build_retire_details(
                                    &response,
                                    expected_revision,
                                    reason,
                                    retire_policy,
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
                    load_existing_retire_response(
                        &self.method_content_repository,
                        &mut tx,
                        result_ref,
                    )
                    .await
                }
                IdempotencyBeginResult::Failed(reason) => Err(failure_reason_to_error(reason)),
                IdempotencyBeginResult::Processing => Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::IdempotencyStatusConflict,
                    "retire request is still processing",
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

    /// Supersedes one published-like definition with a newly published replacement.
    pub async fn supersede(
        &self,
        command: SupersedeMethodContentCommand,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<SupersedeMethodContentResponse, MethodLibraryError> {
        validate_supersede_command(&command)?;
        let idempotency_key = meta.require_idempotency_key()?.clone();
        let mut tx = self.unit_of_work.begin(meta.clone()).await?;
        let now = self.clock.now();
        let scope: IdempotencyScope = SUPERSEDE_SCOPE.to_string();

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
                    let old_history_entry_id = self.id_generator.new_history_entry_id();
                    let new_history_entry_id = self.id_generator.new_history_entry_id();
                    let version_record_id = self.id_generator.new_content_version_record_id();
                    let snapshot_id = self.id_generator.new_snapshot_id();
                    let supersede_link_id = self.id_generator.new_supersede_link_id();
                    let published_event_id = self.id_generator.new_outbox_event_id();
                    let fingerprint_event_id = self.id_generator.new_outbox_event_id();
                    let SupersedeMethodContentCommand {
                        old_content_id,
                        old_expected_revision,
                        new_content_id,
                        new_expected_revision,
                        new_version,
                        approved_gate_ref,
                        reason,
                    } = command;

                    if old_content_id == new_content_id {
                        return Err(MethodLibraryError::validation(
                            MethodLibraryErrorCode::SupersedeConflict,
                            "supersede requires distinct old and new content ids",
                        ));
                    }

                    let mut old_content = self
                        .method_content_repository
                        .get_for_update(&mut tx, old_content_id.clone())
                        .await?
                        .ok_or_else(|| {
                            MethodLibraryError::validation(
                                MethodLibraryErrorCode::MethodContentNotFound,
                                "old method content does not exist",
                            )
                        })?;
                    let mut new_content = self
                        .method_content_repository
                        .get_for_update(&mut tx, new_content_id.clone())
                        .await?
                        .ok_or_else(|| {
                            MethodLibraryError::validation(
                                MethodLibraryErrorCode::MethodContentNotFound,
                                "replacement method content does not exist",
                            )
                        })?;

                    old_content.ensure_revision(old_expected_revision)?;
                    new_content.ensure_revision(new_expected_revision)?;
                    if old_content.kind != new_content.kind {
                        return Err(MethodLibraryError::validation(
                            MethodLibraryErrorCode::SupersedeKindMismatch,
                            "supersede requires the replacement definition kind to match",
                        )
                        .with_detail("old_kind", old_content.kind.as_str())
                        .with_detail("new_kind", new_content.kind.as_str()));
                    }

                    let gate_validation = self
                        .governance_port
                        .validate_approved_gate(
                            approved_gate_ref.clone(),
                            new_content.content_id.clone(),
                            actor.clone(),
                            meta.clone(),
                        )
                        .await?;

                    let old_state = old_content.lifecycle.state;
                    let old_version = old_content.version.clone().ok_or_else(|| {
                        MethodLibraryError::validation(
                            MethodLibraryErrorCode::PublishedContentImmutable,
                            "superseded content version is missing",
                        )
                    })?;
                    let old_fingerprint = old_content.fingerprint.clone().ok_or_else(|| {
                        MethodLibraryError::validation(
                            MethodLibraryErrorCode::PublishedContentImmutable,
                            "superseded content fingerprint is missing",
                        )
                    })?;

                    new_content.content_family_id = old_content.content_family_id.clone();
                    new_content.supersedes_content_id = Some(old_content.content_id.clone());
                    let resolved_refs = resolve_published_refs(
                        &self.method_content_repository,
                        &mut tx,
                        &new_content,
                    )
                    .await?;
                    PublishPolicy::validate_publish(
                        &new_content,
                        &approved_gate_ref,
                        &resolved_refs,
                    )?;
                    let canonical_bytes = FingerprintPolicy::canonicalize(
                        &new_content,
                        CANONICAL_SCHEMA_VERSION.to_string(),
                    )?;
                    let new_fingerprint = self.fingerprint_hasher.hash_canonical_bytes(
                        canonical_bytes,
                        FingerprintAlgorithm::Sha256,
                        CANONICAL_SCHEMA_VERSION.to_string(),
                    )?;

                    old_content.mark_superseded_by(
                        new_content.content_id.clone(),
                        reason.clone(),
                        actor.actor_id.clone(),
                        now,
                    )?;
                    new_content.publish(
                        approved_gate_ref.clone(),
                        new_version.clone(),
                        new_fingerprint.clone(),
                        actor.actor_id.clone(),
                        now,
                    )?;

                    let snapshot_payload =
                        build_snapshot_payload(&new_content, &resolved_refs, now);
                    let blob_ref = self
                        .object_storage_port
                        .put_snapshot_payload(
                            snapshot_payload,
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
                        content_id: new_content.content_id.clone(),
                        version: new_version.clone(),
                        fingerprint: new_fingerprint.clone(),
                        schema_version: SNAPSHOT_SCHEMA_VERSION.to_string(),
                        blob_ref: blob_ref.clone(),
                        created_at: now,
                        content_ref: published_content_ref(&new_content)?,
                        references: resolved_refs.clone(),
                    };

                    let mut published_event = build_publish_outbox_event(
                        &new_content,
                        &approved_gate_ref,
                        &snapshot,
                        &new_fingerprint,
                        published_event_id.clone(),
                        meta.clone(),
                        now,
                    )?;
                    let mut outbox_events = Vec::new();
                    let mut outbox_event_ids = Vec::new();
                    if old_fingerprint != new_fingerprint {
                        published_event.idempotency_key = None;
                        let fingerprint_event = build_fingerprint_changed_outbox_event(
                            &new_content,
                            &old_fingerprint,
                            &new_fingerprint,
                            &reason,
                            &snapshot,
                            fingerprint_event_id.clone(),
                            meta.clone(),
                            now,
                        )?;
                        outbox_event_ids.push(fingerprint_event_id.clone());
                        outbox_events.push(fingerprint_event);
                    }
                    outbox_event_ids.push(published_event_id.clone());
                    outbox_events.push(published_event);

                    let response = build_supersede_response(
                        &old_content,
                        &new_content,
                        snapshot_ref.clone(),
                        supersede_link_id.clone(),
                        outbox_event_ids.clone(),
                    )?;
                    let result_ref = encode_json_result_ref(
                        &SupersedeResultRef {
                            old_content_id: response.old_content_id.clone(),
                            new_content_id: response.new_content_id.clone(),
                            snapshot_id: snapshot.snapshot_id.clone(),
                            supersede_link_id: response.supersede_link_id.clone(),
                            outbox_event_ids: response.outbox_event_ids.clone(),
                        },
                        "supersede idempotency result",
                    )?;

                    self.method_content_reference_repository
                        .replace_published_refs(
                            &mut tx,
                            response.new_content_id.clone(),
                            resolved_refs,
                        )
                        .await?;
                    self.method_content_repository
                        .save(&mut tx, old_content, old_expected_revision)
                        .await?;
                    let content_family_id = new_content.content_family_id.clone();
                    self.method_content_repository
                        .save(&mut tx, new_content, new_expected_revision)
                        .await?;
                    self.method_content_version_repository
                        .insert(
                            &mut tx,
                            MethodContentVersionRecord {
                                content_version_id: version_record_id,
                                content_id: response.new_content_id.clone(),
                                content_family_id: content_family_id.clone(),
                                version: response.new_version.clone(),
                                fingerprint: response.new_fingerprint.clone(),
                                snapshot_id: snapshot.snapshot_id.clone(),
                                published_at: now,
                            },
                        )
                        .await?;
                    self.definition_snapshot_repository
                        .insert(&mut tx, snapshot.clone())
                        .await?;
                    self.supersede_link_repository
                        .insert(
                            &mut tx,
                            SupersedeLink {
                                supersede_link_id: supersede_link_id.clone(),
                                old_content_id: response.old_content_id.clone(),
                                new_content_id: response.new_content_id.clone(),
                                content_family_id,
                                reason: reason.clone(),
                                created_at: now,
                            },
                        )
                        .await?;
                    for outbox_event in outbox_events {
                        self.outbox_repository.append(&mut tx, outbox_event).await?;
                    }
                    self.lifecycle_history_repository
                        .append(
                            &mut tx,
                            build_lifecycle_history_entry(
                                old_history_entry_id,
                                &response.old_content_id,
                                old_state,
                                response.old_lifecycle_state,
                                &actor.actor_id,
                                Some(reason.clone()),
                                now,
                            ),
                        )
                        .await?;
                    self.lifecycle_history_repository
                        .append(
                            &mut tx,
                            build_lifecycle_history_entry(
                                new_history_entry_id,
                                &response.new_content_id,
                                LifecycleState::InReview,
                                response.new_lifecycle_state,
                                &actor.actor_id,
                                Some(reason.clone()),
                                now,
                            ),
                        )
                        .await?;
                    self.audit_repository
                        .append(
                            &mut tx,
                            build_audit_record(
                                &response.old_content_id,
                                &actor,
                                &meta,
                                "supersede",
                                "succeeded",
                                build_supersede_details(
                                    &response,
                                    old_version,
                                    old_fingerprint,
                                    gate_validation,
                                    reason,
                                    snapshot_ref,
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
                    load_existing_supersede_response(
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
                    "supersede request is still processing",
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

fn build_deprecate_response(
    content: &MethodContent,
    outbox_event_id: String,
) -> Result<DeprecateMethodContentResponse, MethodLibraryError> {
    Ok(DeprecateMethodContentResponse {
        content_id: content.content_id.clone(),
        kind: content.kind,
        lifecycle_state: content.lifecycle.state,
        version: content.version.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "deprecated content version is missing",
            )
        })?,
        fingerprint: content.fingerprint.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "deprecated content fingerprint is missing",
            )
        })?,
        outbox_event_id,
        revision: content.revision,
    })
}

fn build_retire_response(
    content: &MethodContent,
    outbox_event_id: String,
) -> Result<RetireMethodContentResponse, MethodLibraryError> {
    Ok(RetireMethodContentResponse {
        content_id: content.content_id.clone(),
        kind: content.kind,
        lifecycle_state: content.lifecycle.state,
        version: content.version.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "retired content version is missing",
            )
        })?,
        fingerprint: content.fingerprint.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "retired content fingerprint is missing",
            )
        })?,
        outbox_event_id,
        revision: content.revision,
    })
}

fn build_supersede_response(
    old_content: &MethodContent,
    new_content: &MethodContent,
    snapshot_ref: SnapshotRef,
    supersede_link_id: String,
    outbox_event_ids: Vec<String>,
) -> Result<SupersedeMethodContentResponse, MethodLibraryError> {
    Ok(SupersedeMethodContentResponse {
        old_content_id: old_content.content_id.clone(),
        old_lifecycle_state: old_content.lifecycle.state,
        new_content_id: new_content.content_id.clone(),
        new_lifecycle_state: new_content.lifecycle.state,
        new_version: new_content.version.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "superseding content version is missing",
            )
        })?,
        new_fingerprint: new_content.fingerprint.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "superseding content fingerprint is missing",
            )
        })?,
        supersede_link_id,
        snapshot_ref,
        outbox_event_ids,
    })
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

fn build_deprecate_details(
    response: &DeprecateMethodContentResponse,
    expected_revision: i64,
    reason: String,
    effective_at: method_library_domain::content::Timestamp,
) -> BTreeMap<String, String> {
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
    details.insert("reason".to_string(), reason);
    details.insert("effective_at".to_string(), effective_at.to_string());
    details.insert(
        "outbox_event_id".to_string(),
        response.outbox_event_id.clone(),
    );
    details
}

fn build_retire_details(
    response: &RetireMethodContentResponse,
    expected_revision: i64,
    reason: String,
    retire_policy: String,
) -> BTreeMap<String, String> {
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
    details.insert("reason".to_string(), reason);
    details.insert("retire_policy".to_string(), retire_policy);
    details.insert(
        "outbox_event_id".to_string(),
        response.outbox_event_id.clone(),
    );
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

fn build_supersede_details(
    response: &SupersedeMethodContentResponse,
    old_version: ContentVersion,
    old_fingerprint: CanonicalFingerprint,
    gate_validation: GateValidationResult,
    reason: String,
    snapshot_ref: SnapshotRef,
    blob_ref: String,
) -> BTreeMap<String, String> {
    let GateValidationResult {
        approved: _,
        gate_ref,
        validated_at,
    } = gate_validation;
    let mut details = BTreeMap::new();
    details.insert(
        "old_content_id".to_string(),
        response.old_content_id.clone(),
    );
    details.insert(
        "old_lifecycle_state".to_string(),
        response.old_lifecycle_state.as_str().to_string(),
    );
    details.insert(
        "new_content_id".to_string(),
        response.new_content_id.clone(),
    );
    details.insert(
        "new_lifecycle_state".to_string(),
        response.new_lifecycle_state.as_str().to_string(),
    );
    details.insert("old_version".to_string(), old_version.raw);
    details.insert("new_version".to_string(), response.new_version.raw.clone());
    details.insert("old_fingerprint".to_string(), old_fingerprint.value);
    details.insert(
        "new_fingerprint".to_string(),
        response.new_fingerprint.value.clone(),
    );
    details.insert(
        "supersede_link_id".to_string(),
        response.supersede_link_id.clone(),
    );
    details.insert("reason".to_string(), reason);
    details.insert("gate_id".to_string(), gate_ref.gate_id);
    details.insert("gate_decision_id".to_string(), gate_ref.gate_decision_id);
    details.insert("gate_validated_at".to_string(), validated_at.to_string());
    details.insert("snapshot_id".to_string(), snapshot_ref.snapshot_id);
    details.insert("snapshot_blob_ref".to_string(), blob_ref);
    details.insert(
        "outbox_event_ids".to_string(),
        response.outbox_event_ids.join(","),
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
    build_definition_outbox_event(
        content,
        payload,
        Some(snapshot.snapshot_ref()),
        outbox_event_id,
        meta,
        now,
    )
}

fn build_deprecate_outbox_event(
    content: &MethodContent,
    reason: String,
    effective_at: method_library_domain::content::Timestamp,
    outbox_event_id: String,
    meta: RequestMeta,
    now: method_library_domain::content::Timestamp,
) -> Result<OutboxEvent, MethodLibraryError> {
    build_definition_outbox_event(
        content,
        DefinitionEventPayload::ContentDeprecated(ContentDeprecatedPayload {
            reason,
            effective_at,
        }),
        None,
        outbox_event_id,
        meta,
        now,
    )
}

fn build_retire_outbox_event(
    content: &MethodContent,
    reason: String,
    retire_policy: String,
    outbox_event_id: String,
    meta: RequestMeta,
    now: method_library_domain::content::Timestamp,
) -> Result<OutboxEvent, MethodLibraryError> {
    build_definition_outbox_event(
        content,
        DefinitionEventPayload::ContentRetired(ContentRetiredPayload {
            reason,
            retire_policy,
        }),
        None,
        outbox_event_id,
        meta,
        now,
    )
}

fn build_fingerprint_changed_outbox_event(
    content: &MethodContent,
    old_fingerprint: &CanonicalFingerprint,
    new_fingerprint: &CanonicalFingerprint,
    change_reason: &str,
    snapshot: &DefinitionSnapshot,
    outbox_event_id: String,
    meta: RequestMeta,
    now: method_library_domain::content::Timestamp,
) -> Result<OutboxEvent, MethodLibraryError> {
    build_definition_outbox_event(
        content,
        DefinitionEventPayload::FingerprintChanged(FingerprintChangedPayload {
            old_fingerprint: old_fingerprint.clone(),
            new_fingerprint: new_fingerprint.clone(),
            change_reason: change_reason.to_string(),
        }),
        Some(snapshot.snapshot_ref()),
        outbox_event_id,
        meta,
        now,
    )
}

fn build_definition_outbox_event(
    content: &MethodContent,
    payload: DefinitionEventPayload,
    snapshot_ref: Option<SnapshotRef>,
    outbox_event_id: String,
    meta: RequestMeta,
    now: method_library_domain::content::Timestamp,
) -> Result<OutboxEvent, MethodLibraryError> {
    let envelope = DefinitionEventEnvelope {
        event_id: outbox_event_id.clone(),
        event_type: payload.event_type(),
        schema_version: EVENT_SCHEMA_VERSION.to_string(),
        occurred_at: now,
        producer: OUTBOX_PRODUCER.to_string(),
        content_ref: published_content_ref(content)?,
        snapshot_ref,
        trace: EventTraceContext {
            request_id: meta.request_id.clone(),
            trace_id: meta.trace_id.clone(),
        },
        payload,
    };
    let payload_hash = hash_canonical_json(&envelope)?;
    OutboxEvent::new_pending(
        outbox_event_id,
        content.content_id.clone(),
        envelope,
        payload_hash,
        meta.idempotency_key.clone(),
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

async fn load_existing_deprecate_response(
    method_content_repository: &Arc<dyn MethodContentRepository>,
    tx: &mut crate::ports::UnitOfWorkTx,
    result_ref: ResultRef,
) -> Result<DeprecateMethodContentResponse, MethodLibraryError> {
    let lifecycle_result: LifecycleCommandResultRef =
        decode_json_result_ref(&result_ref, "deprecate idempotency result")?;
    let content = method_content_repository
        .get_for_update(tx, lifecycle_result.content_id)
        .await?
        .ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::MethodContentNotFound,
                "idempotent deprecate result could not be reloaded",
            )
        })?;

    build_deprecate_response(&content, lifecycle_result.outbox_event_id)
}

async fn load_existing_retire_response(
    method_content_repository: &Arc<dyn MethodContentRepository>,
    tx: &mut crate::ports::UnitOfWorkTx,
    result_ref: ResultRef,
) -> Result<RetireMethodContentResponse, MethodLibraryError> {
    let lifecycle_result: LifecycleCommandResultRef =
        decode_json_result_ref(&result_ref, "retire idempotency result")?;
    let content = method_content_repository
        .get_for_update(tx, lifecycle_result.content_id)
        .await?
        .ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::MethodContentNotFound,
                "idempotent retire result could not be reloaded",
            )
        })?;

    build_retire_response(&content, lifecycle_result.outbox_event_id)
}

async fn load_existing_supersede_response(
    method_content_repository: &Arc<dyn MethodContentRepository>,
    snapshot_repository: &Arc<dyn DefinitionSnapshotRepository>,
    tx: &mut crate::ports::UnitOfWorkTx,
    result_ref: ResultRef,
) -> Result<SupersedeMethodContentResponse, MethodLibraryError> {
    let supersede_result: SupersedeResultRef =
        decode_json_result_ref(&result_ref, "supersede idempotency result")?;
    let old_content = method_content_repository
        .get_for_update(tx, supersede_result.old_content_id.clone())
        .await?
        .ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::MethodContentNotFound,
                "idempotent supersede old content could not be reloaded",
            )
        })?;
    let new_content = method_content_repository
        .get_for_update(tx, supersede_result.new_content_id.clone())
        .await?
        .ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::MethodContentNotFound,
                "idempotent supersede replacement content could not be reloaded",
            )
        })?;
    let snapshot = snapshot_repository
        .get(supersede_result.snapshot_id)
        .await?
        .ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::SnapshotNotFound,
                "idempotent supersede snapshot could not be reloaded",
            )
        })?;

    build_supersede_response(
        &old_content,
        &new_content,
        snapshot.snapshot_ref(),
        supersede_result.supersede_link_id,
        supersede_result.outbox_event_ids,
    )
}

fn encode_json_result_ref<T: Serialize>(
    value: &T,
    label: &str,
) -> Result<String, MethodLibraryError> {
    serde_json::to_string(value).map_err(|error| {
        MethodLibraryError::retryable(
            MethodLibraryErrorCode::PersistenceUnavailable,
            format!("failed to encode {label}: {error}"),
        )
    })
}

fn decode_json_result_ref<T: for<'de> Deserialize<'de>>(
    value: &str,
    label: &str,
) -> Result<T, MethodLibraryError> {
    serde_json::from_str(value).map_err(|error| {
        MethodLibraryError::validation(
            MethodLibraryErrorCode::PersistenceUnavailable,
            format!("failed to decode {label}: {error}"),
        )
    })
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

fn validate_deprecate_command(
    command: &DeprecateMethodContentCommand,
) -> Result<(), MethodLibraryError> {
    if command.content_id.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "deprecated content id must not be empty",
        ));
    }

    if command.reason.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "deprecation reason must not be empty",
        ));
    }

    Ok(())
}

fn validate_retire_command(command: &RetireMethodContentCommand) -> Result<(), MethodLibraryError> {
    if command.content_id.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "retired content id must not be empty",
        ));
    }

    if command.reason.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "retirement reason must not be empty",
        ));
    }

    if command.retire_policy.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "retire policy must not be empty",
        ));
    }

    Ok(())
}

fn validate_supersede_command(
    command: &SupersedeMethodContentCommand,
) -> Result<(), MethodLibraryError> {
    if command.old_content_id.trim().is_empty() || command.new_content_id.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "supersede content ids must not be empty",
        ));
    }

    if command.reason.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::BoundaryViolation,
            "supersede reason must not be empty",
        ));
    }

    if !command.approved_gate_ref.is_complete() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::PublishGateRequired,
            "supersede requires a complete approved gate reference",
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
        InMemorySupersedeLinkRepository, StaticGovernancePort,
    };
    use crate::ports::{MethodContentRepository, UnitOfWork};
    use method_library_contracts::{
        ActorContext, ArtifactRef, CreateMethodContentDraftCommand, DefinitionEventType,
        DeprecateMethodContentCommand, PublishMethodContentCommand, RequestMeta,
        RetireMethodContentCommand, SubmitMethodContentForReviewCommand,
        SupersedeMethodContentCommand, UpdateMethodContentDraftCommand,
    };
    use method_library_domain::MethodLibraryErrorCode;
    use method_library_domain::content::ActorKind;
    use method_library_domain::content::{
        ApprovedGateRef, CanonicalFingerprint, ContentRef, ContentVersion, FingerprintAlgorithm,
        LifecycleState, MethodContent, MethodContentKind, ReferenceState,
    };
    use method_library_domain::definitions::{
        EvidenceKind, EvidenceRule, MethodContentPayload, Qualification, QualificationLevel,
        QualificationLevelModel, RoleDefinition,
    };

    struct TestHarness {
        service: MethodContentCommandService,
        content_repository: Arc<InMemoryMethodContentRepository>,
        version_repository: Arc<InMemoryMethodContentVersionRepository>,
        snapshot_repository: Arc<InMemoryDefinitionSnapshotRepository>,
        supersede_link_repository: Arc<InMemorySupersedeLinkRepository>,
        outbox_repository: Arc<InMemoryOutboxRepository>,
        object_storage: Arc<InMemoryObjectStorage>,
        lifecycle_history_repository: Arc<InMemoryLifecycleHistoryRepository>,
    }

    fn sample_harness() -> TestHarness {
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
        let clock = Arc::new(DeterministicClock::new(datetime!(2026-05-26 08:00:00 UTC)));
        let id_generator = Arc::new(DeterministicIdGenerator::default());

        let service = MethodContentCommandService::new(
            Arc::new(FakeUnitOfWork),
            content_repository.clone(),
            reference_repository,
            version_repository.clone(),
            snapshot_repository.clone(),
            supersede_link_repository.clone(),
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
            supersede_link_repository,
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
            payload: sample_payload_for(MethodContentKind::Qualification),
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

    fn sample_payload_for(kind: MethodContentKind) -> MethodContentPayload {
        match kind {
            MethodContentKind::Qualification => {
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
                })
            }
            MethodContentKind::RoleDefinition => {
                MethodContentPayload::RoleDefinition(RoleDefinition {
                    role_key: "role-1".to_string(),
                    responsibilities: vec!["Own quality outcomes".to_string()],
                    qualification_refs: Vec::new(),
                    default_view_profile_refs: Vec::new(),
                })
            }
            unsupported => panic!(
                "test payload fixture is missing for {}",
                unsupported.as_str()
            ),
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

    fn sample_deprecate_command(
        content_id: impl Into<String>,
        expected_revision: i64,
    ) -> DeprecateMethodContentCommand {
        DeprecateMethodContentCommand {
            content_id: content_id.into(),
            expected_revision,
            reason: "Superseded by newer guidance".to_string(),
            effective_at: Some(datetime!(2026-05-26 08:20:00 UTC)),
        }
    }

    fn sample_retire_command(
        content_id: impl Into<String>,
        expected_revision: i64,
    ) -> RetireMethodContentCommand {
        RetireMethodContentCommand {
            content_id: content_id.into(),
            expected_revision,
            reason: "Retired for trace-only retention".to_string(),
            retire_policy: "stop_new_usage".to_string(),
        }
    }

    fn sample_supersede_command(
        old_content_id: impl Into<String>,
        old_expected_revision: i64,
        new_content_id: impl Into<String>,
        new_expected_revision: i64,
        gate_id: &str,
        gate_decision_id: &str,
    ) -> SupersedeMethodContentCommand {
        SupersedeMethodContentCommand {
            old_content_id: old_content_id.into(),
            old_expected_revision,
            new_content_id: new_content_id.into(),
            new_expected_revision,
            new_version: ContentVersion::new("2.0.0").expect("version should be valid"),
            approved_gate_ref: ApprovedGateRef {
                gate_id: gate_id.to_string(),
                gate_decision_id: gate_decision_id.to_string(),
                approved_at: datetime!(2026-05-26 08:15:00 UTC),
            },
            reason: "Replaced by a newer definition".to_string(),
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

    async fn insert_in_review_content(
        repository: &Arc<InMemoryMethodContentRepository>,
        content_id: &str,
        actor_id: &str,
        kind: MethodContentKind,
    ) -> String {
        let mut content = MethodContent::create_draft(
            content_id.to_string(),
            format!("family-{content_id}"),
            kind,
            format!("In review {content_id}"),
            None,
            sample_payload_for(kind),
            actor_id.to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("in-review fixture draft should build");
        content
            .submit_for_review(actor_id.to_string(), datetime!(2026-05-26 08:05:00 UTC))
            .expect("fixture should enter review");

        let mut tx = FakeUnitOfWork
            .begin(sample_meta_with("idem-seed-review", "hash-seed-review"))
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
    async fn deprecates_published_content_and_reuses_idempotent_result() {
        let harness = sample_harness();
        let actor = sample_actor();
        let content_id = insert_published_content(
            &harness.content_repository,
            "content-published",
            &actor.actor_id,
        )
        .await;
        let meta = sample_meta_with("idem-5", "hash-5");
        let command = sample_deprecate_command(content_id.clone(), 3);

        let response = harness
            .service
            .deprecate(command.clone(), actor.clone(), meta.clone())
            .await
            .expect("published content should deprecate");

        assert_eq!(response.content_id, content_id);
        assert_eq!(response.lifecycle_state, LifecycleState::Deprecated);
        assert_eq!(response.revision, 4);

        let replayed = harness
            .service
            .deprecate(command, actor, meta)
            .await
            .expect("idempotent deprecate should reuse the response");
        assert_eq!(replayed, response);

        let entries = harness
            .lifecycle_history_repository
            .entries()
            .expect("history should be inspectable");
        let last_entry = entries.last().expect("history entry should exist");
        assert_eq!(last_entry.from_state, Some(LifecycleState::Published));
        assert_eq!(last_entry.to_state, LifecycleState::Deprecated);

        let events = harness
            .outbox_repository
            .events()
            .expect("outbox should be inspectable");
        let event = events
            .get(&response.outbox_event_id)
            .expect("deprecated event should exist");
        assert_eq!(
            event.envelope.event_type,
            DefinitionEventType::ContentDeprecated
        );
    }

    #[tokio::test]
    async fn retires_deprecated_content_and_reuses_idempotent_result() {
        let harness = sample_harness();
        let actor = sample_actor();
        let content_id = insert_published_content(
            &harness.content_repository,
            "content-published",
            &actor.actor_id,
        )
        .await;
        harness
            .service
            .deprecate(
                sample_deprecate_command(content_id.clone(), 3),
                actor.clone(),
                sample_meta_with("idem-5", "hash-5"),
            )
            .await
            .expect("content should deprecate before retirement");

        let meta = sample_meta_with("idem-6", "hash-6");
        let command = sample_retire_command(content_id.clone(), 4);
        let response = harness
            .service
            .retire(command.clone(), actor.clone(), meta.clone())
            .await
            .expect("deprecated content should retire");

        assert_eq!(response.content_id, content_id);
        assert_eq!(response.lifecycle_state, LifecycleState::Retired);
        assert_eq!(response.revision, 5);

        let replayed = harness
            .service
            .retire(command, actor, meta)
            .await
            .expect("idempotent retire should reuse the response");
        assert_eq!(replayed, response);

        let entries = harness
            .lifecycle_history_repository
            .entries()
            .expect("history should be inspectable");
        let last_entry = entries.last().expect("history entry should exist");
        assert_eq!(last_entry.from_state, Some(LifecycleState::Deprecated));
        assert_eq!(last_entry.to_state, LifecycleState::Retired);

        let events = harness
            .outbox_repository
            .events()
            .expect("outbox should be inspectable");
        let event = events
            .get(&response.outbox_event_id)
            .expect("retired event should exist");
        assert_eq!(
            event.envelope.event_type,
            DefinitionEventType::ContentRetired
        );
    }

    #[tokio::test]
    async fn supersedes_old_content_and_publishes_replacement() {
        let harness = sample_harness();
        let actor = sample_actor();
        let old_content_id =
            insert_published_content(&harness.content_repository, "content-old", &actor.actor_id)
                .await;
        let new_content_id = insert_in_review_content(
            &harness.content_repository,
            "content-new",
            &actor.actor_id,
            MethodContentKind::Qualification,
        )
        .await;

        let response = harness
            .service
            .supersede(
                sample_supersede_command(
                    old_content_id.clone(),
                    3,
                    new_content_id.clone(),
                    2,
                    "gate-1",
                    "decision-1",
                ),
                actor,
                sample_meta_with("idem-7", "hash-7"),
            )
            .await
            .expect("supersede should succeed");

        assert_eq!(response.old_content_id, old_content_id);
        assert_eq!(response.old_lifecycle_state, LifecycleState::Superseded);
        assert_eq!(response.new_content_id, new_content_id);
        assert_eq!(response.new_lifecycle_state, LifecycleState::Published);
        assert_eq!(response.new_version.raw, "2.0.0");
        assert_eq!(response.outbox_event_ids.len(), 2);

        let contents = harness
            .content_repository
            .contents()
            .expect("contents should be inspectable");
        let old_content = contents
            .get(&response.old_content_id)
            .expect("old content should be stored");
        let new_content = contents
            .get(&response.new_content_id)
            .expect("new content should be stored");
        assert_eq!(old_content.lifecycle.state, LifecycleState::Superseded);
        assert_eq!(
            old_content.superseded_by_content_id.as_deref(),
            Some(response.new_content_id.as_str())
        );
        assert_eq!(new_content.lifecycle.state, LifecycleState::Published);
        assert_eq!(
            new_content.supersedes_content_id.as_deref(),
            Some(response.old_content_id.as_str())
        );
        assert_eq!(new_content.content_family_id, old_content.content_family_id);

        let links = harness
            .supersede_link_repository
            .links()
            .expect("links should be inspectable");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].old_content_id, response.old_content_id);
        assert_eq!(links[0].new_content_id, response.new_content_id);
        assert_eq!(links[0].content_family_id, old_content.content_family_id);

        let events = harness
            .outbox_repository
            .events()
            .expect("outbox should be inspectable");
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[&response.outbox_event_ids[0]].envelope.event_type,
            DefinitionEventType::FingerprintChanged
        );
        assert_eq!(
            events[&response.outbox_event_ids[1]].envelope.event_type,
            DefinitionEventType::ContentPublished
        );
    }

    #[tokio::test]
    async fn rejects_supersede_when_kinds_do_not_match() {
        let harness = sample_harness();
        let actor = sample_actor();
        let old_content_id =
            insert_published_content(&harness.content_repository, "content-old", &actor.actor_id)
                .await;
        let new_content_id = insert_in_review_content(
            &harness.content_repository,
            "content-new",
            &actor.actor_id,
            MethodContentKind::RoleDefinition,
        )
        .await;

        let error = harness
            .service
            .supersede(
                sample_supersede_command(
                    old_content_id,
                    3,
                    new_content_id,
                    2,
                    "gate-1",
                    "decision-1",
                ),
                actor,
                sample_meta_with("idem-7", "hash-7"),
            )
            .await
            .expect_err("supersede should reject mismatched kinds");

        assert_eq!(error.code, MethodLibraryErrorCode::SupersedeKindMismatch);
        let links = harness
            .supersede_link_repository
            .links()
            .expect("links should be inspectable");
        assert!(links.is_empty());
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
