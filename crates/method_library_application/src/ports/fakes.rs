//! In-memory fake adapters for application-port tests.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;

use method_library_contracts::{
    ActorContext, ContentSummaryView, DefinitionEventEnvelope, DefinitionSnapshot,
    DefinitionTraceView, ListMethodContentsQuery, RequestMeta, SnapshotBlobRef, SnapshotPayload,
};
use method_library_domain::content::{
    ApprovedGateRef, CanonicalBytes, CanonicalFingerprint, ContentFamilyId, ContentId, ContentRef,
    ContentVersion, FingerprintAlgorithm, IdempotencyKey, JobName, JobRunId, LeaseDuration,
    MethodContent, OutboxEventId, PublishedContentRef, RequestHash, Revision, SnapshotId,
    Timestamp, WorkerId,
};
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};

use super::{
    AuditId, AuditRecord, AuditRepository, BusPublisherPort, CheckpointName, CheckpointStatus,
    Clock, ContentSummaryProjectionRepository, ContentVersionRecordId,
    DefinitionSnapshotRepository, DefinitionTraceProjectionRepository, FailureReason, FeatureFlag,
    FeatureFlagPort, FingerprintHasher, GateValidationResult, GovernancePort, HistoryEntryId,
    IdGenerator, IdempotencyBeginResult, IdempotencyRecord, IdempotencyRepository,
    IdempotencyScope, IdempotencyStatus, InboundDeadLetter, InboundDeadLetterRepository, JobResult,
    JobRunRecord, JobRunRepository, JobRunStartResult, JobRunStatus, LifecycleHistoryEntry,
    LifecycleHistoryRepository, MethodContentReferenceRepository, MethodContentRepository,
    MethodContentVersionRecord, MethodContentVersionRepository, ObjectStoragePort,
    ObservabilityEvent, ObservabilityPort, OutboxEvent, OutboxRepository, OutboxStatus,
    PageRequest, ProjectionCheckpointRecord, ProjectionCheckpointRepository, PublishAck, ResultRef,
    SupersedeLink, SupersedeLinkRepository, Topic, UnitOfWork, UnitOfWorkTx,
};

/// In-memory unit-of-work implementation.
#[derive(Debug, Default, Clone)]
pub struct FakeUnitOfWork;

/// Deterministic clock used by tests.
#[derive(Debug, Clone)]
pub struct DeterministicClock {
    now: Timestamp,
}

/// Deterministic ID generator that increments per ID kind.
#[derive(Debug, Default)]
pub struct DeterministicIdGenerator {
    counters: Mutex<HashMap<&'static str, u64>>,
}

/// Deterministic fingerprint hasher used by tests.
#[derive(Debug, Default, Clone)]
pub struct DeterministicFingerprintHasher;

/// Feature-flag port backed by a static enabled set.
#[derive(Debug, Default, Clone)]
pub struct StaticFeatureFlagPort {
    enabled: HashSet<FeatureFlag>,
}

/// Observability port that stores events in memory.
#[derive(Debug, Default, Clone)]
pub struct RecordingObservabilityPort {
    events: Arc<Mutex<Vec<ObservabilityEvent>>>,
}

/// Governance port that returns a fixed approval state.
#[derive(Debug, Clone)]
pub struct StaticGovernancePort {
    approved: bool,
    validated_at: Timestamp,
}

/// Bus publisher that stores published events in memory.
#[derive(Debug, Default, Clone)]
pub struct RecordingBusPublisher {
    published: Arc<Mutex<Vec<(Topic, DefinitionEventEnvelope, RequestMeta)>>>,
    fail_with: Arc<Mutex<Option<MethodLibraryError>>>,
}

/// Object storage port backed by an in-memory map.
#[derive(Debug, Default, Clone)]
pub struct InMemoryObjectStorage {
    blobs: Arc<Mutex<HashMap<SnapshotBlobRef, SnapshotPayload>>>,
}

/// In-memory method-content repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryMethodContentRepository {
    contents: Arc<Mutex<HashMap<ContentId, MethodContent>>>,
}

/// In-memory reference repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryMethodContentReferenceRepository {
    draft_refs: Arc<Mutex<HashMap<ContentId, Vec<ContentRef>>>>,
    published_refs: Arc<Mutex<HashMap<ContentId, Vec<PublishedContentRef>>>>,
}

/// In-memory version-history repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryMethodContentVersionRepository {
    records: Arc<Mutex<Vec<MethodContentVersionRecord>>>,
}

/// In-memory supersede-link repository.
#[derive(Debug, Default, Clone)]
pub struct InMemorySupersedeLinkRepository {
    links: Arc<Mutex<Vec<SupersedeLink>>>,
}

/// In-memory lifecycle-history repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryLifecycleHistoryRepository {
    entries: Arc<Mutex<Vec<LifecycleHistoryEntry>>>,
}

