#![allow(dead_code)]

use std::sync::Arc;

use method_library_application::ports::fakes::{
    DeterministicClock, DeterministicFingerprintHasher, DeterministicIdGenerator, FakeUnitOfWork,
    InMemoryAuditRepository, InMemoryContentSummaryProjectionRepository,
    InMemoryDefinitionSnapshotRepository, InMemoryDefinitionTraceProjectionRepository,
    InMemoryIdempotencyRepository, InMemoryJobRunRepository, InMemoryLifecycleHistoryRepository,
    InMemoryMethodContentReferenceRepository, InMemoryMethodContentRepository,
    InMemoryMethodContentVersionRepository, InMemoryObjectStorage, InMemoryOutboxRepository,
    InMemoryProjectionCheckpointRepository, InMemorySupersedeLinkRepository, RecordingBusPublisher,
    RecordingObservabilityPort, StaticGovernancePort,
};
use method_library_application::ports::{OutboxStatus, ProjectionCheckpointRepository};
use method_library_application::{
    MethodContentCommandService, MethodContentQueryService, MethodOperationsService,
    OutboxRelayPolicy, OutboxRelayService, OutboxRelayTopics,
};
use method_library_contracts::{
    ActorContext, ActorRef, CreateMethodContentDraftCommand, ExportDefinitionSnapshotQuery,
    PublishMethodContentCommand, PublishMethodContentResponse, RelayOutboxEventsJobRequest,
    ReplayDefinitionEventsJobRequest, ReplayDefinitionEventsJobResult, RequestMeta,
    ResolveViewProfileQuery, ResolveViewProfileResponse,
};
use method_library_domain::content::{
    ActorKind, ApprovedGateRef, ContentVersion, MethodContentKind, PublishedContentRef,
};
use method_library_domain::definitions::{
    AIPolicyConstraint, AIPolicyDef, ActionAvailability, ActivityDefinition, ArtifactKindRef,
    EvidenceKind, EvidenceRule, FieldVisibility, MethodContentPayload, PolicyRuleRef,
    PolicySeverity, ProcessTemplateDef, Qualification, QualificationLevel, QualificationLevelModel,
    QualityRule, QualityRuleSeverity, RoleDefinition, SchemaRef, TailoringEffect,
    TailoringEffectKind, TailoringOption, TailoringPoint, TaskDefinition, TaskStepDefinition,
    ViewActionRule, ViewCondition, ViewConditionOperator, ViewFieldRule, ViewObjectKind,
    ViewProfile, ViewScopeRule, WorkProductDefinition,
};
use serde_json::json;
use time::macros::datetime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedFixture {
    pub content_id: String,
    pub kind: MethodContentKind,
    pub name: String,
    pub content_ref: PublishedContentRef,
    pub snapshot_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruthCounts {
    pub contents: usize,
    pub snapshots: usize,
    pub outbox_events: usize,
    pub audits: usize,
    pub idempotency_records: usize,
}

pub struct ContractHarness {
    pub command_service: MethodContentCommandService,
    pub query_service: MethodContentQueryService,
    pub operations_service: MethodOperationsService,
    pub live_relay_service: OutboxRelayService,
    pub content_repository: Arc<InMemoryMethodContentRepository>,
    pub snapshot_repository: Arc<InMemoryDefinitionSnapshotRepository>,
    pub outbox_repository: Arc<InMemoryOutboxRepository>,
    pub audit_repository: Arc<InMemoryAuditRepository>,
    pub idempotency_repository: Arc<InMemoryIdempotencyRepository>,
    pub checkpoint_repository: Arc<InMemoryProjectionCheckpointRepository>,
    pub live_bus: Arc<RecordingBusPublisher>,
    pub replay_bus: Arc<RecordingBusPublisher>,
}

impl ContractHarness {
    #[must_use]
    pub fn new() -> Self {
        let unit_of_work = Arc::new(FakeUnitOfWork);
        let content_repository = Arc::new(InMemoryMethodContentRepository::default());
        let reference_repository = Arc::new(InMemoryMethodContentReferenceRepository::default());
        let version_repository = Arc::new(InMemoryMethodContentVersionRepository::default());
        let snapshot_repository = Arc::new(InMemoryDefinitionSnapshotRepository::default());
        let supersede_repository = Arc::new(InMemorySupersedeLinkRepository::default());
        let outbox_repository = Arc::new(InMemoryOutboxRepository::default());
        let lifecycle_repository = Arc::new(InMemoryLifecycleHistoryRepository::default());
        let audit_repository = Arc::new(InMemoryAuditRepository::default());
        let idempotency_repository = Arc::new(InMemoryIdempotencyRepository::default());
        let object_storage = Arc::new(InMemoryObjectStorage::default());
        let summary_repository = Arc::new(InMemoryContentSummaryProjectionRepository::default());
        let trace_repository = Arc::new(InMemoryDefinitionTraceProjectionRepository::default());
        let checkpoint_repository = Arc::new(InMemoryProjectionCheckpointRepository::default());
        let job_run_repository = Arc::new(InMemoryJobRunRepository::default());
        let governance = Arc::new(StaticGovernancePort::new(
            true,
            datetime!(2026-05-27 05:00:00 UTC),
        ));
        let fingerprint_hasher = Arc::new(DeterministicFingerprintHasher::default());
        let clock = Arc::new(DeterministicClock::new(datetime!(2026-05-27 05:00:00 UTC)));
        let id_generator = Arc::new(DeterministicIdGenerator::default());
        let live_bus = Arc::new(RecordingBusPublisher::default());
        let replay_bus = Arc::new(RecordingBusPublisher::default());

        let command_service = MethodContentCommandService::new(
            unit_of_work.clone(),
            content_repository.clone(),
            reference_repository.clone(),
            version_repository.clone(),
            snapshot_repository.clone(),
            supersede_repository.clone(),
            outbox_repository.clone(),
            lifecycle_repository.clone(),
            audit_repository.clone(),
            idempotency_repository.clone(),
            governance,
            object_storage.clone(),
            fingerprint_hasher.clone(),
            clock.clone(),
            id_generator,
        );
        let query_service = MethodContentQueryService::new(
            content_repository.clone(),
            reference_repository,
            version_repository.clone(),
            snapshot_repository.clone(),
            object_storage,
            summary_repository,
            trace_repository,
            checkpoint_repository.clone(),
            clock.clone(),
        );
        let operations_service = MethodOperationsService::new(
            unit_of_work,
            Arc::new(command_service.clone()),
            content_repository.clone(),
            version_repository,
            lifecycle_repository,
            audit_repository.clone(),
            supersede_repository,
            outbox_repository.clone(),
            Arc::new(InMemoryContentSummaryProjectionRepository::default()),
            Arc::new(InMemoryDefinitionTraceProjectionRepository::default()),
            checkpoint_repository.clone(),
            snapshot_repository.clone(),
            job_run_repository,
            replay_bus.clone(),
            fingerprint_hasher,
            clock.clone(),
        );
        let live_relay_service = OutboxRelayService::new(
            outbox_repository.clone(),
            live_bus.clone(),
            Arc::new(RecordingObservabilityPort::default()),
            clock,
            OutboxRelayTopics {
                definition_events: "method-library.definition.events".to_string(),
                lifecycle_events: "method-library.lifecycle.events".to_string(),
            },
            OutboxRelayPolicy {
                max_attempts: 3,
                retry_backoff_ms: 30_000,
            },
        );

        Self {
            command_service,
            query_service,
            operations_service,
            live_relay_service,
            content_repository,
            snapshot_repository,
            outbox_repository,
            audit_repository,
            idempotency_repository,
            checkpoint_repository,
            live_bus,
            replay_bus,
        }
    }

    pub async fn publish_qualification(&self, suffix: &str) -> PublishedFixture {
        self.publish_definition(
            suffix,
            MethodContentKind::Qualification,
            format!("Qualification {suffix}"),
            MethodContentPayload::Qualification(Qualification {
                qualification_key: format!("qualification_{suffix}"),
                name: format!("Qualification {suffix}"),
                description: Some(format!("Qualification definition for {suffix}")),
                level_model: QualificationLevelModel {
                    levels: vec![QualificationLevel {
                        level_key: "baseline".to_string(),
                        name: "Baseline".to_string(),
                        order: 1,
                        description: None,
                    }],
                    default_level_key: Some("baseline".to_string()),
                },
                evidence_rules: vec![EvidenceRule {
                    evidence_kind: EvidenceKind::Document,
                    required: true,
                    description: "Provide evidence".to_string(),
                }],
            }),
        )
        .await
    }

    pub async fn publish_role(
        &self,
        suffix: &str,
        qualification_ref: PublishedContentRef,
    ) -> PublishedFixture {
        self.publish_definition(
            suffix,
            MethodContentKind::RoleDefinition,
            format!("Role {suffix}"),
            MethodContentPayload::RoleDefinition(RoleDefinition {
                role_key: format!("role_{suffix}"),
                responsibilities: vec!["Coordinate delivery".to_string()],
                qualification_refs: vec![qualification_ref],
                default_view_profile_refs: Vec::new(),
            }),
        )
        .await
    }

    pub async fn publish_work_product(&self, suffix: &str) -> PublishedFixture {
        self.publish_definition(
            suffix,
            MethodContentKind::WorkProductDefinition,
            format!("Work Product {suffix}"),
            MethodContentPayload::WorkProductDefinition(WorkProductDefinition {
                work_product_key: format!("work_product_{suffix}"),
                artifact_kind: ArtifactKindRef {
                    artifact_kind: format!("artifact-{suffix}"),
                    schema_version: Some("1.0".to_string()),
                },
                schema_ref: SchemaRef {
                    schema_uri: format!("schema://artifact/{suffix}"),
                    schema_version: Some("1.0".to_string()),
                    schema_fingerprint: None,
                },
                quality_rules: vec![QualityRule {
                    rule_key: format!("quality_rule_{suffix}"),
                    description: "Artifact must be reviewable".to_string(),
                    severity: QualityRuleSeverity::Blocking,
                }],
            }),
        )
        .await
    }

    pub async fn publish_task(
        &self,
        suffix: &str,
        work_product_ref: PublishedContentRef,
    ) -> PublishedFixture {
        self.publish_definition(
            suffix,
            MethodContentKind::TaskDefinition,
            format!("Task {suffix}"),
            MethodContentPayload::TaskDefinition(TaskDefinition {
                task_key: format!("task_{suffix}"),
                purpose: "Produce and verify a work product".to_string(),
                step_defs: vec![TaskStepDefinition {
                    step_key: format!("task_step_{suffix}"),
                    order: 1,
                    title: "Create draft".to_string(),
                    purpose: Some("Draft the artifact".to_string()),
                    expected_output: Some("One artifact revision".to_string()),
                    verification: Some("Peer review completed".to_string()),
                }],
                input_work_product_refs: vec![work_product_ref.clone()],
                output_work_product_refs: vec![work_product_ref],
            }),
        )
        .await
    }

    pub async fn publish_process_template(
        &self,
        suffix: &str,
        task_ref: PublishedContentRef,
        work_product_ref: PublishedContentRef,
        role_ref: PublishedContentRef,
    ) -> PublishedFixture {
        self.publish_definition(
            suffix,
            MethodContentKind::ProcessTemplateDef,
            format!("Process Template {suffix}"),
            MethodContentPayload::ProcessTemplateDef(ProcessTemplateDef {
                template_key: format!("process_template_{suffix}"),
                activity_defs: vec![ActivityDefinition {
                    activity_key: format!("activity_{suffix}"),
                    name: format!("Activity {suffix}"),
                    task_definition_refs: vec![task_ref],
                    input_work_product_refs: vec![work_product_ref.clone()],
                    output_work_product_refs: vec![work_product_ref],
                    role_refs: vec![role_ref],
                }],
                work_product_refs: vec![],
                role_refs: vec![],
                tailoring_points: vec![TailoringPoint {
                    tailoring_key: format!("tailoring_{suffix}"),
                    description: "Optional review policy".to_string(),
                    allowed_options: vec![TailoringOption {
                        option_key: format!("tailoring_option_{suffix}"),
                        description: "Require formal review".to_string(),
                        effect: TailoringEffect {
                            effect_kind: TailoringEffectKind::Require,
                            target_keys: vec!["peer_review".to_string()],
                            parameters: json!({ "required": true }),
                        },
                    }],
                }],
            }),
        )
        .await
    }

    pub async fn publish_view_profile(
        &self,
        suffix: &str,
        role_ref: PublishedContentRef,
    ) -> PublishedFixture {
        self.publish_definition(
            suffix,
            MethodContentKind::ViewProfile,
            format!("View Profile {suffix}"),
            MethodContentPayload::ViewProfile(ViewProfile {
                role_ref,
                object_kind: ViewObjectKind::WorkItem,
                scope_rules: vec![ViewScopeRule {
                    scope_key: format!("scope_{suffix}"),
                    conditions: vec![ViewCondition {
                        field_path: "project.type".to_string(),
                        operator: ViewConditionOperator::Eq,
                        value_json: Some(json!("service")),
                    }],
                }],
                field_rules: vec![ViewFieldRule {
                    field_path: "summary".to_string(),
                    visibility: FieldVisibility::Visible,
                }],
                action_rules: vec![ViewActionRule {
                    action_key: "edit".to_string(),
                    availability: ActionAvailability::Available,
                }],
            }),
        )
        .await
    }

    pub async fn publish_ai_policy(&self, suffix: &str) -> PublishedFixture {
        self.publish_definition(
            suffix,
            MethodContentKind::AIPolicyDef,
            format!("AI Policy {suffix}"),
            MethodContentPayload::AIPolicyDef(AIPolicyDef {
                policy_key: format!("ai_policy_{suffix}"),
                target_actor_kind: ActorKind::AiMember,
                rule_refs: vec![PolicyRuleRef {
                    rule_key: format!("rule_{suffix}"),
                    version: Some("1.0".to_string()),
                }],
                constraints: vec![AIPolicyConstraint {
                    constraint_key: format!("constraint_{suffix}"),
                    description: "Do not bypass human review".to_string(),
                    severity: PolicySeverity::Blocking,
                }],
            }),
        )
        .await
    }

    pub async fn relay_pending_events(&self, worker_id: &str) {
        let result = self
            .live_relay_service
            .relay_pending_events(
                RelayOutboxEventsJobRequest {
                    worker_id: worker_id.to_string(),
                    batch_size: 50,
                    lease_seconds: 60,
                },
                relay_meta(worker_id),
            )
            .await
            .expect("relay should succeed");

        assert!(result.published_count > 0);
    }

    pub async fn export_snapshot(
        &self,
        fixture: &PublishedFixture,
    ) -> method_library_contracts::ExportDefinitionSnapshotResponse {
        self.query_service
            .export_definition_snapshot(
                ExportDefinitionSnapshotQuery {
                    snapshot_id: Some(fixture.snapshot_id.clone()),
                    content_id: None,
                    version: None,
                    verify_fingerprint: true,
                },
                sample_actor(),
                read_meta("snapshot-export"),
            )
            .await
            .expect("snapshot export should succeed")
    }

    pub async fn resolve_view_profile(
        &self,
        query: ResolveViewProfileQuery,
    ) -> ResolveViewProfileResponse {
        self.query_service
            .resolve_view_profile(query, sample_actor(), read_meta("resolve-view-profile"))
            .await
            .expect("view-profile resolve should succeed")
    }

    pub async fn replay_definition_events(
        &self,
        consumer: &str,
        from_cursor: Option<String>,
    ) -> ReplayDefinitionEventsJobResult {
        self.operations_service
            .replay_definition_events(
                ReplayDefinitionEventsJobRequest {
                    consumer: consumer.to_string(),
                    from_cursor,
                    event_types: Vec::new(),
                    batch_size: 50,
                    dry_run: false,
                },
                worker_actor(),
                job_meta("replay"),
            )
            .await
            .expect("replay should succeed")
    }

    #[must_use]
    pub fn live_bus_events(
        &self,
    ) -> Vec<(
        String,
        method_library_contracts::DefinitionEventEnvelope,
        RequestMeta,
    )> {
        self.live_bus
            .published_events()
            .expect("live bus events should be readable")
    }

    #[must_use]
    pub fn replay_bus_events(
        &self,
    ) -> Vec<(
        String,
        method_library_contracts::DefinitionEventEnvelope,
        RequestMeta,
    )> {
        self.replay_bus
            .published_events()
            .expect("replay bus events should be readable")
    }

    #[must_use]
    pub fn published_event_for(
        &self,
        content_id: &str,
    ) -> method_library_contracts::DefinitionEventEnvelope {
        self.live_bus_events()
            .into_iter()
            .map(|(_, event, _)| event)
            .find(|event| event.content_ref.content_id == content_id)
            .expect("published event should exist")
    }

    #[must_use]
    pub fn truth_counts(&self) -> TruthCounts {
        TruthCounts {
            contents: self
                .content_repository
                .contents()
                .expect("contents should be readable")
                .len(),
            snapshots: self
                .snapshot_repository
                .snapshots()
                .expect("snapshots should be readable")
                .len(),
            outbox_events: self
                .outbox_repository
                .events()
                .expect("outbox should be readable")
                .len(),
            audits: self
                .audit_repository
                .records()
                .expect("audit records should be readable")
                .len(),
            idempotency_records: self
                .idempotency_repository
                .records()
                .expect("idempotency records should be readable")
                .len(),
        }
    }

    pub async fn checkpoint_cursor(&self, checkpoint_name: &str) -> Option<String> {
        self.checkpoint_repository
            .get(checkpoint_name.to_string())
            .await
            .expect("checkpoint should be readable")
            .and_then(|record| record.last_processed_event_id)
    }

    #[must_use]
    pub fn outbox_statuses(&self) -> Vec<OutboxStatus> {
        let mut statuses = self
            .outbox_repository
            .events()
            .expect("outbox should be readable")
            .into_values()
            .map(|event| event.status)
            .collect::<Vec<_>>();
        statuses.sort_by_key(|status| format!("{status:?}"));
        statuses
    }

    async fn publish_definition(
        &self,
        suffix: &str,
        kind: MethodContentKind,
        name: String,
        payload: MethodContentPayload,
    ) -> PublishedFixture {
        let meta_suffix = format!("{suffix}-{}", kind.as_str());
        let create_response = self
            .command_service
            .create_draft(
                CreateMethodContentDraftCommand {
                    kind,
                    name: name.clone(),
                    description: Some(format!("Definition {suffix}")),
                    payload,
                    references: Vec::new(),
                    source_refs: Vec::new(),
                },
                sample_actor(),
                command_meta("create", &meta_suffix),
            )
            .await
            .expect("draft creation should succeed");
        let submit_response = self
            .command_service
            .submit_for_review(
                method_library_contracts::SubmitMethodContentForReviewCommand {
                    content_id: create_response.content_id.clone(),
                    expected_revision: create_response.revision,
                    review_reason: Some(format!("Submit {suffix} for review")),
                    review_evidence_refs: Vec::new(),
                },
                sample_actor(),
                command_meta("submit", &meta_suffix),
            )
            .await
            .expect("submit for review should succeed");
        let publish_response = self
            .command_service
            .publish(
                PublishMethodContentCommand {
                    content_id: submit_response.content_id.clone(),
                    expected_revision: submit_response.revision,
                    version: ContentVersion::new("1.0.0").expect("fixture version should be valid"),
                    approved_gate_ref: ApprovedGateRef {
                        gate_id: format!("gate-{suffix}"),
                        gate_decision_id: format!("decision-{suffix}"),
                        approved_at: datetime!(2026-05-27 05:05:00 UTC),
                    },
                    publish_reason: format!("Publish fixture {suffix}"),
                },
                sample_actor(),
                command_meta("publish", &meta_suffix),
            )
            .await
            .expect("publish should succeed");

        self.build_published_fixture(name, publish_response)
    }

    fn build_published_fixture(
        &self,
        name: String,
        response: PublishMethodContentResponse,
    ) -> PublishedFixture {
        let stored = self
            .content_repository
            .contents()
            .expect("contents should be readable")
            .get(&response.content_id)
            .cloned()
            .expect("published content should exist");

        PublishedFixture {
            content_id: response.content_id.clone(),
            kind: response.kind,
            name,
            content_ref: PublishedContentRef {
                content_id: response.content_id,
                kind: response.kind,
                version: stored
                    .version
                    .expect("published content should retain its version"),
                fingerprint: stored
                    .fingerprint
                    .expect("published content should retain its fingerprint"),
            },
            snapshot_id: response.snapshot_ref.snapshot_id,
        }
    }
}

#[must_use]
pub fn sample_actor() -> ActorContext {
    ActorContext {
        actor_id: "actor-contract".to_string(),
        actor_kind: ActorKind::Human,
        actor_ref: ActorRef {
            actor_id: "actor-contract".to_string(),
            actor_kind: ActorKind::Human,
        },
    }
}

#[must_use]
pub fn worker_actor() -> ActorContext {
    ActorContext {
        actor_id: "worker-contract".to_string(),
        actor_kind: ActorKind::System,
        actor_ref: ActorRef {
            actor_id: "worker-contract".to_string(),
            actor_kind: ActorKind::System,
        },
    }
}

#[must_use]
pub fn command_meta(operation: &str, suffix: &str) -> RequestMeta {
    RequestMeta {
        request_id: format!("req-{operation}-{suffix}"),
        trace_id: "trace-contract".to_string(),
        idempotency_key: Some(format!("idem-{operation}-{suffix}")),
        request_hash: format!("hash-{operation}-{suffix}"),
        received_at: datetime!(2026-05-27 05:00:00 UTC),
    }
}

#[must_use]
pub fn relay_meta(worker_id: &str) -> RequestMeta {
    RequestMeta {
        request_id: format!("req-relay-{worker_id}"),
        trace_id: "trace-contract".to_string(),
        idempotency_key: None,
        request_hash: format!("hash-relay-{worker_id}"),
        received_at: datetime!(2026-05-27 05:00:00 UTC),
    }
}

#[must_use]
pub fn read_meta(suffix: &str) -> RequestMeta {
    RequestMeta {
        request_id: format!("req-read-{suffix}"),
        trace_id: "trace-contract".to_string(),
        idempotency_key: None,
        request_hash: format!("hash-read-{suffix}"),
        received_at: datetime!(2026-05-27 05:00:00 UTC),
    }
}

#[must_use]
pub fn job_meta(suffix: &str) -> RequestMeta {
    RequestMeta {
        request_id: format!("req-job-{suffix}"),
        trace_id: "trace-contract".to_string(),
        idempotency_key: Some(format!("idem-job-{suffix}")),
        request_hash: format!("hash-job-{suffix}"),
        received_at: datetime!(2026-05-27 05:00:00 UTC),
    }
}
