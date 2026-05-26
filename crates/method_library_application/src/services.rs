//! Application services that orchestrate P0 method-content command flows.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use method_library_contracts::{
    ActorContext, ArtifactRef, CreateMethodContentDraftCommand, CreateMethodContentDraftResponse,
    RequestMeta,
};
use method_library_domain::content::{ContentRef, MethodContent};
use method_library_domain::definitions::MethodContentPayload;
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};

use crate::ports::{
    AuditRecord, AuditRepository, AuditTargetRef, Clock, FailureReason, IdGenerator,
    IdempotencyBeginResult, IdempotencyRepository, IdempotencyScope,
    MethodContentReferenceRepository, MethodContentRepository, ResultRef, UnitOfWork,
};

const CREATE_DRAFT_SCOPE: &str = "command:create_draft";

/// Application service for method-content command flows.
#[derive(Clone)]
pub struct MethodContentCommandService {
    unit_of_work: Arc<dyn UnitOfWork>,
    method_content_repository: Arc<dyn MethodContentRepository>,
    method_content_reference_repository: Arc<dyn MethodContentReferenceRepository>,
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
        audit_repository: Arc<dyn AuditRepository>,
        idempotency_repository: Arc<dyn IdempotencyRepository>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Self {
        Self {
            unit_of_work,
            method_content_repository,
            method_content_reference_repository,
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
                                &response,
                                &actor,
                                &meta,
                                "create_draft",
                                "succeeded",
                                &references,
                                &source_refs,
                                audit_id,
                            )?,
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

fn build_audit_record(
    response: &CreateMethodContentDraftResponse,
    actor: &ActorContext,
    meta: &RequestMeta,
    action: &str,
    result: &str,
    references: &[ContentRef],
    source_refs: &[ArtifactRef],
    audit_id: String,
) -> Result<AuditRecord, MethodLibraryError> {
    let mut details = BTreeMap::new();
    details.insert("kind".to_string(), response.kind.as_str().to_string());
    details.insert(
        "content_family_id".to_string(),
        response.content_family_id.clone(),
    );
    details.insert("revision".to_string(), response.revision.to_string());
    details.insert("reference_count".to_string(), references.len().to_string());
    details.insert(
        "source_ref_count".to_string(),
        source_refs.len().to_string(),
    );
    details.insert("action".to_string(), action.to_string());
    details.insert("result".to_string(), result.to_string());

    Ok(AuditRecord {
        audit_id,
        request_id: meta.request_id.clone(),
        trace_id: meta.trace_id.clone(),
        actor_context: actor.clone(),
        target_ref: AuditTargetRef {
            target_type: "method_content".to_string(),
            target_id: response.content_id.clone(),
        },
        action: action.to_string(),
        result: result.to_string(),
        details,
        occurred_at: meta.received_at,
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
        InMemoryIdempotencyRepository, InMemoryMethodContentReferenceRepository,
        InMemoryMethodContentRepository,
    };
    use method_library_contracts::{
        ActorContext, ArtifactRef, CreateMethodContentDraftCommand, RequestMeta,
    };
    use method_library_domain::content::ActorKind;
    use method_library_domain::definitions::{
        EvidenceKind, EvidenceRule, MethodContentPayload, Qualification, QualificationLevel,
        QualificationLevelModel,
    };

    fn sample_service() -> MethodContentCommandService {
        MethodContentCommandService::new(
            Arc::new(FakeUnitOfWork),
            Arc::new(InMemoryMethodContentRepository::default()),
            Arc::new(InMemoryMethodContentReferenceRepository::default()),
            Arc::new(InMemoryAuditRepository::default()),
            Arc::new(InMemoryIdempotencyRepository::default()),
            Arc::new(DeterministicClock::new(datetime!(2026-05-26 08:00:00 UTC))),
            Arc::new(DeterministicIdGenerator::default()),
        )
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
        RequestMeta {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            idempotency_key: Some("idem-1".to_string()),
            request_hash: "hash-1".to_string(),
            received_at: datetime!(2026-05-26 08:00:00 UTC),
        }
    }

    fn sample_command() -> CreateMethodContentDraftCommand {
        CreateMethodContentDraftCommand {
            kind: method_library_domain::content::MethodContentKind::Qualification,
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
            references: vec![method_library_domain::content::ContentRef {
                target_content_id: "content-ref-1".to_string(),
                target_kind: method_library_domain::content::MethodContentKind::ViewProfile,
                required_state: method_library_domain::content::ReferenceState::Published,
            }],
            source_refs: vec![ArtifactRef {
                artifact_id: "artifact-1".to_string(),
                artifact_kind: "evidence".to_string(),
            }],
        }
    }

    #[tokio::test]
    async fn creates_draft_and_reuses_idempotent_result() {
        let service = sample_service();
        let actor = sample_actor();
        let meta = sample_meta();
        let command = sample_command();

        let response = service
            .create_draft(command.clone(), actor.clone(), meta.clone())
            .await
            .expect("draft should be created");

        assert_eq!(response.content_id, "content-1");
        assert_eq!(
            response.lifecycle_state,
            method_library_domain::content::LifecycleState::Draft
        );
        assert_eq!(response.revision, 1);

        let replayed = service
            .create_draft(command, actor, meta)
            .await
            .expect("idempotent draft should reuse the response");

        assert_eq!(replayed, response);
    }
}