/// In-memory audit repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryAuditRepository {
    records: Arc<Mutex<Vec<AuditRecord>>>,
}

/// In-memory idempotency repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryIdempotencyRepository {
    records: Arc<Mutex<HashMap<(IdempotencyScope, IdempotencyKey), IdempotencyRecord>>>,
}

/// In-memory outbox repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryOutboxRepository {
    events: Arc<Mutex<HashMap<OutboxEventId, OutboxEvent>>>,
}

/// In-memory snapshot repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryDefinitionSnapshotRepository {
    snapshots: Arc<Mutex<HashMap<SnapshotId, DefinitionSnapshot>>>,
}

/// In-memory content-summary projection repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryContentSummaryProjectionRepository {
    views: Arc<Mutex<HashMap<ContentId, ContentSummaryView>>>,
}

/// In-memory definition-trace projection repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryDefinitionTraceProjectionRepository {
    views: Arc<Mutex<HashMap<ContentId, DefinitionTraceView>>>,
}

/// In-memory projection-checkpoint repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryProjectionCheckpointRepository {
    checkpoints: Arc<Mutex<HashMap<CheckpointName, ProjectionCheckpointRecord>>>,
}

/// In-memory inbound dead-letter repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryInboundDeadLetterRepository {
    records: Arc<Mutex<Vec<InboundDeadLetter>>>,
}

/// In-memory job-run repository.
#[derive(Debug, Default, Clone)]
pub struct InMemoryJobRunRepository {
    records: Arc<Mutex<HashMap<(JobName, String), JobRunRecord>>>,
}

impl DeterministicClock {
    /// Creates a deterministic clock pinned to one instant.
    #[must_use]
    pub fn new(now: Timestamp) -> Self {
        Self { now }
    }
}

impl StaticFeatureFlagPort {
    /// Creates a static feature-flag port from an enabled set.
    #[must_use]
    pub fn new(enabled: impl IntoIterator<Item = FeatureFlag>) -> Self {
        Self {
            enabled: enabled.into_iter().collect(),
        }
    }
}

impl RecordingObservabilityPort {
    /// Returns the recorded observability events.
    pub fn events(&self) -> Result<Vec<ObservabilityEvent>, MethodLibraryError> {
        Ok(lock(&self.events)?.clone())
    }
}

impl StaticGovernancePort {
    /// Creates a static governance port.
    #[must_use]
    pub fn new(approved: bool, validated_at: Timestamp) -> Self {
        Self {
            approved,
            validated_at,
        }
    }
}

impl RecordingBusPublisher {
    /// Configures the publisher to fail with a specific error.
    pub fn set_failure(&self, error: Option<MethodLibraryError>) -> Result<(), MethodLibraryError> {
        *lock(&self.fail_with)? = error;
        Ok(())
    }

    /// Returns the published event log.
    pub fn published_events(
        &self,
    ) -> Result<Vec<(Topic, DefinitionEventEnvelope, RequestMeta)>, MethodLibraryError> {
        Ok(lock(&self.published)?.clone())
    }
}

impl InMemoryMethodContentRepository {
    /// Returns the stored contents for inspection.
    pub fn contents(&self) -> Result<HashMap<ContentId, MethodContent>, MethodLibraryError> {
        Ok(lock(&self.contents)?.clone())
    }
}

impl InMemoryIdempotencyRepository {
    /// Returns the stored idempotency records for inspection.
    pub fn records(
        &self,
    ) -> Result<HashMap<(IdempotencyScope, IdempotencyKey), IdempotencyRecord>, MethodLibraryError>
    {
        Ok(lock(&self.records)?.clone())
    }
}

impl InMemoryOutboxRepository {
    /// Returns the stored outbox records for inspection.
    pub fn events(&self) -> Result<HashMap<OutboxEventId, OutboxEvent>, MethodLibraryError> {
        Ok(lock(&self.events)?.clone())
    }
}

impl InMemoryProjectionCheckpointRepository {
    /// Returns the stored checkpoint records for inspection.
    pub fn checkpoints(
        &self,
    ) -> Result<HashMap<CheckpointName, ProjectionCheckpointRecord>, MethodLibraryError> {
        Ok(lock(&self.checkpoints)?.clone())
    }
}

#[async_trait]
impl UnitOfWork for FakeUnitOfWork {
    async fn begin(&self, meta: RequestMeta) -> Result<UnitOfWorkTx, MethodLibraryError> {
        Ok(UnitOfWorkTx::new(meta))
    }
}

impl Clock for DeterministicClock {
    fn now(&self) -> Timestamp {
        self.now
    }
}

impl IdGenerator for DeterministicIdGenerator {
    fn new_content_id(&self) -> ContentId {
        next_id(&self.counters, "content", "content")
    }

    fn new_content_family_id(&self) -> ContentFamilyId {
        next_id(&self.counters, "content_family", "family")
    }

