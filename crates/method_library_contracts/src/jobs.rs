//! Worker and operations job contracts.

use serde::{Deserialize, Serialize};

use method_library_domain::MethodLibraryErrorCode;
use method_library_domain::content::{
    BatchSize, CanonicalFingerprint, CanonicalSchemaVersion, ContentId, JobRunId,
    MethodContentKind, OutboxEventId, Timestamp, WorkerId,
};

use crate::events::DefinitionEventType;

/// Stable seed asset-set identifier.
pub type SeedAssetSet = String;
/// Stable downstream consumer identifier.
pub type ConsumerRef = String;
/// Stable projection name.
pub type ProjectionName = String;
/// Stable job-scope hash.
pub type JobScopeHash = String;

/// Item-level job failure summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobItemFailure {
    /// Logical item reference.
    pub item_ref: String,
    /// Stable error code.
    pub error_code: MethodLibraryErrorCode,
    /// Human-readable message.
    pub message: String,
}

/// Fingerprint mismatch report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FingerprintMismatch {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Content kind.
    pub kind: MethodContentKind,
    /// Stored fingerprint.
    pub stored_fingerprint: CanonicalFingerprint,
    /// Recalculated fingerprint.
    pub recalculated_fingerprint: CanonicalFingerprint,
}

/// Projection checkpoint summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionCheckpointView {
    /// Projection name.
    pub projection_name: ProjectionName,
    /// Last processed event identifier.
    pub last_processed_event_id: Option<OutboxEventId>,
    /// Last update timestamp.
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: Timestamp,
}

/// Request DTO for seeding initial method assets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedInitialMethodAssetsJobRequest {
    /// Seed asset-set identifier.
    pub asset_set: SeedAssetSet,
    /// Kinds to seed.
    pub kinds: Vec<MethodContentKind>,
    /// Whether seeded assets should be published.
    pub publish: bool,
    /// Whether the job is a dry run.
    pub dry_run: bool,
}

/// Result DTO for seeding initial method assets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedInitialMethodAssetsJobResult {
    /// Job run identifier.
    pub job_run_id: JobRunId,
    /// Number of created assets.
    pub created_count: u32,
    /// Number of published assets.
    pub published_count: u32,
    /// Number of skipped assets.
    pub skipped_count: u32,
    /// Item-level failures.
    pub failures: Vec<JobItemFailure>,
}

/// Request DTO for replaying definition events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayDefinitionEventsJobRequest {
    /// Target consumer identifier.
    pub consumer: ConsumerRef,
    /// Optional starting cursor.
    pub from_cursor: Option<OutboxEventId>,
    /// Event-type filter.
    pub event_types: Vec<DefinitionEventType>,
    /// Batch size.
    pub batch_size: BatchSize,
    /// Whether the job is a dry run.
    pub dry_run: bool,
}

/// Result DTO for replaying definition events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayDefinitionEventsJobResult {
    /// Job run identifier.
    pub job_run_id: JobRunId,
    /// Number of replayed events.
    pub replayed_count: u32,
    /// Next cursor after the replay.
    pub next_cursor: Option<OutboxEventId>,
    /// Item-level failures.
    pub failures: Vec<JobItemFailure>,
}

/// Request DTO for recalculating fingerprints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecalculateFingerprintJobRequest {
    /// Target content identifiers.
    pub content_ids: Vec<ContentId>,
    /// Optional kind filter.
    pub kind: Option<MethodContentKind>,
    /// Canonical schema version to use.
    pub canonical_schema_version: CanonicalSchemaVersion,
    /// Whether the job is a dry run.
    pub dry_run: bool,
}

/// Result DTO for recalculating fingerprints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecalculateFingerprintJobResult {
    /// Job run identifier.
    pub job_run_id: JobRunId,
    /// Number of checked contents.
    pub checked_count: u32,
    /// Mismatch reports.
    pub mismatches: Vec<FingerprintMismatch>,
}

/// Request DTO for rebuilding read models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuildReadModelsJobRequest {
    /// Projection names to rebuild.
    pub projection_names: Vec<ProjectionName>,
    /// Optional starting cursor.
    pub from_cursor: Option<OutboxEventId>,
    /// Batch size.
    pub batch_size: BatchSize,
    /// Whether the job is a dry run.
    pub dry_run: bool,
}

/// Result DTO for rebuilding read models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuildReadModelsJobResult {
    /// Job run identifier.
    pub job_run_id: JobRunId,
    /// Number of processed items.
    pub processed_count: u32,
    /// Updated projection checkpoint.
    pub checkpoint: Option<ProjectionCheckpointView>,
    /// Item-level failures.
    pub failures: Vec<JobItemFailure>,
}

/// Request DTO for relaying pending outbox events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayOutboxEventsJobRequest {
    /// Worker identifier claiming the outbox events.
    pub worker_id: WorkerId,
    /// Batch size.
    pub batch_size: BatchSize,
    /// Lease duration in seconds.
    pub lease_seconds: u64,
}

/// Result DTO for relaying pending outbox events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayOutboxEventsJobResult {
    /// Worker identifier that executed the relay.
    pub worker_id: WorkerId,
    /// Number of published events.
    pub published_count: u32,
    /// Number of retryable failures.
    pub retryable_failure_count: u32,
    /// Number of dead-lettered events.
    pub dead_letter_count: u32,
}

#[cfg(test)]
mod tests {
    use super::{JobItemFailure, ReplayDefinitionEventsJobRequest};
    use method_library_domain::MethodLibraryErrorCode;

    #[test]
    fn serializes_job_requests_and_failures() {
        let request = ReplayDefinitionEventsJobRequest {
            consumer: "identity".to_string(),
            from_cursor: Some("evt-10".to_string()),
            event_types: Vec::new(),
            batch_size: 50,
            dry_run: true,
        };
        let failure = JobItemFailure {
            item_ref: "evt-10".to_string(),
            error_code: MethodLibraryErrorCode::BusPublishFailed,
            message: "bus unavailable".to_string(),
        };

        let request_json = serde_json::to_string(&request).expect("request should serialize");
        let failure_json = serde_json::to_string(&failure).expect("failure should serialize");

        assert!(request_json.contains("\"consumer\":\"identity\""));
        assert!(failure_json.contains("BUS_PUBLISH_FAILED"));
    }
}
