//! Application-facing ports, transaction tokens, and durable record models.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use method_library_contracts::{
    ActorContext, ContentSummaryView, DefinitionEventEnvelope, DefinitionSnapshot,
    DefinitionTraceView, ListMethodContentsQuery, RequestMeta, SnapshotBlobRef, SnapshotPayload,
};
use method_library_domain::content::{
    ApprovedGateRef, BatchSize, CanonicalBytes, CanonicalFingerprint, ContentFamilyId, ContentId,
    ContentRef, ContentVersion, FingerprintAlgorithm, IdempotencyKey, JobName, JobRunId,
    LeaseDuration, LifecycleState, MethodContent, OutboxEventId, PublishedContentRef, RequestHash,
    RequestId, Revision, SnapshotId, Timestamp, TraceId, WorkerId,
};
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};

pub mod fakes;

/// Stable write-model version-record identifier.
pub type ContentVersionRecordId = String;
/// Stable lifecycle-history identifier.
pub type HistoryEntryId = String;
/// Stable audit-record identifier.
pub type AuditId = String;
/// Stable checkpoint name.
pub type CheckpointName = String;
/// Stable dead-letter identifier.
pub type DeadLetterId = String;
/// Stable result reference used by idempotency records.
pub type ResultRef = String;
/// Stable payload hash used by outbox records.
pub type PayloadHash = String;
/// Stable object-storage key.
pub type ObjectKey = String;
/// Stable idempotency scope.
pub type IdempotencyScope = String;
/// Stable audit action label.
pub type AuditAction = String;
/// Stable audit result label.
pub type AuditResult = String;
/// Stable bus topic label.
pub type Topic = String;

/// Current lifecycle state of an outbox record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboxStatus {
    /// The event is waiting to be claimed by a worker.
    Pending,
    /// The event has been claimed and is currently being published.
    Publishing,
    /// The event was published successfully.
    Published,
    /// The event failed in a retryable way.
    RetryableFailed,
    /// The event was moved to dead letter.
    DeadLettered,
}

/// Current state of an idempotency record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyStatus {
    /// The request is still being processed.
    Processing,
    /// The request succeeded and has a stable result reference.
    Succeeded,
    /// The request failed and recorded a reason.
    Failed,
}

/// Current state of an operations job run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobRunStatus {
    /// The job is running.
    Running,
    /// The job succeeded completely.
    Succeeded,
    /// The job partially succeeded.
    PartiallySucceeded,
    /// The job failed.
    Failed,
}

/// Current state of a projection checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointStatus {
    /// The checkpoint is active.
    Active,
    /// The checkpoint is being rebuilt.
    Rebuilding,
    /// The checkpoint is in a failed state.
    Failed,
}

/// Feature flags reserved for optional P1 capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureFlag {
    /// P1 plugin support.
    P1Plugin,
    /// P1 configuration support.
    P1Configuration,
}

/// Shared failure reason used by durable records and fake adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureReason {
    /// Stable error code.
    pub code: MethodLibraryErrorCode,
    /// Human-readable error message.
    pub message: String,
    /// Whether the failure is retryable.
    pub retryable: bool,
}

/// Driver used by a unit-of-work token to commit or roll back the backing transaction.
#[async_trait]
pub trait TransactionDriver: Send + Sync {
    /// Commits the backing transaction identified by the request id.
    async fn commit(&self, request_id: &RequestId) -> Result<(), MethodLibraryError>;

    /// Rolls back the backing transaction identified by the request id.
    async fn rollback(&self, request_id: &RequestId) -> Result<(), MethodLibraryError>;
}

/// Transaction token passed to repository ports.
pub struct UnitOfWorkTx {
    /// Request metadata that opened the transaction.
    pub request_meta: RequestMeta,
    /// Whether the transaction is still open.
    pub state: TransactionState,
    driver: Arc<dyn TransactionDriver>,
}

/// State of a unit-of-work transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionState {
    /// The transaction is open.
    Open,
    /// The transaction committed.
    Committed,
    /// The transaction rolled back.
    RolledBack,
}