    fn new_outbox_event_id(&self) -> OutboxEventId {
        next_id(&self.counters, "outbox", "evt")
    }

    fn new_snapshot_id(&self) -> SnapshotId {
        next_id(&self.counters, "snapshot", "snap")
    }

    fn new_job_run_id(&self) -> JobRunId {
        next_id(&self.counters, "job_run", "job")
    }

    fn new_content_version_record_id(&self) -> ContentVersionRecordId {
        next_id(&self.counters, "content_version", "version_record")
    }

    fn new_history_entry_id(&self) -> HistoryEntryId {
        next_id(&self.counters, "history", "history")
    }

    fn new_audit_id(&self) -> AuditId {
        next_id(&self.counters, "audit", "audit")
    }

    fn new_supersede_link_id(&self) -> String {
        next_id(&self.counters, "supersede", "supersede")
    }

    fn new_dead_letter_id(&self) -> super::DeadLetterId {
        next_id(&self.counters, "dead_letter", "dead_letter")
    }
}

impl FingerprintHasher for DeterministicFingerprintHasher {
    fn hash_canonical_bytes(
        &self,
        bytes: CanonicalBytes,
        algorithm: FingerprintAlgorithm,
        schema_version: String,
    ) -> Result<CanonicalFingerprint, MethodLibraryError> {
        let hex = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        CanonicalFingerprint::new(algorithm, hex, schema_version)
    }
}

impl FeatureFlagPort for StaticFeatureFlagPort {
    fn ensure_enabled(&self, flag: FeatureFlag) -> Result<(), MethodLibraryError> {
        if self.enabled.contains(&flag) {
            Ok(())
        } else {
            Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::P1FeatureDisabled,
                "requested feature flag is disabled",
            ))
        }
    }
}

impl ObservabilityPort for RecordingObservabilityPort {
    fn record_event(&self, event: ObservabilityEvent) -> Result<(), MethodLibraryError> {
        lock(&self.events)?.push(event);
        Ok(())
    }
}

#[async_trait]
impl GovernancePort for StaticGovernancePort {
    async fn validate_approved_gate(
        &self,
        gate_ref: ApprovedGateRef,
        _content_id: ContentId,
        _actor: ActorContext,
        _meta: RequestMeta,
    ) -> Result<GateValidationResult, MethodLibraryError> {
        if !self.approved {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishGateInvalid,
                "approved gate was rejected by the fake governance adapter",
            ));
        }

        Ok(GateValidationResult {
            approved: true,
            gate_ref,
            validated_at: self.validated_at,
        })
    }
}

#[async_trait]
impl BusPublisherPort for RecordingBusPublisher {
    async fn publish(
        &self,
        topic: Topic,
        event: DefinitionEventEnvelope,
        meta: RequestMeta,
    ) -> Result<PublishAck, MethodLibraryError> {
        if let Some(error) = lock(&self.fail_with)?.clone() {
            return Err(error);
        }

        lock(&self.published)?.push((topic.clone(), event.clone(), meta));
        Ok(PublishAck {
            topic,
            event_id: event.event_id,
            published_at: event.occurred_at,
        })
    }
}

#[async_trait]
impl ObjectStoragePort for InMemoryObjectStorage {
    async fn put_snapshot_payload(
        &self,
        payload: SnapshotPayload,
        object_key: super::ObjectKey,
        _meta: RequestMeta,
    ) -> Result<SnapshotBlobRef, MethodLibraryError> {
        let blob_ref = format!("object://{object_key}");
        lock(&self.blobs)?.insert(blob_ref.clone(), payload);
        Ok(blob_ref)
    }

    async fn get_snapshot_payload(
        &self,
        blob_ref: SnapshotBlobRef,
        _meta: RequestMeta,
    ) -> Result<SnapshotPayload, MethodLibraryError> {
        lock(&self.blobs)?.get(&blob_ref).cloned().ok_or_else(|| {
            MethodLibraryError::retryable(
                MethodLibraryErrorCode::ObjectStorageUnavailable,
                "snapshot payload blob is missing from fake object storage",
            )
        })
    }
}

#[async_trait]
impl MethodContentRepository for InMemoryMethodContentRepository {
    async fn get_for_update(
        &self,
        tx: &mut UnitOfWorkTx,
        content_id: ContentId,
    ) -> Result<Option<MethodContent>, MethodLibraryError> {
        tx.ensure_open()?;
        Ok(lock(&self.contents)?.get(&content_id).cloned())
    }

