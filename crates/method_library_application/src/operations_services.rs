//! Application services for operations jobs and projection rebuild flows.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use method_library_contracts::{
    ActorContext, ContentSummaryView, ContentVersionView, CreateMethodContentDraftCommand,
    DefinitionTraceView, FingerprintMismatch, JobItemFailure, ProjectionCheckpointView,
    PublishMethodContentCommand, RebuildReadModelsJobRequest, RebuildReadModelsJobResult,
    RecalculateFingerprintJobRequest, RecalculateFingerprintJobResult,
    ReplayDefinitionEventsJobRequest, ReplayDefinitionEventsJobResult, RequestMeta,
    SeedInitialMethodAssetsJobRequest, SeedInitialMethodAssetsJobResult,
    SubmitMethodContentForReviewCommand,
};
use method_library_domain::content::{
    ActorKind, ApprovedGateRef, CanonicalFingerprint, ContentVersion, FingerprintAlgorithm,
    LifecycleState, MethodContent, MethodContentKind, PublishedContentRef,
};
use method_library_domain::definitions::{
    AIPolicyConstraint, AIPolicyDef, EvidenceKind, EvidenceRule, MethodContentPayload,
    PolicySeverity, ProcessTemplateDef, Qualification, QualificationLevel, QualificationLevelModel,
    RoleDefinition, TailoringPoint, ViewObjectKind, ViewProfile,
};
use method_library_domain::policies::FingerprintPolicy;
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use crate::ports::{
    AuditRepository, BusPublisherPort, Clock, ContentSummaryProjectionRepository, FailureReason,
    FingerprintHasher, JobRunRecord, JobRunRepository, JobRunStartResult, JobRunStatus,
    LifecycleHistoryRepository, MethodContentRepository, MethodContentVersionRepository,
    OutboxRepository, ProjectionCheckpointRepository, SupersedeLinkRepository, UnitOfWork,
};
use crate::services::MethodContentCommandService;

const SEED_JOB_NAME: &str = "seed_initial_method_assets";
const REPLAY_JOB_NAME: &str = "replay_definition_events";
const RECALCULATE_JOB_NAME: &str = "recalculate_fingerprint";
const REBUILD_JOB_NAME: &str = "rebuild_read_models";

const BASELINE_ASSET_SET: &str = "baseline";
const DEFAULT_PUBLISHED_VERSION: &str = "1.0.0";
const CANONICAL_SCHEMA_VERSION: &str = "1.0";

const CONTENT_SUMMARY_PROJECTION: &str = "content_summary_projection";
const DEFINITION_TRACE_PROJECTION: &str = "definition_trace_projection";
const REPLAY_CHECKPOINT_PREFIX: &str = "replay_consumer";

/// Application service for P0 operations jobs.
#[derive(Clone)]
pub struct MethodOperationsService {
    unit_of_work: Arc<dyn UnitOfWork>,
    command_service: Arc<MethodContentCommandService>,
    method_content_repository: Arc<dyn MethodContentRepository>,
    method_content_version_repository: Arc<dyn MethodContentVersionRepository>,
    lifecycle_history_repository: Arc<dyn LifecycleHistoryRepository>,
    audit_repository: Arc<dyn AuditRepository>,
    supersede_link_repository: Arc<dyn SupersedeLinkRepository>,
    outbox_repository: Arc<dyn OutboxRepository>,
    content_summary_projection_repository: Arc<dyn ContentSummaryProjectionRepository>,
    definition_trace_projection_repository:
        Arc<dyn crate::ports::DefinitionTraceProjectionRepository>,
    projection_checkpoint_repository: Arc<dyn ProjectionCheckpointRepository>,
    definition_snapshot_repository: Arc<dyn crate::ports::DefinitionSnapshotRepository>,
    job_run_repository: Arc<dyn JobRunRepository>,
    bus_publisher: Arc<dyn BusPublisherPort>,
    fingerprint_hasher: Arc<dyn FingerprintHasher>,
    clock: Arc<dyn Clock>,
}