/// Persisted version-history record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodContentVersionRecord {
    /// Version-record identifier.
    pub content_version_id: ContentVersionRecordId,
    /// Content identifier.
    pub content_id: ContentId,
    /// Content family identifier.
    pub content_family_id: ContentFamilyId,
    /// Published version.
    pub version: ContentVersion,
    /// Published fingerprint.
    pub fingerprint: CanonicalFingerprint,
    /// Snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Publish timestamp.
    pub published_at: Timestamp,
}

/// Persisted supersede-link record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupersedeLink {
    /// Supersede-link identifier.
    pub supersede_link_id: String,
    /// Old content identifier.
    pub old_content_id: ContentId,
    /// New content identifier.
    pub new_content_id: ContentId,
    /// Shared content family identifier for the replacement chain.
    pub content_family_id: ContentFamilyId,
    /// Human-readable reason.
    pub reason: String,
    /// Creation timestamp.
    pub created_at: Timestamp,
}

/// Persisted lifecycle-history record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleHistoryEntry {
    /// History-entry identifier.
    pub history_entry_id: HistoryEntryId,
    /// Content identifier.
    pub content_id: ContentId,
    /// Previous lifecycle state.
    pub from_state: Option<LifecycleState>,
    /// New lifecycle state.
    pub to_state: LifecycleState,
    /// Actor identifier.
    pub actor_id: String,
    /// Optional reason.
    pub reason: Option<String>,
    /// Creation timestamp.
    pub created_at: Timestamp,
}

/// Audit target reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditTargetRef {
    /// Logical target type.
    pub target_type: String,
    /// Logical target identifier.
    pub target_id: String,
}

/// Persisted audit record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Audit-record identifier.
    pub audit_id: AuditId,
    /// Request identifier.
    pub request_id: RequestId,
    /// Trace identifier.
    pub trace_id: TraceId,
    /// Actor context at the time of the action.
    pub actor_context: ActorContext,
    /// Target reference.
    pub target_ref: AuditTargetRef,
    /// Audit action label.
    pub action: AuditAction,
    /// Audit result label.
    pub result: AuditResult,
    /// Structured details safe for audit output.
    pub details: BTreeMap<String, String>,
    /// Occurrence timestamp.
    pub occurred_at: Timestamp,
}

/// Persisted idempotency record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdempotencyRecord {
    /// Scope of the request.
    pub scope: IdempotencyScope,
    /// Stable idempotency key.
    pub idempotency_key: IdempotencyKey,
    /// Canonical request hash.
    pub request_hash: RequestHash,
    /// Current idempotency status.
    pub status: IdempotencyStatus,
    /// Optional stable result reference.
    pub result_ref: Option<ResultRef>,
    /// Optional failure reason.
    pub failure_reason: Option<FailureReason>,
    /// Last update timestamp.
    pub updated_at: Timestamp,
}

/// Result of attempting to begin an idempotent request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdempotencyBeginResult {
    /// The request may proceed and owns the key.
    Started,
    /// The request already succeeded and has a reusable result reference.
    Succeeded(ResultRef),
    /// The request already failed.
    Failed(FailureReason),
    /// The request is still processing.
    Processing,
}

/// Persisted outbox record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxEvent {
    /// Outbox-event identifier.
    pub event_id: OutboxEventId,
    /// Source aggregate identifier.
    pub aggregate_id: ContentId,
    /// Typed event envelope.
    pub envelope: DefinitionEventEnvelope,
    /// Stable payload hash.
    pub payload_hash: PayloadHash,
    /// Current outbox status.
    pub status: OutboxStatus,
    /// Number of retry attempts.
    pub retry_count: u32,
    /// Optional next-retry timestamp.
    pub next_retry_at: Option<Timestamp>,
    /// Worker currently holding the lease.
    pub worker_id: Option<WorkerId>,
    /// Lease expiry timestamp.
    pub lease_until: Option<Timestamp>,
    /// Successful publish timestamp.
    pub published_at: Option<Timestamp>,
    /// Optional originating idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Projection checkpoint record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionCheckpointRecord {
    /// Checkpoint name.
    pub checkpoint_name: CheckpointName,
    /// Last processed event identifier.
    pub last_processed_event_id: Option<OutboxEventId>,
    /// Checkpoint status.
    pub status: CheckpointStatus,
    /// Last update timestamp.
    pub updated_at: Timestamp,
}