    async fn insert(
        &self,
        tx: &mut UnitOfWorkTx,
        content: MethodContent,
    ) -> Result<(), MethodLibraryError> {
        tx.ensure_open()?;
        let mut contents = lock(&self.contents)?;
        if contents.contains_key(&content.content_id) {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::ContentVersionConflict,
                "content already exists in the fake repository",
            ));
        }
        contents.insert(content.content_id.clone(), content);
        Ok(())
    }

    async fn save(
        &self,
        tx: &mut UnitOfWorkTx,
        content: MethodContent,
        expected_revision: Revision,
    ) -> Result<Revision, MethodLibraryError> {
        tx.ensure_open()?;
        let mut contents = lock(&self.contents)?;
        let stored = contents.get(&content.content_id).ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::MethodContentNotFound,
                "content does not exist in the fake repository",
            )
        })?;

        if stored.revision != expected_revision {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::RevisionConflict,
                "expected revision does not match the fake repository state",
            )
            .with_detail("expected_revision", expected_revision.to_string())
            .with_detail("actual_revision", stored.revision.to_string()));
        }

        contents.insert(content.content_id.clone(), content.clone());
        Ok(content.revision)
    }
}

#[async_trait]
impl MethodContentReferenceRepository for InMemoryMethodContentReferenceRepository {
    async fn replace_refs(
        &self,
        tx: &mut UnitOfWorkTx,
        source_content_id: ContentId,
        refs: Vec<ContentRef>,
    ) -> Result<(), MethodLibraryError> {
        tx.ensure_open()?;
        lock(&self.draft_refs)?.insert(source_content_id, refs);
        Ok(())
    }

    async fn replace_published_refs(
        &self,
        tx: &mut UnitOfWorkTx,
        source_content_id: ContentId,
        refs: Vec<PublishedContentRef>,
    ) -> Result<(), MethodLibraryError> {
        tx.ensure_open()?;
        lock(&self.published_refs)?.insert(source_content_id, refs);
        Ok(())
    }
}

#[async_trait]
impl MethodContentVersionRepository for InMemoryMethodContentVersionRepository {
    async fn insert(
        &self,
        tx: &mut UnitOfWorkTx,
        record: MethodContentVersionRecord,
    ) -> Result<(), MethodLibraryError> {
        tx.ensure_open()?;
        lock(&self.records)?.push(record);
        Ok(())
    }
}

#[async_trait]
impl SupersedeLinkRepository for InMemorySupersedeLinkRepository {
    async fn insert(
        &self,
        tx: &mut UnitOfWorkTx,
        link: SupersedeLink,
    ) -> Result<(), MethodLibraryError> {
        tx.ensure_open()?;
        lock(&self.links)?.push(link);
        Ok(())
    }
}

#[async_trait]
impl LifecycleHistoryRepository for InMemoryLifecycleHistoryRepository {
    async fn append(
        &self,
        tx: &mut UnitOfWorkTx,
        entry: LifecycleHistoryEntry,
    ) -> Result<(), MethodLibraryError> {
        tx.ensure_open()?;
        lock(&self.entries)?.push(entry);
        Ok(())
    }
}

#[async_trait]
impl AuditRepository for InMemoryAuditRepository {
    async fn append(
        &self,
        tx: &mut UnitOfWorkTx,
        record: AuditRecord,
    ) -> Result<(), MethodLibraryError> {
        tx.ensure_open()?;
        lock(&self.records)?.push(record);
        Ok(())
    }
}

#[async_trait]
impl IdempotencyRepository for InMemoryIdempotencyRepository {
    async fn try_begin(
        &self,
        tx: &mut UnitOfWorkTx,
        key: IdempotencyKey,
        scope: IdempotencyScope,
        request_hash: RequestHash,
        now: Timestamp,
    ) -> Result<IdempotencyBeginResult, MethodLibraryError> {
        tx.ensure_open()?;
        let mut records = lock(&self.records)?;
        let composite_key = (scope.clone(), key.clone());

        match records.get(&composite_key) {
            None => {
                records.insert(
                    composite_key,
                    IdempotencyRecord {
                        scope,
                        idempotency_key: key,
                        request_hash,
                        status: IdempotencyStatus::Processing,
                        result_ref: None,
                        failure_reason: None,
                        updated_at: now,
                    },
                );
                Ok(IdempotencyBeginResult::Started)
            }
            Some(existing) if existing.request_hash != request_hash => {
                Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::IdempotencyConflict,
                    "idempotency key was reused with a different request hash",
                ))
            }
            Some(existing) => Ok(match existing.status {
                IdempotencyStatus::Processing => IdempotencyBeginResult::Processing,
                IdempotencyStatus::Succeeded => IdempotencyBeginResult::Succeeded(
                    existing
                        .result_ref
                        .clone()
                        .expect("succeeded idempotency record should carry a result"),
                ),
                IdempotencyStatus::Failed => IdempotencyBeginResult::Failed(
                    existing
                        .failure_reason
                        .clone()
                        .expect("failed idempotency record should carry a failure reason"),
                ),
            }),
        }
    }

    async fn mark_completed(
        &self,
        tx: &mut UnitOfWorkTx,
        key: IdempotencyKey,
        scope: IdempotencyScope,
        result_ref: ResultRef,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        tx.ensure_open()?;
        let mut records = lock(&self.records)?;
        let record = records.get_mut(&(scope, key)).ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::IdempotencyStatusConflict,
                "idempotency record does not exist",
            )
        })?;

        if record.status != IdempotencyStatus::Processing {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::IdempotencyStatusConflict,
                "idempotency record is not in processing state",
            ));
        }

        record.status = IdempotencyStatus::Succeeded;
        record.result_ref = Some(result_ref);
        record.updated_at = now;
        Ok(())
    }

    async fn mark_failed(
        &self,
        tx: &mut UnitOfWorkTx,
        key: IdempotencyKey,
        scope: IdempotencyScope,
        reason: FailureReason,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        tx.ensure_open()?;
        let mut records = lock(&self.records)?;
        let record = records.get_mut(&(scope, key)).ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::IdempotencyStatusConflict,
                "idempotency record does not exist",
            )
        })?;

        if record.status != IdempotencyStatus::Processing {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::IdempotencyStatusConflict,
                "idempotency record is not in processing state",
            ));
        }

        record.status = IdempotencyStatus::Failed;
        record.failure_reason = Some(reason);
        record.updated_at = now;
        Ok(())
    }
}