impl MethodOperationsService {
    /// Creates an operations service from port implementations.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        unit_of_work: Arc<dyn UnitOfWork>,
        command_service: Arc<MethodContentCommandService>,
        method_content_repository: Arc<dyn MethodContentRepository>,
        method_content_version_repository: Arc<dyn MethodContentVersionRepository>,
        lifecycle_history_repository: Arc<dyn LifecycleHistoryRepository>,
        audit_repository: Arc<dyn AuditRepository>,
        supersede_link_repository: Arc<dyn SupersedeLinkRepository>,
        outbox_repository: Arc<dyn OutboxRepository>,
        content_summary_projection_repository: Arc<dyn ContentSummaryProjectionRepository>,
        definition_trace_projection_repository: Arc<
            dyn crate::ports::DefinitionTraceProjectionRepository,
        >,
        projection_checkpoint_repository: Arc<dyn ProjectionCheckpointRepository>,
        definition_snapshot_repository: Arc<dyn crate::ports::DefinitionSnapshotRepository>,
        job_run_repository: Arc<dyn JobRunRepository>,
        bus_publisher: Arc<dyn BusPublisherPort>,
        fingerprint_hasher: Arc<dyn FingerprintHasher>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            unit_of_work,
            command_service,
            method_content_repository,
            method_content_version_repository,
            lifecycle_history_repository,
            audit_repository,
            supersede_link_repository,
            outbox_repository,
            content_summary_projection_repository,
            definition_trace_projection_repository,
            projection_checkpoint_repository,
            definition_snapshot_repository,
            job_run_repository,
            bus_publisher,
            fingerprint_hasher,
            clock,
        }
    }

    /// Seeds the baseline method-library assets by reusing the command service.
    pub async fn seed_initial_method_assets(
        &self,
        request: SeedInitialMethodAssetsJobRequest,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<SeedInitialMethodAssetsJobResult, MethodLibraryError> {
        validate_seed_request(&request)?;
        let scope_hash = hash_scope(&request)?;
        let now = self.clock.now();
        let start = self
            .start_job(SEED_JOB_NAME.to_string(), scope_hash, meta.clone(), now)
            .await?;
        let job_run_id = match start {
            StartedJob::New(job_run_id) => job_run_id,
            StartedJob::Existing(result) => return Ok(result),
        };

        let operation = self
            .run_seed_job(&request, &job_run_id, &actor, &meta)
            .await;
        self.finish_job(SEED_JOB_NAME, job_run_id, operation, &meta)
            .await
    }

    /// Replays historical definition events to one downstream consumer.
    pub async fn replay_definition_events(
        &self,
        request: ReplayDefinitionEventsJobRequest,
        _actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<ReplayDefinitionEventsJobResult, MethodLibraryError> {
        validate_replay_request(&request)?;
        let scope_hash = hash_scope(&request)?;
        let now = self.clock.now();
        let start = self
            .start_job(REPLAY_JOB_NAME.to_string(), scope_hash, meta.clone(), now)
            .await?;
        let job_run_id = match start {
            StartedJob::New(job_run_id) => job_run_id,
            StartedJob::Existing(result) => return Ok(result),
        };

        let operation = self
            .run_replay_job(request, job_run_id.clone(), meta.clone())
            .await;
        self.finish_job(REPLAY_JOB_NAME, job_run_id, operation, &meta)
            .await
    }

    /// Recalculates fingerprints and reports published mismatches without mutating truth.
    pub async fn recalculate_fingerprint(
        &self,
        request: RecalculateFingerprintJobRequest,
        _actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<RecalculateFingerprintJobResult, MethodLibraryError> {
        validate_recalculate_request(&request)?;
        let scope_hash = hash_scope(&request)?;
        let now = self.clock.now();
        let start = self
            .start_job(
                RECALCULATE_JOB_NAME.to_string(),
                scope_hash,
                meta.clone(),
                now,
            )
            .await?;
        let job_run_id = match start {
            StartedJob::New(job_run_id) => job_run_id,
            StartedJob::Existing(result) => return Ok(result),
        };

        let operation = self.run_recalculate_job(request, job_run_id.clone()).await;
        self.finish_job(RECALCULATE_JOB_NAME, job_run_id, operation, &meta)
            .await
    }

    /// Rebuilds read-model projections from persisted truth and historical outbox events.
    pub async fn rebuild_read_models(
        &self,
        request: RebuildReadModelsJobRequest,
        _actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<RebuildReadModelsJobResult, MethodLibraryError> {
        let projection_names = normalize_projection_names(&request.projection_names)?;
        validate_rebuild_request(&request)?;
        let scope_hash = hash_scope(&request)?;
        let now = self.clock.now();
        let start = self
            .start_job(REBUILD_JOB_NAME.to_string(), scope_hash, meta.clone(), now)
            .await?;
        let job_run_id = match start {
            StartedJob::New(job_run_id) => job_run_id,
            StartedJob::Existing(result) => return Ok(result),
        };

        let operation = self
            .run_rebuild_job(request, projection_names, job_run_id.clone())
            .await;
        self.finish_job(REBUILD_JOB_NAME, job_run_id, operation, &meta)
            .await
    }

    async fn run_seed_job(
        &self,
        request: &SeedInitialMethodAssetsJobRequest,
        job_run_id: &str,
        actor: &ActorContext,
        meta: &RequestMeta,
    ) -> Result<SeedInitialMethodAssetsJobResult, MethodLibraryError> {
        let mut result = SeedInitialMethodAssetsJobResult {
            job_run_id: job_run_id.to_string(),
            created_count: 0,
            published_count: 0,
            skipped_count: 0,
            failures: Vec::new(),
        };
        let publish_version =
            ContentVersion::new(DEFAULT_PUBLISHED_VERSION).expect("seed version is valid");
        let existing = self.method_content_repository.list_all().await?;
        let mut existing_by_key = existing
            .into_iter()
            .map(|content| ((content.kind, content.name.clone()), content))
            .collect::<HashMap<_, _>>();
        let mut role_refs = HashMap::new();

        for (role_key, role_name, responsibility) in baseline_role_specs() {
            if !request.kinds.is_empty()
                && !request.kinds.contains(&MethodContentKind::RoleDefinition)
            {
                break;
            }

            let command = CreateMethodContentDraftCommand {
                kind: MethodContentKind::RoleDefinition,
                name: role_name.to_string(),
                description: Some(format!("Baseline role seed for {role_name}")),
                payload: MethodContentPayload::RoleDefinition(RoleDefinition {
                    role_key: role_key.to_string(),
                    responsibilities: vec![responsibility.to_string()],
                    qualification_refs: Vec::new(),
                    default_view_profile_refs: Vec::new(),
                }),
                references: Vec::new(),
                source_refs: Vec::new(),
            };

            match self
                .apply_seed_asset(
                    &SeedAssetPlan {
                        asset_key: format!("role_definition:{role_key}"),
                        kind: MethodContentKind::RoleDefinition,
                        name: role_name.to_string(),
                        create_command: command,
                    },
                    request.publish,
                    request.dry_run,
                    request.asset_set.clone(),
                    publish_version.clone(),
                    job_run_id.to_string(),
                    actor.clone(),
                    meta.clone(),
                    &mut existing_by_key,
                    &mut result,
                )
                .await
            {
                Ok(Some(role_ref)) => {
                    role_refs.insert(role_key.to_string(), role_ref);
                }
                Ok(None) => {}
                Err(error) => result.failures.push(job_item_failure(
                    &format!("role_definition:{role_key}"),
                    error,
                )),
            }
        }

        if request.kinds.is_empty() || request.kinds.contains(&MethodContentKind::Qualification) {
            let command = CreateMethodContentDraftCommand {
                kind: MethodContentKind::Qualification,
                name: "Baseline Delivery Qualification".to_string(),
                description: Some(
                    "Baseline qualification for method-library seed data".to_string(),
                ),
                payload: MethodContentPayload::Qualification(Qualification {
                    qualification_key: "baseline_delivery".to_string(),
                    name: "Baseline Delivery Qualification".to_string(),
                    description: Some(
                        "Baseline qualification used by the seed method library".to_string(),
                    ),
                    level_model: QualificationLevelModel {
                        levels: vec![QualificationLevel {
                            level_key: "baseline".to_string(),
                            name: "Baseline".to_string(),
                            order: 1,
                            description: Some("Baseline delivery competence".to_string()),
                        }],
                        default_level_key: Some("baseline".to_string()),
                    },
                    evidence_rules: vec![EvidenceRule {
                        evidence_kind: EvidenceKind::Document,
                        required: true,
                        description: "Reference delivery evidence".to_string(),
                    }],
                }),
                references: Vec::new(),
                source_refs: Vec::new(),
            };

            if let Err(error) = self
                .apply_seed_asset(
                    &SeedAssetPlan {
                        asset_key: "qualification:baseline_delivery".to_string(),
                        kind: MethodContentKind::Qualification,
                        name: "Baseline Delivery Qualification".to_string(),
                        create_command: command,
                    },
                    request.publish,
                    request.dry_run,
                    request.asset_set.clone(),
                    publish_version.clone(),
                    job_run_id.to_string(),
                    actor.clone(),
                    meta.clone(),
                    &mut existing_by_key,
                    &mut result,
                )
                .await
            {
                result
                    .failures
                    .push(job_item_failure("qualification:baseline_delivery", error));
            }
        }

        if request.kinds.is_empty()
            || request
                .kinds
                .contains(&MethodContentKind::ProcessTemplateDef)
        {
            for (template_key, template_name) in baseline_process_template_specs() {
                let command = CreateMethodContentDraftCommand {
                    kind: MethodContentKind::ProcessTemplateDef,
                    name: template_name.to_string(),
                    description: Some(format!(
                        "Baseline process template seed for {template_name}"
                    )),
                    payload: MethodContentPayload::ProcessTemplateDef(ProcessTemplateDef {
                        template_key: template_key.to_string(),
                        activity_defs: Vec::new(),
                        work_product_refs: Vec::new(),
                        role_refs: Vec::new(),
                        tailoring_points: Vec::<TailoringPoint>::new(),
                    }),
                    references: Vec::new(),
                    source_refs: Vec::new(),
                };

                if let Err(error) = self
                    .apply_seed_asset(
                        &SeedAssetPlan {
                            asset_key: format!("process_template:{template_key}"),
                            kind: MethodContentKind::ProcessTemplateDef,
                            name: template_name.to_string(),
                            create_command: command,
                        },
                        request.publish,
                        request.dry_run,
                        request.asset_set.clone(),
                        publish_version.clone(),
                        job_run_id.to_string(),
                        actor.clone(),
                        meta.clone(),
                        &mut existing_by_key,
                        &mut result,
                    )
                    .await
                {
                    result.failures.push(job_item_failure(
                        &format!("process_template:{template_key}"),
                        error,
                    ));
                }
            }
        }

        if request.kinds.is_empty() || request.kinds.contains(&MethodContentKind::ViewProfile) {
            for (role_key, role_name, _) in baseline_role_specs() {
                let Some(role_ref) = role_refs.get(role_key).cloned() else {
                    result.failures.push(JobItemFailure {
                        item_ref: format!("view_profile:{role_key}:definition"),
                        error_code: MethodLibraryErrorCode::ReferenceNotPublished,
                        message: format!(
                            "published role definition is required before seeding the {role_name} view profile"
                        ),
                    });
                    continue;
                };
                let command = CreateMethodContentDraftCommand {
                    kind: MethodContentKind::ViewProfile,
                    name: format!("{role_name} Definition View"),
                    description: Some(format!(
                        "Baseline definition view profile for the {role_name} role"
                    )),
                    payload: MethodContentPayload::ViewProfile(ViewProfile {
                        role_ref,
                        object_kind: ViewObjectKind::Definition,
                        scope_rules: Vec::new(),
                        field_rules: Vec::new(),
                        action_rules: Vec::new(),
                    }),
                    references: Vec::new(),
                    source_refs: Vec::new(),
                };

                if let Err(error) = self
                    .apply_seed_asset(
                        &SeedAssetPlan {
                            asset_key: format!("view_profile:{role_key}:definition"),
                            kind: MethodContentKind::ViewProfile,
                            name: format!("{role_name} Definition View"),
                            create_command: command,
                        },
                        request.publish,
                        request.dry_run,
                        request.asset_set.clone(),
                        publish_version.clone(),
                        job_run_id.to_string(),
                        actor.clone(),
                        meta.clone(),
                        &mut existing_by_key,
                        &mut result,
                    )
                    .await
                {
                    result.failures.push(job_item_failure(
                        &format!("view_profile:{role_key}:definition"),
                        error,
                    ));
                }
            }
        }

        if request.kinds.is_empty() || request.kinds.contains(&MethodContentKind::AIPolicyDef) {
            let command = CreateMethodContentDraftCommand {
                kind: MethodContentKind::AIPolicyDef,
                name: "Baseline AI Collaboration Policy".to_string(),
                description: Some("Baseline AI policy for seed method assets".to_string()),
                payload: MethodContentPayload::AIPolicyDef(AIPolicyDef {
                    policy_key: "baseline_ai_collaboration".to_string(),
                    target_actor_kind: ActorKind::AiMember,
                    rule_refs: Vec::new(),
                    constraints: vec![AIPolicyConstraint {
                        constraint_key: "cite_sources".to_string(),
                        description: "Cite relevant sources for generated outputs".to_string(),
                        severity: PolicySeverity::Advisory,
                    }],
                }),
                references: Vec::new(),
                source_refs: Vec::new(),
            };

            if let Err(error) = self
                .apply_seed_asset(
                    &SeedAssetPlan {
                        asset_key: "ai_policy:baseline_ai_collaboration".to_string(),
                        kind: MethodContentKind::AIPolicyDef,
                        name: "Baseline AI Collaboration Policy".to_string(),
                        create_command: command,
                    },
                    request.publish,
                    request.dry_run,
                    request.asset_set.clone(),
                    publish_version,
                    job_run_id.to_string(),
                    actor.clone(),
                    meta.clone(),
                    &mut existing_by_key,
                    &mut result,
                )
                .await
            {
                result.failures.push(job_item_failure(
                    "ai_policy:baseline_ai_collaboration",
                    error,
                ));
            }
        }

        Ok(result)
    }

    async fn run_replay_job(
        &self,
        request: ReplayDefinitionEventsJobRequest,
        job_run_id: String,
        meta: RequestMeta,
    ) -> Result<ReplayDefinitionEventsJobResult, MethodLibraryError> {
        let checkpoint_name = replay_checkpoint_name(&request.consumer);
        let expected_cursor = match request.from_cursor.clone() {
            Some(cursor) => Some(cursor),
            None => self
                .projection_checkpoint_repository
                .get(checkpoint_name.clone())
                .await?
                .and_then(|record| record.last_processed_event_id),
        };
        let events = self
            .outbox_repository
            .list_replayable(
                expected_cursor.clone(),
                request.event_types.clone(),
                request.batch_size,
            )
            .await?;
        let mut result = ReplayDefinitionEventsJobResult {
            job_run_id,
            replayed_count: 0,
            next_cursor: expected_cursor.clone(),
            failures: Vec::new(),
        };

        if request.dry_run {
            result.replayed_count = events.len() as u32;
            result.next_cursor = events
                .last()
                .map(|event| event.event_id.clone())
                .or(expected_cursor);
            return Ok(result);
        }

        let mut successful_cursor = expected_cursor.clone();
        for event in events {
            match self
                .bus_publisher
                .publish(
                    replay_topic_for_event_type(event.envelope.event_type),
                    event.envelope.clone(),
                    meta.clone(),
                )
                .await
            {
                Ok(_) => {
                    result.replayed_count += 1;
                    successful_cursor = Some(event.event_id);
                }
                Err(error) => {
                    result
                        .failures
                        .push(job_item_failure(&event.event_id, error));
                    break;
                }
            }
        }

        if let Some(next_cursor) = successful_cursor.clone()
            && successful_cursor != expected_cursor
        {
            self.projection_checkpoint_repository
                .advance_if_current(
                    checkpoint_name,
                    expected_cursor.clone(),
                    next_cursor.clone(),
                    self.clock.now(),
                )
                .await?;
            result.next_cursor = Some(next_cursor);
        }

        Ok(result)
    }

    async fn run_recalculate_job(
        &self,
        request: RecalculateFingerprintJobRequest,
        job_run_id: String,
    ) -> Result<RecalculateFingerprintJobResult, MethodLibraryError> {
        let mut contents = if request.content_ids.is_empty() {
            self.method_content_repository.list_all().await?
        } else {
            let mut items = Vec::with_capacity(request.content_ids.len());
            for content_id in &request.content_ids {
                let content = self
                    .method_content_repository
                    .get(content_id.clone())
                    .await?
                    .ok_or_else(|| {
                        MethodLibraryError::validation(
                            MethodLibraryErrorCode::MethodContentNotFound,
                            "method content does not exist",
                        )
                        .with_detail("content_id", content_id.clone())
                    })?;
                items.push(content);
            }
            items
        };

        if let Some(kind) = request.kind {
            contents.retain(|content| content.kind == kind);
        }
        contents.retain(|content| content.is_published_like() && content.fingerprint.is_some());

        let mut mismatches = Vec::new();
        for content in &contents {
            let canonical_bytes =
                FingerprintPolicy::canonicalize(content, request.canonical_schema_version.clone())?;
            let recalculated = self.fingerprint_hasher.hash_canonical_bytes(
                canonical_bytes,
                FingerprintAlgorithm::Sha256,
                request.canonical_schema_version.clone(),
            )?;
            let stored = content
                .fingerprint
                .clone()
                .expect("published content should carry a fingerprint");
            if stored != recalculated {
                mismatches.push(FingerprintMismatch {
                    content_id: content.content_id.clone(),
                    kind: content.kind,
                    stored_fingerprint: stored,
                    recalculated_fingerprint: recalculated,
                });
            }
        }

        Ok(RecalculateFingerprintJobResult {
            job_run_id,
            checked_count: contents.len() as u32,
            mismatches,
        })
    }

    async fn run_rebuild_job(
        &self,
        request: RebuildReadModelsJobRequest,
        projection_names: Vec<String>,
        job_run_id: String,
    ) -> Result<RebuildReadModelsJobResult, MethodLibraryError> {
        let contents = self.method_content_repository.list_all().await?;
        let now = self.clock.now();
        let mut current_checkpoints = HashMap::new();
        for projection_name in &projection_names {
            let checkpoint = self
                .projection_checkpoint_repository
                .get(projection_name.clone())
                .await?
                .and_then(|record| record.last_processed_event_id);
            current_checkpoints.insert(projection_name.clone(), checkpoint);
        }
        let outbox_events = self
            .outbox_repository
            .list_replayable(request.from_cursor.clone(), Vec::new(), request.batch_size)
            .await?;

        for content in &contents {
            if projection_names
                .iter()
                .any(|name| name == CONTENT_SUMMARY_PROJECTION)
            {
                let view = ContentSummaryView {
                    content_id: content.content_id.clone(),
                    kind: content.kind,
                    name: content.name.clone(),
                    lifecycle_state: content.lifecycle.state,
                    version: content.version.clone(),
                    updated_at: content.updated_at,
                };
                if !request.dry_run {
                    self.content_summary_projection_repository
                        .upsert(view)
                        .await?;
                }
            }

            if projection_names
                .iter()
                .any(|name| name == DEFINITION_TRACE_PROJECTION)
            {
                let trace = self.build_definition_trace_view(content).await?;
                if !request.dry_run {
                    self.definition_trace_projection_repository
                        .upsert(trace)
                        .await?;
                }
            }
        }

        let checkpoint = if request.dry_run {
            None
        } else if let Some(next_cursor) = outbox_events.last().map(|event| event.event_id.clone()) {
            let mut updated_projection_names = Vec::with_capacity(projection_names.len());
            for projection_name in &projection_names {
                let expected_cursor = request
                    .from_cursor
                    .clone()
                    .or_else(|| current_checkpoints.get(projection_name).cloned().flatten());
                self.projection_checkpoint_repository
                    .advance_if_current(
                        projection_name.clone(),
                        expected_cursor,
                        next_cursor.clone(),
                        now,
                    )
                    .await?;
                updated_projection_names.push(projection_name.clone());
            }

            Some(ProjectionCheckpointView {
                projection_name: updated_projection_names.join(","),
                last_processed_event_id: Some(next_cursor),
                updated_at: now,
            })
        } else {
            None
        };

        Ok(RebuildReadModelsJobResult {
            job_run_id,
            processed_count: contents.len() as u32,
            checkpoint,
            failures: Vec::new(),
        })
    }

    async fn build_definition_trace_view(
        &self,
        content: &MethodContent,
    ) -> Result<DefinitionTraceView, MethodLibraryError> {
        let mut versions = Vec::new();
        for record in self
            .method_content_version_repository
            .list_by_content(content.content_id.clone())
            .await?
        {
            let snapshot_ref = self
                .definition_snapshot_repository
                .get(record.snapshot_id.clone())
                .await?
                .map(|snapshot| snapshot.snapshot_ref());
            versions.push(ContentVersionView {
                content_id: record.content_id,
                content_family_id: record.content_family_id,
                version: record.version,
                fingerprint: record.fingerprint,
                snapshot_ref,
                published_at: record.published_at,
            });
        }

        let lifecycle_history = self
            .lifecycle_history_repository
            .list_by_content(content.content_id.clone())
            .await?
            .into_iter()
            .map(
                |entry| method_library_contracts::LifecycleHistoryEntryView {
                    from_state: entry.from_state,
                    to_state: entry.to_state,
                    actor_id: entry.actor_id,
                    reason: entry.reason,
                    created_at: entry.created_at,
                },
            )
            .collect();

        let audit_records = self
            .audit_repository
            .list_by_content(content.content_id.clone())
            .await?
            .into_iter()
            .map(|record| method_library_contracts::AuditRecordView {
                request_id: record.request_id,
                trace_id: record.trace_id,
                actor_ref: record.actor_context.actor_ref,
                action: record.action,
                result: record.result,
                occurred_at: record.occurred_at,
            })
            .collect();

        let outbox_events = self
            .outbox_repository
            .list_by_aggregate(content.content_id.clone())
            .await?
            .into_iter()
            .map(|event| method_library_contracts::OutboxEventView {
                event_id: event.event_id,
                event_type: event.envelope.event_type,
                occurred_at: event.envelope.occurred_at,
                snapshot_ref: event.envelope.snapshot_ref,
            })
            .collect();

        let supersede_chain = self
            .supersede_link_repository
            .list_by_content(content.content_id.clone())
            .await?
            .into_iter()
            .map(|link| method_library_contracts::SupersedeLinkView {
                old_content_id: link.old_content_id,
                new_content_id: link.new_content_id,
                reason: link.reason,
                created_at: link.created_at,
            })
            .collect();

        Ok(DefinitionTraceView {
            content_id: content.content_id.clone(),
            versions,
            lifecycle_history,
            audit_records,
            outbox_events,
            supersede_chain,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn apply_seed_asset(
        &self,
        plan: &SeedAssetPlan,
        publish: bool,
        dry_run: bool,
        asset_set: String,
        publish_version: ContentVersion,
        job_run_id: String,
        actor: ActorContext,
        meta: RequestMeta,
        existing_by_key: &mut HashMap<(MethodContentKind, String), MethodContent>,
        result: &mut SeedInitialMethodAssetsJobResult,
    ) -> Result<Option<PublishedContentRef>, MethodLibraryError> {
        let composite_key = (plan.kind, plan.name.clone());
        let existing = existing_by_key.get(&composite_key).cloned();

        if let Some(content) = existing {
            match content.lifecycle.state {
                LifecycleState::Published | LifecycleState::Deprecated => {
                    result.skipped_count += 1;
                    return published_content_ref(&content).map(Some);
                }
                LifecycleState::Retired | LifecycleState::Superseded => {
                    return Err(MethodLibraryError::validation(
                        MethodLibraryErrorCode::LifecycleTransitionNotAllowed,
                        "seed assets cannot reuse retired or superseded content",
                    )
                    .with_detail("content_id", content.content_id)
                    .with_detail("lifecycle_state", content.lifecycle.state.as_str()));
                }
                LifecycleState::Draft | LifecycleState::InReview => {
                    if !publish {
                        result.skipped_count += 1;
                        return Ok(None);
                    }

                    if dry_run {
                        result.published_count += 1;
                        return Ok(Some(synthetic_published_ref(plan.kind, &plan.asset_key)));
                    }

                    let final_content = self
                        .publish_seed_content(
                            content,
                            &plan.asset_key,
                            &asset_set,
                            &publish_version,
                            &job_run_id,
                            &actor,
                            &meta,
                        )
                        .await?;
                    result.published_count += 1;
                    existing_by_key.insert(composite_key, final_content.clone());
                    return published_content_ref(&final_content).map(Some);
                }
            }
        }

        if dry_run {
            result.created_count += 1;
            if publish {
                result.published_count += 1;
                return Ok(Some(synthetic_published_ref(plan.kind, &plan.asset_key)));
            }
            return Ok(None);
        }

        let create_meta = command_meta(
            meta.clone(),
            format!("{}:create", plan.asset_key),
            &plan.create_command,
        )?;
        let created = self
            .command_service
            .create_draft(plan.create_command.clone(), actor.clone(), create_meta)
            .await?;
        result.created_count += 1;

        let mut content = self
            .method_content_repository
            .get(created.content_id.clone())
            .await?
            .ok_or_else(|| {
                MethodLibraryError::retryable(
                    MethodLibraryErrorCode::PersistenceUnavailable,
                    "created seed content could not be reloaded",
                )
            })?;

        if !publish {
            existing_by_key.insert(composite_key, content);
            return Ok(None);
        }

        content = self
            .publish_seed_content(
                content,
                &plan.asset_key,
                &asset_set,
                &publish_version,
                &job_run_id,
                &actor,
                &meta,
            )
            .await?;
        result.published_count += 1;
        existing_by_key.insert(composite_key, content.clone());
        published_content_ref(&content).map(Some)
    }

    async fn publish_seed_content(
        &self,
        mut content: MethodContent,
        asset_key: &str,
        asset_set: &str,
        publish_version: &ContentVersion,
        job_run_id: &str,
        actor: &ActorContext,
        meta: &RequestMeta,
    ) -> Result<MethodContent, MethodLibraryError> {
        if content.lifecycle.state == LifecycleState::Draft {
            let submit_command = SubmitMethodContentForReviewCommand {
                content_id: content.content_id.clone(),
                expected_revision: content.revision,
                review_reason: Some(format!("Seed review for {asset_key}")),
                review_evidence_refs: Vec::new(),
            };
            let submit_meta = command_meta(
                meta.clone(),
                format!("{asset_key}:submit_for_review"),
                &submit_command,
            )?;
            let submitted = self
                .command_service
                .submit_for_review(submit_command, actor.clone(), submit_meta)
                .await?;
            content = self
                .method_content_repository
                .get(submitted.content_id)
                .await?
                .ok_or_else(|| {
                    MethodLibraryError::retryable(
                        MethodLibraryErrorCode::PersistenceUnavailable,
                        "submitted seed content could not be reloaded",
                    )
                })?;
        }

        if content.lifecycle.state == LifecycleState::InReview {
            let publish_command = PublishMethodContentCommand {
                content_id: content.content_id.clone(),
                expected_revision: content.revision,
                version: publish_version.clone(),
                approved_gate_ref: ApprovedGateRef {
                    gate_id: format!("seed/{asset_set}"),
                    gate_decision_id: format!("{job_run_id}:{asset_key}"),
                    approved_at: self.clock.now(),
                },
                publish_reason: format!("Seeded from asset set {asset_set}"),
            };
            let publish_meta = command_meta(
                meta.clone(),
                format!("{asset_key}:publish"),
                &publish_command,
            )?;
            let published = self
                .command_service
                .publish(publish_command, actor.clone(), publish_meta)
                .await?;
            content = self
                .method_content_repository
                .get(published.content_id)
                .await?
                .ok_or_else(|| {
                    MethodLibraryError::retryable(
                        MethodLibraryErrorCode::PersistenceUnavailable,
                        "published seed content could not be reloaded",
                    )
                })?;
        }

        if !content.is_published_like() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::LifecycleTransitionNotAllowed,
                "seed content did not reach a published lifecycle state",
            )
            .with_detail("content_id", content.content_id)
            .with_detail("lifecycle_state", content.lifecycle.state.as_str()));
        }

        Ok(content)
    }

    async fn start_job<T: DeserializeOwned>(
        &self,
        job_name: String,
        scope_hash: String,
        meta: RequestMeta,
        now: method_library_domain::content::Timestamp,
    ) -> Result<StartedJob<T>, MethodLibraryError> {
        let idempotency_key = meta.require_idempotency_key()?.clone();
        let mut tx = self.unit_of_work.begin(meta.clone()).await?;
        let outcome = self
            .job_run_repository
            .start_once(&mut tx, job_name, scope_hash, idempotency_key, now)
            .await;

        match outcome {
            Ok(started) => {
                tx.commit().await?;
                match started {
                    JobRunStartResult::Started(job_run_id) => Ok(StartedJob::New(job_run_id)),
                    JobRunStartResult::AlreadyRunning(job_run_id) => {
                        Err(MethodLibraryError::validation(
                            MethodLibraryErrorCode::JobStatusConflict,
                            "operations job is already running",
                        )
                        .with_detail("job_run_id", job_run_id))
                    }
                    JobRunStartResult::Existing(record) => {
                        Ok(StartedJob::Existing(decode_existing_job_result(record)?))
                    }
                }
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    async fn finish_job<T>(
        &self,
        job_name: &str,
        job_run_id: String,
        operation: Result<T, MethodLibraryError>,
        meta: &RequestMeta,
    ) -> Result<T, MethodLibraryError>
    where
        T: Serialize + HasPartialFailures,
    {
        match operation {
            Ok(result) => {
                let job_result = serde_json::to_value(&result).map_err(|error| {
                    MethodLibraryError::retryable(
                        MethodLibraryErrorCode::PersistenceUnavailable,
                        format!("failed to serialize {job_name} result: {error}"),
                    )
                })?;
                let mut tx = self.unit_of_work.begin(meta.clone()).await?;
                let completion = if result.has_partial_failures() {
                    self.job_run_repository
                        .complete_with_partial_failure(
                            &mut tx,
                            job_run_id,
                            job_result,
                            self.clock.now(),
                        )
                        .await
                } else {
                    self.job_run_repository
                        .complete(&mut tx, job_run_id, job_result, self.clock.now())
                        .await
                };
                match completion {
                    Ok(()) => {
                        tx.commit().await?;
                        Ok(result)
                    }
                    Err(error) => {
                        let _ = tx.rollback().await;
                        Err(error)
                    }
                }
            }
            Err(error) => {
                let reason = FailureReason::from_error(&error);
                let mut tx = self.unit_of_work.begin(meta.clone()).await?;
                let failed = self
                    .job_run_repository
                    .fail(&mut tx, job_run_id, reason, self.clock.now())
                    .await;
                match failed {
                    Ok(()) => {
                        tx.commit().await?;
                        Err(error)
                    }
                    Err(mark_error) => {
                        let _ = tx.rollback().await;
                        Err(mark_error)
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct SeedAssetPlan {
    asset_key: String,
    kind: MethodContentKind,
    name: String,
    create_command: CreateMethodContentDraftCommand,
}

enum StartedJob<T> {
    New(String),
    Existing(T),
}

trait HasPartialFailures {
    fn has_partial_failures(&self) -> bool;
}

impl HasPartialFailures for SeedInitialMethodAssetsJobResult {
    fn has_partial_failures(&self) -> bool {
        !self.failures.is_empty()
    }
}

impl HasPartialFailures for ReplayDefinitionEventsJobResult {
    fn has_partial_failures(&self) -> bool {
        !self.failures.is_empty()
    }
}

impl HasPartialFailures for RecalculateFingerprintJobResult {
    fn has_partial_failures(&self) -> bool {
        false
    }
}

impl HasPartialFailures for RebuildReadModelsJobResult {
    fn has_partial_failures(&self) -> bool {
        !self.failures.is_empty()
    }
}

fn validate_seed_request(
    request: &SeedInitialMethodAssetsJobRequest,
) -> Result<(), MethodLibraryError> {
    if request.asset_set.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "seed requests require a non-empty asset set",
        ));
    }
    if request.asset_set != BASELINE_ASSET_SET {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "only the baseline asset set is supported in P0",
        )
        .with_detail("asset_set", request.asset_set.clone()));
    }
    ensure_supported_seed_kinds(&request.kinds)
}

fn validate_replay_request(
    request: &ReplayDefinitionEventsJobRequest,
) -> Result<(), MethodLibraryError> {
    if request.consumer.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "replay requests require a consumer identifier",
        ));
    }
    if request.batch_size == 0 {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "replay requests require a positive batch size",
        ));
    }
    Ok(())
}

fn validate_recalculate_request(
    request: &RecalculateFingerprintJobRequest,
) -> Result<(), MethodLibraryError> {
    if request.canonical_schema_version.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "recalculate requests require a canonical schema version",
        ));
    }
    Ok(())
}