/// Persisted inbound dead-letter record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboundDeadLetter {
    /// Dead-letter identifier.
    pub dead_letter_id: DeadLetterId,
    /// Source module label.
    pub source_module: String,
    /// Source event type.
    pub event_type: String,
    /// Raw payload snapshot.
    pub payload: JsonValue,
    /// Failure reason.
    pub failure_reason: FailureReason,
    /// Creation timestamp.
    pub created_at: Timestamp,
}

/// Generic persisted job result payload.
pub type JobResult = JsonValue;

/// Persisted job-run record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRunRecord {
    /// Job-run identifier.
    pub job_run_id: JobRunId,
    /// Job name.
    pub job_name: JobName,
    /// Scope hash.
    pub scope_hash: String,
    /// Idempotency key.
    pub idempotency_key: IdempotencyKey,
    /// Job-run status.
    pub status: JobRunStatus,
    /// Optional result payload.
    pub result: Option<JobResult>,
    /// Start timestamp.
    pub started_at: Timestamp,
    /// Finish timestamp.
    pub finished_at: Option<Timestamp>,
}

/// Result of attempting to start a job run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobRunStartResult {
    /// A new job run was started.
    Started(JobRunId),
    /// A matching job run is already running.
    AlreadyRunning(JobRunId),
    /// A matching job run already completed or failed.
    Existing(JobRunRecord),
}

/// Result of validating an approved gate reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateValidationResult {
    /// Whether the gate was approved.
    pub approved: bool,
    /// Approved gate reference.
    pub gate_ref: ApprovedGateRef,
    /// Validation timestamp.
    pub validated_at: Timestamp,
}

/// Result of successfully publishing an event to the bus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishAck {
    /// Published topic.
    pub topic: Topic,
    /// Published event identifier.
    pub event_id: OutboxEventId,
    /// Publish timestamp.
    pub published_at: Timestamp,
}

/// Observability event emitted by application services and workers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityEvent {
    /// Event name.
    pub name: String,
    /// Trace identifier.
    pub trace_id: Option<TraceId>,
    /// Structured attributes.
    pub attributes: BTreeMap<String, String>,
}

/// Simple page request used by projection repositories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageRequest {
    /// Optional cursor.
    pub cursor: Option<String>,
    /// Maximum page size.
    pub limit: u32,
}

impl UnitOfWorkTx {
    /// Creates a new open transaction token.
    #[must_use]
    pub fn new(request_meta: RequestMeta, driver: Arc<dyn TransactionDriver>) -> Self {
        Self {
            request_meta,
            state: TransactionState::Open,
            driver,
        }
    }

    /// Commits the transaction token.
    pub async fn commit(&mut self) -> Result<(), MethodLibraryError> {
        self.ensure_open()?;
        self.driver.commit(&self.request_meta.request_id).await?;
        self.state = TransactionState::Committed;
        Ok(())
    }

    /// Rolls back the transaction token.
    pub async fn rollback(&mut self) -> Result<(), MethodLibraryError> {
        self.ensure_open()?;
        self.driver.rollback(&self.request_meta.request_id).await?;
        self.state = TransactionState::RolledBack;
        Ok(())
    }

    /// Ensures the transaction is still open.
    pub fn ensure_open(&self) -> Result<(), MethodLibraryError> {
        if self.state != TransactionState::Open {
            return Err(MethodLibraryError::retryable(
                MethodLibraryErrorCode::TransactionCommitFailed,
                "unit-of-work transaction is no longer open",
            ));
        }

        Ok(())
    }

    /// Returns the request identifier associated with the transaction.
    #[must_use]
    pub fn request_id(&self) -> &RequestId {
        &self.request_meta.request_id
    }
}