#[async_trait]
impl OutboxRepository for InMemoryOutboxRepository {
    async fn append(
        &self,
        tx: &mut UnitOfWorkTx,
        event: OutboxEvent,
    ) -> Result<(), MethodLibraryError> {
        tx.ensure_open()?;
        let mut events = lock(&self.events)?;
        if events.contains_key(&event.event_id) {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxStatusConflict,
                "outbox event already exists in the fake repository",
            ));
        }
        events.insert(event.event_id.clone(), event);
        Ok(())
    }

    async fn claim_pending(
        &self,
        limit: super::BatchSize,
        worker_id: WorkerId,
        now: Timestamp,
        lease: LeaseDuration,
    ) -> Result<Vec<OutboxEvent>, MethodLibraryError> {
        let mut events = lock(&self.events)?;
        let mut claimable_ids = events
            .iter()
            .filter_map(|(event_id, event)| {
                let expired_publishing = event.status == OutboxStatus::Publishing
                    && event
                        .lease_until
                        .is_some_and(|lease_until| now >= lease_until);
                let retry_due = event.is_retry_due(now);
                if event.status == OutboxStatus::Pending || retry_due || expired_publishing {
                    Some(event_id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        claimable_ids.sort();
        claimable_ids.truncate(limit as usize);

        let mut claimed = Vec::with_capacity(claimable_ids.len());
        for event_id in claimable_ids {
            let event = events.get_mut(&event_id).expect("event should exist");
            event.mark_in_progress(worker_id.clone(), now, lease)?;
            claimed.push(event.clone());
        }

        Ok(claimed)
    }

    async fn mark_published(
        &self,
        event_id: OutboxEventId,
        worker_id: WorkerId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        let mut events = lock(&self.events)?;
        let event = events.get_mut(&event_id).ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxStatusConflict,
                "outbox event does not exist",
            )
        })?;
        event.mark_published(&worker_id, now)
    }

    async fn mark_retryable_failure(
        &self,
        event_id: OutboxEventId,
        worker_id: WorkerId,
        _reason: FailureReason,
        next_retry_at: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        let mut events = lock(&self.events)?;
        let event = events.get_mut(&event_id).ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxStatusConflict,
                "outbox event does not exist",
            )
        })?;
        event.mark_retryable_failure(&worker_id, next_retry_at)
    }

    async fn mark_dead_lettered(
        &self,
        event_id: OutboxEventId,
        worker_id: WorkerId,
        _reason: FailureReason,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        let mut events = lock(&self.events)?;
        let event = events.get_mut(&event_id).ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxStatusConflict,
                "outbox event does not exist",
            )
        })?;
        event.mark_dead_lettered(&worker_id, now)
    }
}

#[async_trait]
impl DefinitionSnapshotRepository for InMemoryDefinitionSnapshotRepository {
    async fn insert(
        &self,
        tx: &mut UnitOfWorkTx,
        snapshot: DefinitionSnapshot,
    ) -> Result<(), MethodLibraryError> {
        tx.ensure_open()?;
        lock(&self.snapshots)?.insert(snapshot.snapshot_id.clone(), snapshot);
        Ok(())
    }

    async fn get(
        &self,
        snapshot_id: SnapshotId,
    ) -> Result<Option<DefinitionSnapshot>, MethodLibraryError> {
        Ok(lock(&self.snapshots)?.get(&snapshot_id).cloned())
    }

    async fn get_by_content_version(
        &self,
        content_id: ContentId,
        version: ContentVersion,
    ) -> Result<Option<DefinitionSnapshot>, MethodLibraryError> {
        Ok(lock(&self.snapshots)?
            .values()
            .find(|snapshot| snapshot.content_id == content_id && snapshot.version == version)
            .cloned())
    }
}