fn validate_rebuild_request(
    request: &RebuildReadModelsJobRequest,
) -> Result<(), MethodLibraryError> {
    if request.batch_size == 0 {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "rebuild requests require a positive batch size",
        ));
    }
    Ok(())
}

fn normalize_projection_names(
    projection_names: &[String],
) -> Result<Vec<String>, MethodLibraryError> {
    let defaults = vec![
        CONTENT_SUMMARY_PROJECTION.to_string(),
        DEFINITION_TRACE_PROJECTION.to_string(),
    ];
    if projection_names.is_empty() {
        return Ok(defaults);
    }

    let supported = [CONTENT_SUMMARY_PROJECTION, DEFINITION_TRACE_PROJECTION]
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for name in projection_names {
        if !supported.contains(name.as_str()) {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::JobRequestInvalid,
                "rebuild requests contain an unknown projection name",
            )
            .with_detail("projection_name", name.clone()));
        }
        if seen.insert(name.clone()) {
            names.push(name.clone());
        }
    }

    Ok(names)
}

fn ensure_supported_seed_kinds(kinds: &[MethodContentKind]) -> Result<(), MethodLibraryError> {
    let supported = [
        MethodContentKind::Qualification,
        MethodContentKind::RoleDefinition,
        MethodContentKind::ProcessTemplateDef,
        MethodContentKind::ViewProfile,
        MethodContentKind::AIPolicyDef,
    ];

    for kind in kinds {
        if !supported.contains(kind) {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::JobRequestInvalid,
                "seed requests contain a kind outside the P0 baseline seed set",
            )
            .with_detail("kind", kind.as_str()));
        }
    }

    Ok(())
}