impl std::fmt::Debug for UnitOfWorkTx {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UnitOfWorkTx")
            .field("request_id", &self.request_meta.request_id)
            .field("state", &self.state)
            .finish()
    }
}

impl FailureReason {
    /// Builds a failure reason from a domain error.
    #[must_use]
    pub fn from_error(error: &MethodLibraryError) -> Self {
        Self {
            code: error.code,
            message: error.message.clone(),
            retryable: error.retryable,
        }
    }
}

impl OutboxEvent {
    /// Creates a new pending outbox record.
    pub fn new_pending(
        event_id: OutboxEventId,
        aggregate_id: ContentId,
        envelope: DefinitionEventEnvelope,
        payload_hash: PayloadHash,
        idempotency_key: Option<IdempotencyKey>,
    ) -> Result<Self, MethodLibraryError> {
        if event_id.trim().is_empty()
            || aggregate_id.trim().is_empty()
            || payload_hash.trim().is_empty()
        {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxEventInvalid,
                "pending outbox events require identifiers and a payload hash",
            ));
        }

        Ok(Self {
            event_id,
            aggregate_id,
            envelope,
            payload_hash,
            status: OutboxStatus::Pending,
            retry_count: 0,
            next_retry_at: None,
            worker_id: None,
            lease_until: None,
            published_at: None,
            idempotency_key,
        })
    }

    /// Marks the outbox event as claimed by a worker.
    pub fn mark_in_progress(
        &mut self,
        worker_id: WorkerId,
        now: Timestamp,
        lease: LeaseDuration,
    ) -> Result<(), MethodLibraryError> {
        if !matches!(
            self.status,
            OutboxStatus::Pending | OutboxStatus::RetryableFailed
        ) {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxStatusConflict,
                "outbox event is not claimable",
            ));
        }

        self.status = OutboxStatus::Publishing;
        self.worker_id = Some(worker_id);
        self.lease_until = Some(now + lease);
        self.next_retry_at = None;
        Ok(())
    }

    /// Marks the outbox event as published.
    pub fn mark_published(
        &mut self,
        worker_id: &WorkerId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        self.ensure_worker_owner(worker_id)?;
        if self.status != OutboxStatus::Publishing {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxStatusConflict,
                "only publishing events may be marked published",
            ));
        }

        self.status = OutboxStatus::Published;
        self.published_at = Some(now);
        self.lease_until = None;
        Ok(())
    }

    /// Marks the outbox event as retryable failed.
    pub fn mark_retryable_failure(
        &mut self,
        worker_id: &WorkerId,
        next_retry_at: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        self.ensure_worker_owner(worker_id)?;
        if self.status != OutboxStatus::Publishing {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxStatusConflict,
                "only publishing events may be marked retryable failed",
            ));
        }

        self.status = OutboxStatus::RetryableFailed;
        self.retry_count += 1;
        self.next_retry_at = Some(next_retry_at);
        self.lease_until = None;
        Ok(())
    }

    /// Marks the outbox event as dead-lettered.
    pub fn mark_dead_lettered(
        &mut self,
        worker_id: &WorkerId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        self.ensure_worker_owner(worker_id)?;
        if !matches!(
            self.status,
            OutboxStatus::Publishing | OutboxStatus::RetryableFailed
        ) {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxStatusConflict,
                "only publishing or retryable events may be dead-lettered",
            ));
        }

        self.status = OutboxStatus::DeadLettered;
        self.published_at = Some(now);
        self.lease_until = None;
        Ok(())
    }

    /// Returns whether the event is due for retry.
    #[must_use]
    pub fn is_retry_due(&self, now: Timestamp) -> bool {
        matches!(self.status, OutboxStatus::RetryableFailed)
            && self.next_retry_at.is_some_and(|due_at| now >= due_at)
    }

    fn ensure_worker_owner(&self, worker_id: &WorkerId) -> Result<(), MethodLibraryError> {
        if self.worker_id.as_ref() != Some(worker_id) {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxStatusConflict,
                "worker does not hold the outbox lease",
            ));
        }

        Ok(())
    }
}