#[async_trait]
impl ContentSummaryProjectionRepository for InMemoryContentSummaryProjectionRepository {
    async fn upsert(&self, view: ContentSummaryView) -> Result<(), MethodLibraryError> {
        lock(&self.views)?.insert(view.content_id.clone(), view);
        Ok(())
    }

    async fn list(
        &self,
        query: &ListMethodContentsQuery,
        page: &PageRequest,
    ) -> Result<Vec<ContentSummaryView>, MethodLibraryError> {
        let mut views = lock(&self.views)?
            .values()
            .filter(|view| query.kind.is_none_or(|kind| view.kind == kind))
            .filter(|view| {
                query
                    .lifecycle_state
                    .is_none_or(|state| view.lifecycle_state == state)
            })
            .cloned()
            .collect::<Vec<_>>();
        views.sort_by(|left, right| left.content_id.cmp(&right.content_id));

        if let Some(cursor) = &page.cursor {
            views.retain(|view| view.content_id > *cursor);
        }

        views.truncate(page.limit as usize);
        Ok(views)
    }
}

#[async_trait]
impl DefinitionTraceProjectionRepository for InMemoryDefinitionTraceProjectionRepository {
    async fn upsert(&self, view: DefinitionTraceView) -> Result<(), MethodLibraryError> {
        lock(&self.views)?.insert(view.content_id.clone(), view);
        Ok(())
    }

    async fn get(
        &self,
        content_id: ContentId,
    ) -> Result<Option<DefinitionTraceView>, MethodLibraryError> {
        Ok(lock(&self.views)?.get(&content_id).cloned())
    }
}

#[async_trait]
impl ProjectionCheckpointRepository for InMemoryProjectionCheckpointRepository {
    async fn advance_if_current(
        &self,
        name: CheckpointName,
        expected_cursor: Option<OutboxEventId>,
        next_cursor: OutboxEventId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        let mut checkpoints = lock(&self.checkpoints)?;
        match checkpoints.get_mut(&name) {
            Some(record) if record.last_processed_event_id == expected_cursor => {
                record.last_processed_event_id = Some(next_cursor);
                record.status = CheckpointStatus::Active;
                record.updated_at = now;
                Ok(())
            }
            Some(_) => Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::CheckpointConflict,
                "projection checkpoint compare-and-swap failed",
            )),
            None if expected_cursor.is_none() => {
                checkpoints.insert(
                    name.clone(),
                    ProjectionCheckpointRecord {
                        checkpoint_name: name,
                        last_processed_event_id: Some(next_cursor),
                        status: CheckpointStatus::Active,
                        updated_at: now,
                    },
                );
                Ok(())
            }
            None => Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::CheckpointConflict,
                "projection checkpoint does not exist for the expected cursor",
            )),
        }
    }

    async fn get(
        &self,
        name: CheckpointName,
    ) -> Result<Option<ProjectionCheckpointRecord>, MethodLibraryError> {
        Ok(lock(&self.checkpoints)?.get(&name).cloned())
    }
}

#[async_trait]
impl InboundDeadLetterRepository for InMemoryInboundDeadLetterRepository {
    async fn append(&self, record: InboundDeadLetter) -> Result<(), MethodLibraryError> {
        lock(&self.records)?.push(record);
        Ok(())
    }
}

#[async_trait]
impl JobRunRepository for InMemoryJobRunRepository {
    async fn start_once(
        &self,
        tx: &mut UnitOfWorkTx,
        job_name: JobName,
        scope_hash: String,
        key: IdempotencyKey,
        now: Timestamp,
    ) -> Result<JobRunStartResult, MethodLibraryError> {
        tx.ensure_open()?;
        let composite_key = (job_name.clone(), scope_hash.clone());
        let mut records = lock(&self.records)?;

        match records.get(&composite_key) {
            Some(record) if record.status == JobRunStatus::Running => {
                Ok(JobRunStartResult::AlreadyRunning(record.job_run_id.clone()))
            }
            Some(record) => Ok(JobRunStartResult::Existing(record.clone())),
            None => {
                let job_run_id = format!("{job_name}:{scope_hash}");
                let record = JobRunRecord {
                    job_run_id: job_run_id.clone(),
                    job_name,
                    scope_hash,
                    idempotency_key: key,
                    status: JobRunStatus::Running,
                    result: None,
                    started_at: now,
                    finished_at: None,
                };
                records.insert(composite_key, record);
                Ok(JobRunStartResult::Started(job_run_id))
            }
        }
    }

    async fn complete(
        &self,
        tx: &mut UnitOfWorkTx,
        job_run_id: JobRunId,
        result: JobResult,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        complete_job_run(
            &self.records,
            tx,
            job_run_id,
            JobRunStatus::Succeeded,
            result,
            now,
        )
    }

