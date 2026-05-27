//! Worker wrapper for seed, replay, recalculate, and rebuild operations jobs.

use method_library_application::MethodOperationsService;
use method_library_contracts::{
    ActorContext, RebuildReadModelsJobRequest, RebuildReadModelsJobResult,
    RecalculateFingerprintJobRequest, RecalculateFingerprintJobResult,
    ReplayDefinitionEventsJobRequest, ReplayDefinitionEventsJobResult, RequestMeta,
    SeedInitialMethodAssetsJobRequest, SeedInitialMethodAssetsJobResult,
};
use method_library_domain::MethodLibraryError;

/// Thin worker entrypoint for operations jobs.
#[derive(Clone)]
pub struct OperationsJobRunner {
    service: MethodOperationsService,
}

impl OperationsJobRunner {
    /// Creates a worker runner from the application operations service.
    #[must_use]
    pub fn new(service: MethodOperationsService) -> Self {
        Self { service }
    }

    /// Runs the seed job once.
    pub async fn run_seed(
        &self,
        request: SeedInitialMethodAssetsJobRequest,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<SeedInitialMethodAssetsJobResult, MethodLibraryError> {
        self.service
            .seed_initial_method_assets(request, actor, meta)
            .await
    }

    /// Runs the replay job once.
    pub async fn run_replay(
        &self,
        request: ReplayDefinitionEventsJobRequest,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<ReplayDefinitionEventsJobResult, MethodLibraryError> {
        self.service
            .replay_definition_events(request, actor, meta)
            .await
    }

    /// Runs the fingerprint recalculation job once.
    pub async fn run_recalculate(
        &self,
        request: RecalculateFingerprintJobRequest,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<RecalculateFingerprintJobResult, MethodLibraryError> {
        self.service
            .recalculate_fingerprint(request, actor, meta)
            .await
    }

    /// Runs the read-model rebuild job once.
    pub async fn run_rebuild(
        &self,
        request: RebuildReadModelsJobRequest,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<RebuildReadModelsJobResult, MethodLibraryError> {
        self.service.rebuild_read_models(request, actor, meta).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use method_library_application::MethodContentRepository;
    use method_library_application::ports::fakes::{
        DeterministicClock, DeterministicFingerprintHasher, DeterministicIdGenerator,
        FakeUnitOfWork, InMemoryAuditRepository, InMemoryContentSummaryProjectionRepository,
        InMemoryDefinitionSnapshotRepository, InMemoryDefinitionTraceProjectionRepository,
        InMemoryIdempotencyRepository, InMemoryJobRunRepository,
        InMemoryLifecycleHistoryRepository, InMemoryMethodContentReferenceRepository,
        InMemoryMethodContentRepository, InMemoryMethodContentVersionRepository,
        InMemoryObjectStorage, InMemoryOutboxRepository, InMemoryProjectionCheckpointRepository,
        InMemorySupersedeLinkRepository, RecordingBusPublisher, StaticGovernancePort,
    };
    use method_library_application::{MethodContentCommandService, MethodOperationsService};
    use method_library_contracts::{
        ActorContext, ActorRef, RequestMeta, SeedInitialMethodAssetsJobRequest,
    };
    use method_library_domain::content::ActorKind;
    use time::macros::datetime;

    use super::OperationsJobRunner;

    fn sample_actor() -> ActorContext {
        ActorContext {
            actor_id: "worker-actor".to_string(),
            actor_kind: ActorKind::System,
            actor_ref: ActorRef {
                actor_id: "worker-actor".to_string(),
                actor_kind: ActorKind::System,
            },
        }
    }

    fn sample_meta() -> RequestMeta {
        RequestMeta {
            request_id: "req-worker".to_string(),
            trace_id: "trace-worker".to_string(),
            idempotency_key: Some("idem-worker".to_string()),
            request_hash: "hash-worker".to_string(),
            received_at: datetime!(2026-05-27 04:00:00 UTC),
        }
    }

    #[tokio::test]
    async fn runs_seed_jobs_through_the_application_service() {
        let unit_of_work = Arc::new(FakeUnitOfWork);
        let content_repository = Arc::new(InMemoryMethodContentRepository::default());
        let version_repository = Arc::new(InMemoryMethodContentVersionRepository::default());
        let lifecycle_repository = Arc::new(InMemoryLifecycleHistoryRepository::default());
        let audit_repository = Arc::new(InMemoryAuditRepository::default());
        let outbox_repository = Arc::new(InMemoryOutboxRepository::default());
        let snapshot_repository = Arc::new(InMemoryDefinitionSnapshotRepository::default());
        let supersede_repository = Arc::new(InMemorySupersedeLinkRepository::default());
        let fingerprint_hasher = Arc::new(DeterministicFingerprintHasher::default());
        let clock = Arc::new(DeterministicClock::new(datetime!(2026-05-27 04:00:00 UTC)));
        let command_service = Arc::new(MethodContentCommandService::new(
            unit_of_work.clone(),
            content_repository.clone(),
            Arc::new(InMemoryMethodContentReferenceRepository::default()),
            version_repository.clone(),
            snapshot_repository.clone(),
            supersede_repository.clone(),
            outbox_repository.clone(),
            lifecycle_repository.clone(),
            audit_repository.clone(),
            Arc::new(InMemoryIdempotencyRepository::default()),
            Arc::new(StaticGovernancePort::new(
                true,
                datetime!(2026-05-27 04:00:00 UTC),
            )),
            Arc::new(InMemoryObjectStorage::default()),
            fingerprint_hasher.clone(),
            clock.clone(),
            Arc::new(DeterministicIdGenerator::default()),
        ));
        let service = MethodOperationsService::new(
            unit_of_work,
            command_service,
            content_repository.clone(),
            version_repository,
            lifecycle_repository,
            audit_repository,
            supersede_repository,
            outbox_repository,
            Arc::new(InMemoryContentSummaryProjectionRepository::default()),
            Arc::new(InMemoryDefinitionTraceProjectionRepository::default()),
            Arc::new(InMemoryProjectionCheckpointRepository::default()),
            snapshot_repository,
            Arc::new(InMemoryJobRunRepository::default()),
            Arc::new(RecordingBusPublisher::default()),
            fingerprint_hasher,
            clock,
        );
        let runner = OperationsJobRunner::new(service);

        let result = runner
            .run_seed(
                SeedInitialMethodAssetsJobRequest {
                    asset_set: "baseline".to_string(),
                    kinds: Vec::new(),
                    publish: true,
                    dry_run: true,
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect("worker runner should execute the seed job");

        assert_eq!(result.created_count, 28);
        assert_eq!(result.published_count, 28);
        assert!(
            content_repository
                .list_all()
                .await
                .expect("contents should be readable")
                .is_empty()
        );
    }
}