fn job_item_failure(item_ref: &str, error: MethodLibraryError) -> JobItemFailure {
    JobItemFailure {
        item_ref: item_ref.to_string(),
        error_code: error.code,
        message: error.message,
    }
}

fn hash_scope<T: Serialize>(value: &T) -> Result<String, MethodLibraryError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            format!("job request could not be canonicalized: {error}"),
        )
    })?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

fn command_meta<T: Serialize>(
    parent_meta: RequestMeta,
    suffix: String,
    command: &T,
) -> Result<RequestMeta, MethodLibraryError> {
    Ok(RequestMeta {
        request_id: format!("{}:{suffix}", parent_meta.request_id),
        trace_id: parent_meta.trace_id,
        idempotency_key: Some(format!("seed:{suffix}")),
        request_hash: hash_scope(command)?,
        received_at: parent_meta.received_at,
    })
}

fn decode_existing_job_result<T: DeserializeOwned>(
    record: JobRunRecord,
) -> Result<T, MethodLibraryError> {
    match record.status {
        JobRunStatus::Running => Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobStatusConflict,
            "operations job is still running",
        )
        .with_detail("job_run_id", record.job_run_id)),
        JobRunStatus::Failed => {
            let reason: FailureReason = serde_json::from_value(record.result.ok_or_else(|| {
                MethodLibraryError::retryable(
                    MethodLibraryErrorCode::PersistenceUnavailable,
                    "failed job run is missing its serialized failure reason",
                )
            })?)
            .map_err(|error| {
                MethodLibraryError::retryable(
                    MethodLibraryErrorCode::PersistenceUnavailable,
                    format!("failed to decode job failure reason: {error}"),
                )
            })?;
            Err(MethodLibraryError::new(
                reason.code,
                reason.message,
                reason.retryable,
            ))
        }
        JobRunStatus::Succeeded | JobRunStatus::PartiallySucceeded => {
            serde_json::from_value::<T>(record.result.ok_or_else(|| {
                MethodLibraryError::retryable(
                    MethodLibraryErrorCode::PersistenceUnavailable,
                    "completed job run is missing its serialized result",
                )
            })?)
            .map_err(|error| {
                MethodLibraryError::retryable(
                    MethodLibraryErrorCode::PersistenceUnavailable,
                    format!("failed to decode cached job result: {error}"),
                )
            })
        }
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
                "published content is missing its version",
            )
            .with_detail("content_id", content.content_id.clone())
        })?,
        fingerprint: content.fingerprint.clone().ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "published content is missing its fingerprint",
            )
            .with_detail("content_id", content.content_id.clone())
        })?,
    })
}