    async fn complete_with_partial_failure(
        &self,
        tx: &mut UnitOfWorkTx,
        job_run_id: JobRunId,
        result: JobResult,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        complete_job_run(
            &self.records,
            tx,
            job_run_id,
            JobRunStatus::PartiallySucceeded,
            result,
            now,
        )
    }

    async fn fail(
        &self,
        tx: &mut UnitOfWorkTx,
        job_run_id: JobRunId,
        reason: FailureReason,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        complete_job_run(
            &self.records,
            tx,
            job_run_id,
            JobRunStatus::Failed,
            serde_json::json!({
                "code": reason.code.as_str(),
                "message": reason.message,
                "retryable": reason.retryable,
            }),
            now,
        )
    }
}

fn complete_job_run(
    records: &Arc<Mutex<HashMap<(JobName, String), JobRunRecord>>>,
    tx: &mut UnitOfWorkTx,
    job_run_id: JobRunId,
    status: JobRunStatus,
    result: JobResult,
    now: Timestamp,
) -> Result<(), MethodLibraryError> {
    tx.ensure_open()?;
    let mut records = lock(records)?;
    let record = records
        .values_mut()
        .find(|record| record.job_run_id == job_run_id)
        .ok_or_else(|| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::JobStatusConflict,
                "job run does not exist in the fake repository",
            )
        })?;

    if record.status != JobRunStatus::Running {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobStatusConflict,
            "job run is not running",
        ));
    }

    record.status = status;
    record.result = Some(result);
    record.finished_at = Some(now);
    Ok(())
}

fn next_id(
    counters: &Mutex<HashMap<&'static str, u64>>,
    bucket: &'static str,
    prefix: &'static str,
) -> String {
    let mut counters = counters
        .lock()
        .expect("deterministic id generator lock should not poison");
    let counter = counters.entry(bucket).or_insert(0);
    *counter += 1;
    format!("{prefix}-{}", counter)
}