/// Transaction boundary used by application services.
#[async_trait]
pub trait UnitOfWork: Send + Sync {
    /// Opens a new transaction bound to request metadata.
    async fn begin(&self, meta: RequestMeta) -> Result<UnitOfWorkTx, MethodLibraryError>;
}

/// Write-model repository for method-content aggregates.
#[async_trait]
pub trait MethodContentRepository: Send + Sync {
    /// Reads one aggregate for update.
    async fn get_for_update(
        &self,
        tx: &mut UnitOfWorkTx,
        content_id: ContentId,
    ) -> Result<Option<MethodContent>, MethodLibraryError>;

    /// Inserts a new aggregate.
    async fn insert(
        &self,
        tx: &mut UnitOfWorkTx,
        content: MethodContent,
    ) -> Result<(), MethodLibraryError>;

    /// Saves an existing aggregate guarded by an expected revision.
    async fn save(
        &self,
        tx: &mut UnitOfWorkTx,
        content: MethodContent,
        expected_revision: Revision,
    ) -> Result<Revision, MethodLibraryError>;
}

/// Repository for draft and published references.
#[async_trait]
pub trait MethodContentReferenceRepository: Send + Sync {
    /// Replaces draft references for a source content aggregate.
    async fn replace_refs(
        &self,
        tx: &mut UnitOfWorkTx,
        source_content_id: ContentId,
        refs: Vec<ContentRef>,
    ) -> Result<(), MethodLibraryError>;

    /// Replaces published references for a source content aggregate.
    async fn replace_published_refs(
        &self,
        tx: &mut UnitOfWorkTx,
        source_content_id: ContentId,
        refs: Vec<PublishedContentRef>,
    ) -> Result<(), MethodLibraryError>;
}

/// Repository for version-history records.
#[async_trait]
pub trait MethodContentVersionRepository: Send + Sync {
    /// Inserts a version-history record.
    async fn insert(
        &self,
        tx: &mut UnitOfWorkTx,
        record: MethodContentVersionRecord,
    ) -> Result<(), MethodLibraryError>;
}

/// Repository for supersede links.
#[async_trait]
pub trait SupersedeLinkRepository: Send + Sync {
    /// Inserts a supersede link.
    async fn insert(
        &self,
        tx: &mut UnitOfWorkTx,
        link: SupersedeLink,
    ) -> Result<(), MethodLibraryError>;
}

/// Repository for lifecycle history.
#[async_trait]
pub trait LifecycleHistoryRepository: Send + Sync {
    /// Appends a lifecycle-history entry.
    async fn append(
        &self,
        tx: &mut UnitOfWorkTx,
        entry: LifecycleHistoryEntry,
    ) -> Result<(), MethodLibraryError>;
}

/// Repository for audit records.
#[async_trait]
pub trait AuditRepository: Send + Sync {
    /// Appends an audit record.
    async fn append(
        &self,
        tx: &mut UnitOfWorkTx,
        record: AuditRecord,
    ) -> Result<(), MethodLibraryError>;
}

/// Repository for request idempotency.
#[async_trait]
pub trait IdempotencyRepository: Send + Sync {
    /// Attempts to start a new idempotent request.
    async fn try_begin(
        &self,
        tx: &mut UnitOfWorkTx,
        key: IdempotencyKey,
        scope: IdempotencyScope,
        request_hash: RequestHash,
        now: Timestamp,
    ) -> Result<IdempotencyBeginResult, MethodLibraryError>;