fn synthetic_published_ref(kind: MethodContentKind, asset_key: &str) -> PublishedContentRef {
    PublishedContentRef {
        content_id: format!("planned:{asset_key}"),
        kind,
        version: ContentVersion::new(DEFAULT_PUBLISHED_VERSION).expect("seed version is valid"),
        fingerprint: CanonicalFingerprint::new(
            FingerprintAlgorithm::Sha256,
            format!("planned-{asset_key}"),
            CANONICAL_SCHEMA_VERSION,
        )
        .expect("synthetic seed fingerprint should be valid"),
    }
}

fn replay_checkpoint_name(consumer: &str) -> String {
    format!("{REPLAY_CHECKPOINT_PREFIX}:{consumer}")
}

fn replay_topic_for_event_type(
    event_type: method_library_contracts::DefinitionEventType,
) -> String {
    match event_type {
        method_library_contracts::DefinitionEventType::ContentPublished
        | method_library_contracts::DefinitionEventType::FingerprintChanged => {
            "method-library.definition.events".to_string()
        }
        method_library_contracts::DefinitionEventType::ContentDeprecated
        | method_library_contracts::DefinitionEventType::ContentRetired => {
            "method-library.lifecycle.events".to_string()
        }
    }
}

fn baseline_role_specs() -> [(&'static str, &'static str, &'static str); 9] {
    [
        (
            "assistant",
            "Assistant",
            "Provide general delivery assistance",
        ),
        (
            "tech-lead",
            "Tech Lead",
            "Guide technical design and delivery",
        ),
        (
            "backend-dev",
            "Backend Developer",
            "Implement backend services",
        ),
        (
            "frontend-dev",
            "Frontend Developer",
            "Implement frontend experiences",
        ),
        ("qa", "QA", "Verify delivery quality"),
        ("ux", "UX", "Shape user experience decisions"),
        ("devops", "DevOps", "Operate build and deployment flows"),
        ("auditor", "Auditor", "Review compliance and traceability"),
        (
            "observer",
            "Observer",
            "Inspect delivery progress without mutating it",
        ),
    ]
}

fn baseline_process_template_specs() -> [(&'static str, &'static str); 8] {
    [
        ("waterfall", "Waterfall"),
        ("v-model", "V-Model"),
        ("incremental", "Incremental"),
        ("evolutionary", "Evolutionary"),
        ("iterative", "Iterative"),
        ("spiral", "Spiral"),
        ("agile", "Agile"),
        ("devops", "DevOps"),
    ]
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use method_library_contracts::{
        ActorContext, ActorRef, ContentPublishedPayload, DefinitionEventEnvelope,
        DefinitionEventPayload, DefinitionEventType, EventTraceContext,
        RebuildReadModelsJobRequest, RecalculateFingerprintJobRequest,
        ReplayDefinitionEventsJobRequest, RequestMeta, SeedInitialMethodAssetsJobRequest,
    };
    use time::macros::datetime;

    use super::{CANONICAL_SCHEMA_VERSION, MethodOperationsService};
    use crate::ports::fakes::{
        DeterministicClock, DeterministicFingerprintHasher, DeterministicIdGenerator,
        FakeUnitOfWork, InMemoryAuditRepository, InMemoryContentSummaryProjectionRepository,
        InMemoryDefinitionSnapshotRepository, InMemoryDefinitionTraceProjectionRepository,
        InMemoryIdempotencyRepository, InMemoryJobRunRepository,
        InMemoryLifecycleHistoryRepository, InMemoryMethodContentReferenceRepository,
        InMemoryMethodContentRepository, InMemoryMethodContentVersionRepository,
        InMemoryObjectStorage, InMemoryOutboxRepository, InMemoryProjectionCheckpointRepository,
        InMemorySupersedeLinkRepository, RecordingBusPublisher, StaticGovernancePort,
    };
    use crate::ports::{
        ContentSummaryProjectionRepository, DefinitionTraceProjectionRepository,
        MethodContentRepository, OutboxEvent, OutboxRepository, ProjectionCheckpointRepository,
        UnitOfWork,
    };
    use crate::services::MethodContentCommandService;
    use method_library_domain::content::{
        ActorKind, ApprovedGateRef, CanonicalFingerprint, ContentVersion, FingerprintAlgorithm,
        MethodContent, MethodContentKind, PublishedContentRef,
    };
    use method_library_domain::definitions::{
        EvidenceKind, EvidenceRule, MethodContentPayload, Qualification, QualificationLevel,
        QualificationLevelModel,
    };
    use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};

    struct TestHarness {
        service: MethodOperationsService,
        unit_of_work: Arc<FakeUnitOfWork>,
        content_repository: Arc<InMemoryMethodContentRepository>,
        outbox_repository: Arc<InMemoryOutboxRepository>,
        summary_repository: Arc<InMemoryContentSummaryProjectionRepository>,
        trace_repository: Arc<InMemoryDefinitionTraceProjectionRepository>,
        checkpoint_repository: Arc<InMemoryProjectionCheckpointRepository>,
        bus_publisher: Arc<RecordingBusPublisher>,
    }

    fn sample_actor() -> ActorContext {
        ActorContext {
            actor_id: "actor-1".to_string(),
            actor_kind: ActorKind::System,
            actor_ref: ActorRef {
                actor_id: "actor-1".to_string(),
                actor_kind: ActorKind::System,
            },
        }
    }

    fn sample_meta(idempotency_key: &str) -> RequestMeta {
        RequestMeta {
            request_id: format!("req-{idempotency_key}"),
            trace_id: format!("trace-{idempotency_key}"),
            idempotency_key: Some(idempotency_key.to_string()),
            request_hash: format!("hash-{idempotency_key}"),
            received_at: datetime!(2026-05-27 03:00:00 UTC),
        }
    }

    fn test_harness() -> TestHarness {
        let unit_of_work = Arc::new(FakeUnitOfWork);
        let content_repository = Arc::new(InMemoryMethodContentRepository::default());
        let content_reference_repository =
            Arc::new(InMemoryMethodContentReferenceRepository::default());
        let version_repository = Arc::new(InMemoryMethodContentVersionRepository::default());
        let snapshot_repository = Arc::new(InMemoryDefinitionSnapshotRepository::default());
        let supersede_link_repository = Arc::new(InMemorySupersedeLinkRepository::default());
        let outbox_repository = Arc::new(InMemoryOutboxRepository::default());
        let lifecycle_repository = Arc::new(InMemoryLifecycleHistoryRepository::default());
        let audit_repository = Arc::new(InMemoryAuditRepository::default());
        let idempotency_repository = Arc::new(InMemoryIdempotencyRepository::default());
        let object_storage = Arc::new(InMemoryObjectStorage::default());
        let fingerprint_hasher = Arc::new(DeterministicFingerprintHasher::default());
        let clock = Arc::new(DeterministicClock::new(datetime!(2026-05-27 03:00:00 UTC)));
        let id_generator = Arc::new(DeterministicIdGenerator::default());
        let summary_repository = Arc::new(InMemoryContentSummaryProjectionRepository::default());
        let trace_repository = Arc::new(InMemoryDefinitionTraceProjectionRepository::default());
        let checkpoint_repository = Arc::new(InMemoryProjectionCheckpointRepository::default());
        let job_run_repository = Arc::new(InMemoryJobRunRepository::default());
        let bus_publisher = Arc::new(RecordingBusPublisher::default());

        let command_service = Arc::new(MethodContentCommandService::new(
            unit_of_work.clone(),
            content_repository.clone(),
            content_reference_repository,
            version_repository.clone(),
            snapshot_repository.clone(),
            supersede_link_repository.clone(),
            outbox_repository.clone(),
            lifecycle_repository.clone(),
            audit_repository.clone(),
            idempotency_repository,
            Arc::new(StaticGovernancePort::new(
                true,
                datetime!(2026-05-27 03:00:00 UTC),
            )),
            object_storage,
            fingerprint_hasher.clone(),
            clock.clone(),
            id_generator,
        ));

        let service = MethodOperationsService::new(
            unit_of_work.clone(),
            command_service,
            content_repository.clone(),
            version_repository,
            lifecycle_repository,
            audit_repository,
            supersede_link_repository,
            outbox_repository.clone(),
            summary_repository.clone(),
            trace_repository.clone(),
            checkpoint_repository.clone(),
            snapshot_repository,
            job_run_repository,
            bus_publisher.clone(),
            fingerprint_hasher,
            clock,
        );

        TestHarness {
            service,
            unit_of_work,
            content_repository,
            outbox_repository,
            summary_repository,
            trace_repository,
            checkpoint_repository,
            bus_publisher,
        }
    }

    #[tokio::test]
    async fn seeds_the_baseline_asset_set_with_publish_enabled() {
        let harness = test_harness();

        let result = harness
            .service
            .seed_initial_method_assets(
                SeedInitialMethodAssetsJobRequest {
                    asset_set: "baseline".to_string(),
                    kinds: Vec::new(),
                    publish: true,
                    dry_run: false,
                },
                sample_actor(),
                sample_meta("seed-baseline"),
            )
            .await
            .expect("seed should succeed");

        assert_eq!(result.created_count, 28);
        assert_eq!(result.published_count, 28);
        assert_eq!(result.skipped_count, 0);
        assert!(result.failures.is_empty());

        let contents = harness
            .content_repository
            .list_all()
            .await
            .expect("seeded contents should be readable");
        assert_eq!(contents.len(), 28);
        assert!(contents.iter().all(MethodContent::is_published_like));
    }

    #[tokio::test]
    async fn seed_dry_run_reports_without_writing_truth_or_outbox() {
        let harness = test_harness();

        let result = harness
            .service
            .seed_initial_method_assets(
                SeedInitialMethodAssetsJobRequest {
                    asset_set: "baseline".to_string(),
                    kinds: Vec::new(),
                    publish: true,
                    dry_run: true,
                },
                sample_actor(),
                sample_meta("seed-dry-run"),
            )
            .await
            .expect("dry-run seed should succeed");

        assert_eq!(result.created_count, 28);
        assert_eq!(result.published_count, 28);
        assert!(
            harness
                .content_repository
                .list_all()
                .await
                .expect("contents should be readable")
                .is_empty()
        );
        assert!(
            harness
                .outbox_repository
                .events()
                .expect("outbox events should be readable")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn replay_reuses_original_event_ids_and_advances_the_consumer_checkpoint() {
        let harness = test_harness();
        append_outbox_event(&harness, sample_outbox_event("evt-1")).await;
        append_outbox_event(&harness, sample_outbox_event("evt-2")).await;

        let result = harness
            .service
            .replay_definition_events(
                ReplayDefinitionEventsJobRequest {
                    consumer: "identity".to_string(),
                    from_cursor: None,
                    event_types: vec![DefinitionEventType::ContentPublished],
                    batch_size: 10,
                    dry_run: false,
                },
                sample_actor(),
                sample_meta("replay-1"),
            )
            .await
            .expect("replay should succeed");

        assert_eq!(result.replayed_count, 2);
        assert_eq!(result.next_cursor.as_deref(), Some("evt-2"));
        assert!(result.failures.is_empty());

        let published = harness
            .bus_publisher
            .published_events()
            .expect("published events should be readable");
        assert_eq!(published.len(), 2);
        assert_eq!(published[0].1.event_id, "evt-1");
        assert_eq!(published[1].1.event_id, "evt-2");

        let checkpoint = harness
            .checkpoint_repository
            .get("replay_consumer:identity".to_string())
            .await
            .expect("checkpoint should be readable")
            .expect("checkpoint should exist");
        assert_eq!(checkpoint.last_processed_event_id.as_deref(), Some("evt-2"));
    }

    #[tokio::test]
    async fn replay_failure_does_not_advance_past_the_failed_event() {
        let harness = test_harness();
        append_outbox_event(&harness, sample_outbox_event("evt-1")).await;
        append_outbox_event(&harness, sample_outbox_event("evt-2")).await;
        harness
            .bus_publisher
            .set_failure(Some(MethodLibraryError::retryable(
                MethodLibraryErrorCode::BusPublishFailed,
                "temporary outage",
            )))
            .expect("bus failure should be configurable");

        let result = harness
            .service
            .replay_definition_events(
                ReplayDefinitionEventsJobRequest {
                    consumer: "identity".to_string(),
                    from_cursor: None,
                    event_types: vec![DefinitionEventType::ContentPublished],
                    batch_size: 10,
                    dry_run: false,
                },
                sample_actor(),
                sample_meta("replay-failure"),
            )
            .await
            .expect("replay should complete with partial failure");

        assert_eq!(result.replayed_count, 0);
        assert_eq!(result.next_cursor, None);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].item_ref, "evt-1");
        assert!(
            harness
                .checkpoint_repository
                .get("replay_consumer:identity".to_string())
                .await
                .expect("checkpoint should be readable")
                .is_none()
        );
    }

    #[tokio::test]
    async fn recalculate_reports_mismatches_without_mutating_published_truth() {
        let harness = test_harness();
        let wrong = sample_published_content_with_fingerprint("wrong-fingerprint");
        insert_content(&harness, wrong.clone(), "wrong").await;

        let result = harness
            .service
            .recalculate_fingerprint(
                RecalculateFingerprintJobRequest {
                    content_ids: vec![wrong.content_id.clone()],
                    kind: None,
                    canonical_schema_version: CANONICAL_SCHEMA_VERSION.to_string(),
                    dry_run: true,
                },
                sample_actor(),
                sample_meta("recalculate-1"),
            )
            .await
            .expect("recalculate should succeed");

        assert_eq!(result.checked_count, 1);
        assert_eq!(result.mismatches.len(), 1);
        let stored = harness
            .content_repository
            .get(wrong.content_id.clone())
            .await
            .expect("content should be readable")
            .expect("content should exist");
        assert_eq!(stored.fingerprint, wrong.fingerprint);
    }

    #[tokio::test]
    async fn rebuild_reconstructs_summary_and_trace_projections() {
        let harness = test_harness();
        let seed = harness
            .service
            .seed_initial_method_assets(
                SeedInitialMethodAssetsJobRequest {
                    asset_set: "baseline".to_string(),
                    kinds: vec![MethodContentKind::Qualification],
                    publish: true,
                    dry_run: false,
                },
                sample_actor(),
                sample_meta("seed-for-rebuild"),
            )
            .await
            .expect("seed should succeed");
        assert_eq!(seed.created_count, 1);

        let result = harness
            .service
            .rebuild_read_models(
                RebuildReadModelsJobRequest {
                    projection_names: Vec::new(),
                    from_cursor: None,
                    batch_size: 50,
                    dry_run: false,
                },
                sample_actor(),
                sample_meta("rebuild-1"),
            )
            .await
            .expect("rebuild should succeed");

        assert_eq!(result.processed_count, 1);
        assert!(result.failures.is_empty());
        assert!(result.checkpoint.is_some());
        assert_eq!(
            harness
                .summary_repository
                .list(
                    &method_library_contracts::ListMethodContentsQuery {
                        kind: Some(MethodContentKind::Qualification),
                        lifecycle_state: None,
                        read_mode: method_library_contracts::ReadMode::Published,
                        cursor: None,
                        limit: 10,
                        sort: Some("content_id".to_string()),
                    },
                    &crate::ports::PageRequest {
                        cursor: None,
                        limit: 10,
                    },
                )
                .await
                .expect("summary projection should be readable")
                .len(),
            1
        );
        assert_eq!(
            harness
                .trace_repository
                .get("content-1".to_string())
                .await
                .expect("trace projection should be readable")
                .expect("trace projection should exist")
                .versions
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn rebuild_dry_run_does_not_write_projections_or_checkpoints() {
        let harness = test_harness();
        insert_content(
            &harness,
            sample_published_content_with_fingerprint("abc123"),
            "content",
        )
        .await;

        let result = harness
            .service
            .rebuild_read_models(
                RebuildReadModelsJobRequest {
                    projection_names: Vec::new(),
                    from_cursor: None,
                    batch_size: 50,
                    dry_run: true,
                },
                sample_actor(),
                sample_meta("rebuild-dry-run"),
            )
            .await
            .expect("dry-run rebuild should succeed");

        assert_eq!(result.processed_count, 1);
        assert!(result.checkpoint.is_none());
        assert!(
            harness
                .summary_repository
                .list(
                    &method_library_contracts::ListMethodContentsQuery {
                        kind: None,
                        lifecycle_state: None,
                        read_mode: method_library_contracts::ReadMode::Authoring,
                        cursor: None,
                        limit: 10,
                        sort: Some("content_id".to_string()),
                    },
                    &crate::ports::PageRequest {
                        cursor: None,
                        limit: 10,
                    },
                )
                .await
                .expect("summary projection should be readable")
                .is_empty()
        );
        assert!(
            harness
                .checkpoint_repository
                .get("content_summary_projection".to_string())
                .await
                .expect("checkpoint should be readable")
                .is_none()
        );
    }

    async fn append_outbox_event(harness: &TestHarness, event: OutboxEvent) {
        let mut tx = harness
            .unit_of_work
            .begin(sample_meta("append-outbox"))
            .await
            .expect("transaction should begin");
        harness
            .outbox_repository
            .append(&mut tx, event)
            .await
            .expect("outbox event should append");
        tx.commit().await.expect("transaction should commit");
    }

    async fn insert_content(harness: &TestHarness, content: MethodContent, key: &str) {
        let mut tx = harness
            .unit_of_work
            .begin(sample_meta(key))
            .await
            .expect("transaction should begin");
        harness
            .content_repository
            .insert(&mut tx, content)
            .await
            .expect("content should insert");
        tx.commit().await.expect("transaction should commit");
    }

    fn sample_outbox_event(event_id: &str) -> OutboxEvent {
        OutboxEvent::new_pending(
            event_id.to_string(),
            "content-1".to_string(),
            DefinitionEventEnvelope {
                event_id: event_id.to_string(),
                event_type: DefinitionEventType::ContentPublished,
                schema_version: "1.0".to_string(),
                occurred_at: datetime!(2026-05-27 03:10:00 UTC),
                producer: "L3-method-library".to_string(),
                content_ref: PublishedContentRef {
                    content_id: "content-1".to_string(),
                    kind: MethodContentKind::Qualification,
                    version: ContentVersion::new("1.0.0").expect("version should be valid"),
                    fingerprint: CanonicalFingerprint::new(
                        FingerprintAlgorithm::Sha256,
                        "abc123",
                        "1.0",
                    )
                    .expect("fingerprint should be valid"),
                },
                snapshot_ref: None,
                trace: EventTraceContext {
                    request_id: "req-source".to_string(),
                    trace_id: "trace-source".to_string(),
                },
                payload: DefinitionEventPayload::ContentPublished(ContentPublishedPayload {
                    gate_ref: ApprovedGateRef {
                        gate_id: "gate-1".to_string(),
                        gate_decision_id: "decision-1".to_string(),
                        approved_at: datetime!(2026-05-27 03:05:00 UTC),
                    },
                    version: ContentVersion::new("1.0.0").expect("version should be valid"),
                    fingerprint: CanonicalFingerprint::new(
                        FingerprintAlgorithm::Sha256,
                        "abc123",
                        "1.0",
                    )
                    .expect("fingerprint should be valid"),
                }),
            },
            format!("payload-hash-{event_id}"),
            Some(format!("idem-{event_id}")),
        )
        .expect("outbox event should be valid")
    }

    fn sample_published_content_with_fingerprint(fingerprint_value: &str) -> MethodContent {
        let mut content = MethodContent::create_draft(
            "content-1".to_string(),
            "family-1".to_string(),
            MethodContentKind::Qualification,
            "Baseline Delivery Qualification".to_string(),
            None,
            MethodContentPayload::Qualification(Qualification {
                qualification_key: "baseline_delivery".to_string(),
                name: "Baseline Delivery Qualification".to_string(),
                description: None,
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
                    description: "Evidence".to_string(),
                }],
            }),
            "actor-1".to_string(),
            datetime!(2026-05-27 03:00:00 UTC),
        )
        .expect("content should be valid");
        content
            .submit_for_review("actor-1".to_string(), datetime!(2026-05-27 03:01:00 UTC))
            .expect("content should enter review");
        content
            .publish(
                ApprovedGateRef {
                    gate_id: "gate-1".to_string(),
                    gate_decision_id: "decision-1".to_string(),
                    approved_at: datetime!(2026-05-27 03:02:00 UTC),
                },
                ContentVersion::new("1.0.0").expect("version should be valid"),
                CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, fingerprint_value, "1.0")
                    .expect("fingerprint should be valid"),
                "actor-1".to_string(),
                datetime!(2026-05-27 03:02:00 UTC),
            )
            .expect("content should publish");
        content
    }
}