fn lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, MethodLibraryError> {
    mutex.lock().map_err(|_| {
        MethodLibraryError::retryable(
            MethodLibraryErrorCode::PersistenceUnavailable,
            "in-memory fake adapter lock was poisoned",
        )
    })
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::{
        DeterministicClock, FakeUnitOfWork, InMemoryIdempotencyRepository,
        InMemoryMethodContentRepository, InMemoryOutboxRepository,
        InMemoryProjectionCheckpointRepository, OutboxEvent, OutboxRepository,
        ProjectionCheckpointRepository, UnitOfWork,
    };
    use crate::ports::{
        Clock, IdempotencyBeginResult, IdempotencyRepository, MethodContentRepository,
    };
    use method_library_contracts::{
        ContentPublishedPayload, DefinitionEventEnvelope, DefinitionEventPayload,
        DefinitionEventType, EventTraceContext, RequestMeta,
    };
    use method_library_domain::MethodLibraryErrorCode;
    use method_library_domain::content::{
        ApprovedGateRef, CanonicalFingerprint, ContentVersion, FingerprintAlgorithm, MethodContent,
        MethodContentKind,
    };
    use method_library_domain::definitions::{
        EvidenceKind, EvidenceRule, MethodContentPayload, Qualification, QualificationLevel,
        QualificationLevelModel,
    };

    fn sample_meta() -> RequestMeta {
        RequestMeta {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            idempotency_key: Some("idem-1".to_string()),
            request_hash: "hash-1".to_string(),
            received_at: datetime!(2026-05-26 08:00:00 UTC),
        }
    }

    fn sample_content() -> MethodContent {
        MethodContent::create_draft(
            "content-1".to_string(),
            "family-1".to_string(),
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
            "actor-1".to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("content should be valid")
    }

    fn sample_event() -> OutboxEvent {
        OutboxEvent::new_pending(
            "evt-1".to_string(),
            "content-1".to_string(),
            DefinitionEventEnvelope {
                event_id: "evt-1".to_string(),
                event_type: DefinitionEventType::ContentPublished,
                schema_version: "1.0".to_string(),
                occurred_at: datetime!(2026-05-26 09:00:00 UTC),
                producer: "L3-method-library".to_string(),
                content_ref: method_library_domain::content::PublishedContentRef {
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
                    request_id: "req-1".to_string(),
                    trace_id: "trace-1".to_string(),
                },
                payload: DefinitionEventPayload::ContentPublished(ContentPublishedPayload {
                    gate_ref: ApprovedGateRef {
                        gate_id: "gate-1".to_string(),
                        gate_decision_id: "decision-1".to_string(),
                        approved_at: datetime!(2026-05-26 08:30:00 UTC),
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
            "payload-hash-1".to_string(),
            Some("idem-1".to_string()),
        )
        .expect("outbox event should be valid")
    }

    #[tokio::test]
    async fn fake_unit_of_work_tracks_commit_state() {
        let unit_of_work = FakeUnitOfWork;
        let mut tx = unit_of_work
            .begin(sample_meta())
            .await
            .expect("transaction should begin");

        tx.commit().await.expect("transaction should commit");

        assert_eq!(tx.state, crate::ports::TransactionState::Committed);
    }

    #[tokio::test]
    async fn in_memory_method_content_repository_detects_revision_conflicts() {
        let repository = InMemoryMethodContentRepository::default();
        let unit_of_work = FakeUnitOfWork;
        let mut tx = unit_of_work
            .begin(sample_meta())
            .await
            .expect("transaction should begin");
        let mut content = sample_content();
        repository
            .insert(&mut tx, content.clone())
            .await
            .expect("content should insert");

        content.revision = 2;
        let error = repository
            .save(&mut tx, content, 2)
            .await
            .expect_err("stale expected revision should fail");

        assert_eq!(error.code, MethodLibraryErrorCode::RevisionConflict);
    }

    #[tokio::test]
    async fn in_memory_idempotency_repository_reuses_results_and_detects_conflicts() {
        let repository = InMemoryIdempotencyRepository::default();
        let unit_of_work = FakeUnitOfWork;
        let mut tx = unit_of_work
            .begin(sample_meta())
            .await
            .expect("transaction should begin");

        let started = repository
            .try_begin(
                &mut tx,
                "idem-1".to_string(),
                "command:create_draft".to_string(),
                "hash-1".to_string(),
                datetime!(2026-05-26 08:00:00 UTC),
            )
            .await
            .expect("idempotency record should start");
        assert_eq!(started, IdempotencyBeginResult::Started);

        repository
            .mark_completed(
                &mut tx,
                "idem-1".to_string(),
                "command:create_draft".to_string(),
                "result-1".to_string(),
                datetime!(2026-05-26 08:01:00 UTC),
            )
            .await
            .expect("idempotency record should complete");

        let reused = repository
            .try_begin(
                &mut tx,
                "idem-1".to_string(),
                "command:create_draft".to_string(),
                "hash-1".to_string(),
                datetime!(2026-05-26 08:02:00 UTC),
            )
            .await
            .expect("same key and hash should reuse the result");
        assert_eq!(
            reused,
            IdempotencyBeginResult::Succeeded("result-1".to_string())
        );

        let error = repository
            .try_begin(
                &mut tx,
                "idem-1".to_string(),
                "command:create_draft".to_string(),
                "different-hash".to_string(),
                datetime!(2026-05-26 08:03:00 UTC),
            )
            .await
            .expect_err("same key with different hash should fail");
        assert_eq!(error.code, MethodLibraryErrorCode::IdempotencyConflict);
    }

    #[tokio::test]
    async fn in_memory_outbox_repository_claims_and_completes_events() {
        let repository = InMemoryOutboxRepository::default();
        let unit_of_work = FakeUnitOfWork;
        let mut tx = unit_of_work
            .begin(sample_meta())
            .await
            .expect("transaction should begin");
        repository
            .append(&mut tx, sample_event())
            .await
            .expect("outbox event should append");

        let claimed = repository
            .claim_pending(
                10,
                "worker-1".to_string(),
                datetime!(2026-05-26 09:10:00 UTC),
                time::Duration::minutes(5),
            )
            .await
            .expect("event should be claimed");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].status, crate::ports::OutboxStatus::Publishing);

        repository
            .mark_published(
                "evt-1".to_string(),
                "worker-1".to_string(),
                datetime!(2026-05-26 09:11:00 UTC),
            )
            .await
            .expect("worker owner should mark publish success");

        let stored = repository.events().expect("events should be readable");
        assert_eq!(
            stored.get("evt-1").expect("event should exist").status,
            crate::ports::OutboxStatus::Published
        );
    }

    #[tokio::test]
    async fn in_memory_projection_checkpoint_repository_uses_compare_and_swap() {
        let repository = InMemoryProjectionCheckpointRepository::default();

        repository
            .advance_if_current(
                "summary".to_string(),
                None,
                "evt-1".to_string(),
                datetime!(2026-05-26 09:00:00 UTC),
            )
            .await
            .expect("first checkpoint should create the record");

        let error = repository
            .advance_if_current(
                "summary".to_string(),
                Some("evt-stale".to_string()),
                "evt-2".to_string(),
                datetime!(2026-05-26 09:01:00 UTC),
            )
            .await
            .expect_err("stale cursor should fail compare-and-swap");

        assert_eq!(error.code, MethodLibraryErrorCode::CheckpointConflict);
    }

    #[test]
    fn deterministic_clock_returns_a_fixed_instant() {
        let now = datetime!(2026-05-26 10:00:00 UTC);
        let clock = DeterministicClock::new(now);

        assert_eq!(clock.now(), now);
    }
}