    /// Marks a request as completed.
    async fn mark_completed(
        &self,
        tx: &mut UnitOfWorkTx,
        key: IdempotencyKey,
        scope: IdempotencyScope,
        result_ref: ResultRef,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError>;

    /// Marks a request as failed.
    async fn mark_failed(
        &self,
        tx: &mut UnitOfWorkTx,
        key: IdempotencyKey,
        scope: IdempotencyScope,
        reason: FailureReason,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError>;
}

/// Repository for reliable outbox events.
#[async_trait]
pub trait OutboxRepository: Send + Sync {
    /// Appends a new outbox record inside a write transaction.
    async fn append(
        &self,
        tx: &mut UnitOfWorkTx,
        event: OutboxEvent,
    ) -> Result<(), MethodLibraryError>;

    /// Claims claimable outbox events for a worker.
    async fn claim_pending(
        &self,
        limit: BatchSize,
        worker_id: WorkerId,
        now: Timestamp,
        lease: LeaseDuration,
    ) -> Result<Vec<OutboxEvent>, MethodLibraryError>;

    /// Marks an outbox event as published.
    async fn mark_published(
        &self,
        event_id: OutboxEventId,
        worker_id: WorkerId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError>;

    /// Marks an outbox event as retryable failed.
    async fn mark_retryable_failure(
        &self,
        event_id: OutboxEventId,
        worker_id: WorkerId,
        reason: FailureReason,
        next_retry_at: Timestamp,
    ) -> Result<(), MethodLibraryError>;

    /// Marks an outbox event as dead-lettered.
    async fn mark_dead_lettered(
        &self,
        event_id: OutboxEventId,
        worker_id: WorkerId,
        reason: FailureReason,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError>;
}

/// Repository for snapshot metadata.
#[async_trait]
pub trait DefinitionSnapshotRepository: Send + Sync {
    /// Inserts snapshot metadata.
    async fn insert(
        &self,
        tx: &mut UnitOfWorkTx,
        snapshot: DefinitionSnapshot,
    ) -> Result<(), MethodLibraryError>;

    /// Reads one snapshot by identifier.
    async fn get(
        &self,
        snapshot_id: SnapshotId,
    ) -> Result<Option<DefinitionSnapshot>, MethodLibraryError>;

    /// Reads one snapshot by content and version.
    async fn get_by_content_version(
        &self,
        content_id: ContentId,
        version: ContentVersion,
    ) -> Result<Option<DefinitionSnapshot>, MethodLibraryError>;
}

/// Repository for content-summary projections.
#[async_trait]
pub trait ContentSummaryProjectionRepository: Send + Sync {
    /// Upserts one content-summary view.
    async fn upsert(&self, view: ContentSummaryView) -> Result<(), MethodLibraryError>;

    /// Lists content-summary views by query and page request.
    async fn list(
        &self,
        query: &ListMethodContentsQuery,
        page: &PageRequest,
    ) -> Result<Vec<ContentSummaryView>, MethodLibraryError>;
}

/// Repository for definition-trace projections.
#[async_trait]
pub trait DefinitionTraceProjectionRepository: Send + Sync {
    /// Upserts one definition-trace view.
    async fn upsert(&self, view: DefinitionTraceView) -> Result<(), MethodLibraryError>;

    /// Reads one definition-trace view.
    async fn get(
        &self,
        content_id: ContentId,
    ) -> Result<Option<DefinitionTraceView>, MethodLibraryError>;
}

/// Repository for projection checkpoints.
#[async_trait]
pub trait ProjectionCheckpointRepository: Send + Sync {
    /// Advances a checkpoint if the expected cursor still matches.
    async fn advance_if_current(
        &self,
        name: CheckpointName,
        expected_cursor: Option<OutboxEventId>,
        next_cursor: OutboxEventId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError>;

    /// Reads one checkpoint.
    async fn get(
        &self,
        name: CheckpointName,
    ) -> Result<Option<ProjectionCheckpointRecord>, MethodLibraryError>;
}

/// Repository for inbound dead letters.
#[async_trait]
pub trait InboundDeadLetterRepository: Send + Sync {
    /// Appends an inbound dead-letter record.
    async fn append(&self, record: InboundDeadLetter) -> Result<(), MethodLibraryError>;
}

/// Repository for job-run records.
#[async_trait]
pub trait JobRunRepository: Send + Sync {
    /// Starts a job run if one does not already exist for the same scope.
    async fn start_once(
        &self,
        tx: &mut UnitOfWorkTx,
        job_name: JobName,
        scope_hash: String,
        key: IdempotencyKey,
        now: Timestamp,
    ) -> Result<JobRunStartResult, MethodLibraryError>;

    /// Marks a job run as succeeded.
    async fn complete(
        &self,
        tx: &mut UnitOfWorkTx,
        job_run_id: JobRunId,
        result: JobResult,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError>;

    /// Marks a job run as partially succeeded.
    async fn complete_with_partial_failure(
        &self,
        tx: &mut UnitOfWorkTx,
        job_run_id: JobRunId,
        result: JobResult,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError>;

    /// Marks a job run as failed.
    async fn fail(
        &self,
        tx: &mut UnitOfWorkTx,
        job_run_id: JobRunId,
        reason: FailureReason,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError>;
}

/// Outbound governance validation port.
#[async_trait]
pub trait GovernancePort: Send + Sync {
    /// Validates an approved gate reference.
    async fn validate_approved_gate(
        &self,
        gate_ref: ApprovedGateRef,
        content_id: ContentId,
        actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<GateValidationResult, MethodLibraryError>;
}

/// Outbound event-bus publishing port.
#[async_trait]
pub trait BusPublisherPort: Send + Sync {
    /// Publishes a definition event to a topic.
    async fn publish(
        &self,
        topic: Topic,
        event: DefinitionEventEnvelope,
        meta: RequestMeta,
    ) -> Result<PublishAck, MethodLibraryError>;
}

/// Outbound object-storage port.
#[async_trait]
pub trait ObjectStoragePort: Send + Sync {
    /// Stores a snapshot payload and returns a blob reference.
    async fn put_snapshot_payload(
        &self,
        payload: SnapshotPayload,
        object_key: ObjectKey,
        meta: RequestMeta,
    ) -> Result<SnapshotBlobRef, MethodLibraryError>;

    /// Reads a snapshot payload by blob reference.
    async fn get_snapshot_payload(
        &self,
        blob_ref: SnapshotBlobRef,
        meta: RequestMeta,
    ) -> Result<SnapshotPayload, MethodLibraryError>;
}

/// Clock port for deterministic time.
pub trait Clock: Send + Sync {
    /// Returns the current time.
    fn now(&self) -> Timestamp;
}

/// ID generation port.
pub trait IdGenerator: Send + Sync {
    /// Generates a new content identifier.
    fn new_content_id(&self) -> ContentId;
    /// Generates a new content family identifier.
    fn new_content_family_id(&self) -> ContentFamilyId;
    /// Generates a new outbox-event identifier.
    fn new_outbox_event_id(&self) -> OutboxEventId;
    /// Generates a new snapshot identifier.
    fn new_snapshot_id(&self) -> SnapshotId;
    /// Generates a new job-run identifier.
    fn new_job_run_id(&self) -> JobRunId;
    /// Generates a new version-record identifier.
    fn new_content_version_record_id(&self) -> ContentVersionRecordId;
    /// Generates a new lifecycle-history identifier.
    fn new_history_entry_id(&self) -> HistoryEntryId;
    /// Generates a new audit-record identifier.
    fn new_audit_id(&self) -> AuditId;
    /// Generates a new supersede-link identifier.
    fn new_supersede_link_id(&self) -> String;
    /// Generates a new dead-letter identifier.
    fn new_dead_letter_id(&self) -> DeadLetterId;
}

/// Fingerprint hashing port.
pub trait FingerprintHasher: Send + Sync {
    /// Hashes canonical bytes with the requested algorithm.
    fn hash_canonical_bytes(
        &self,
        bytes: CanonicalBytes,
        algorithm: FingerprintAlgorithm,
        schema_version: String,
    ) -> Result<CanonicalFingerprint, MethodLibraryError>;
}

/// Feature-flag port.
pub trait FeatureFlagPort: Send + Sync {
    /// Ensures a feature flag is enabled.
    fn ensure_enabled(&self, flag: FeatureFlag) -> Result<(), MethodLibraryError>;
}

/// Observability port.
pub trait ObservabilityPort: Send + Sync {
    /// Records an observability event.
    fn record_event(&self, event: ObservabilityEvent) -> Result<(), MethodLibraryError>;
}
