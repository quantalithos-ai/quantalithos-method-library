//! Shared domain and application error primitives.

use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

/// Stable error code surface used across commands, queries, workers, and jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MethodLibraryErrorCode {
    /// Required gateway headers were missing from the inbound request.
    GatewayContextMissing,
    /// Gateway headers were present but failed trust or shape validation.
    GatewayContextInvalid,
    /// A path identifier and its body counterpart disagreed.
    PathBodyMismatch,
    /// A command or job request omitted the required idempotency key.
    IdempotencyKeyRequired,
    /// A content kind did not match the provided payload shape.
    PayloadKindMismatch,
    /// A payload attempted to carry downstream use-truth data.
    BoundaryViolation,
    /// A reference target was malformed or used a forbidden kind.
    ReferenceInvalid,
    /// A reference target existed but was not in a publishable state.
    ReferenceNotPublished,
    /// The requested method-content aggregate did not exist.
    MethodContentNotFound,
    /// A requested published version did not exist.
    ContentVersionNotFound,
    /// A requested snapshot metadata record did not exist.
    SnapshotNotFound,
    /// The requested lifecycle transition violated the state machine.
    LifecycleTransitionNotAllowed,
    /// A caller tried to mutate a published definition in place.
    PublishedContentImmutable,
    /// The caller revision no longer matched the stored write-model revision.
    RevisionConflict,
    /// A version already existed for the target content family.
    ContentVersionConflict,
    /// A supersede command conflicted with an existing replacement chain.
    SupersedeConflict,
    /// The replacement content kind did not match the content being superseded.
    SupersedeKindMismatch,
    /// A supersede command did not specify the replacement target it requires.
    SupersedeTargetRequired,
    /// An idempotency key was reused with a different request hash.
    IdempotencyConflict,
    /// An idempotency record was in a state that blocks the requested operation.
    IdempotencyStatusConflict,
    /// A publish-like operation omitted the approved gate reference.
    PublishGateRequired,
    /// A publish gate reference was rejected by governance validation.
    PublishGateInvalid,
    /// The governance dependency could not validate the approved gate.
    GovernanceUnavailable,
    /// Canonical fingerprint material could not be generated or hashed.
    FingerprintBuildFailed,
    /// A recalculated fingerprint diverged from the stored published value.
    FingerprintMismatch,
    /// Snapshot metadata or payload construction failed before commit.
    SnapshotBuildFailed,
    /// The object-storage dependency could not read or write snapshot payloads.
    ObjectStorageUnavailable,
    /// The event bus rejected a publish attempt outside the write transaction.
    BusPublishFailed,
    /// An outbox event status change violated the relay state machine.
    OutboxStatusConflict,
    /// An outbox event payload was not valid for publication.
    OutboxEventInvalid,
    /// An outbox retry was attempted before the retry lease became due.
    OutboxRetryNotDue,
    /// A required projection was not available for the requested read path.
    ProjectionNotReady,
    /// A projection was available but lagged behind the desired freshness bound.
    StaleProjection,
    /// A projection rebuild or upsert operation failed.
    ProjectionUpdateFailed,
    /// A checkpoint compare-and-swap lost a concurrency race.
    CheckpointConflict,
    /// An operations job request was malformed or violated job guards.
    JobRequestInvalid,
    /// A job-run status transition violated the job state machine.
    JobStatusConflict,
    /// A dry-run job attempted to write truth, outbox, or checkpoints.
    JobDryRunWriteForbidden,
    /// The persistence dependency was unavailable for the requested operation.
    PersistenceUnavailable,
    /// The local transaction failed to commit cleanly.
    TransactionCommitFailed,
    /// A reserved P1 endpoint or operation was called while the feature is disabled.
    P1FeatureDisabled,
}

impl MethodLibraryErrorCode {
    /// Returns the stable string representation used in JSON responses and logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GatewayContextMissing => "GATEWAY_CONTEXT_MISSING",
            Self::GatewayContextInvalid => "GATEWAY_CONTEXT_INVALID",
            Self::PathBodyMismatch => "PATH_BODY_MISMATCH",
            Self::IdempotencyKeyRequired => "IDEMPOTENCY_KEY_REQUIRED",
            Self::PayloadKindMismatch => "PAYLOAD_KIND_MISMATCH",
            Self::BoundaryViolation => "BOUNDARY_VIOLATION",
            Self::ReferenceInvalid => "REFERENCE_INVALID",
            Self::ReferenceNotPublished => "REFERENCE_NOT_PUBLISHED",
            Self::MethodContentNotFound => "METHOD_CONTENT_NOT_FOUND",
            Self::ContentVersionNotFound => "CONTENT_VERSION_NOT_FOUND",
            Self::SnapshotNotFound => "SNAPSHOT_NOT_FOUND",
            Self::LifecycleTransitionNotAllowed => "LIFECYCLE_TRANSITION_NOT_ALLOWED",
            Self::PublishedContentImmutable => "PUBLISHED_CONTENT_IMMUTABLE",
            Self::RevisionConflict => "REVISION_CONFLICT",
            Self::ContentVersionConflict => "CONTENT_VERSION_CONFLICT",
            Self::SupersedeConflict => "SUPERSEDE_CONFLICT",
            Self::SupersedeKindMismatch => "SUPERSEDE_KIND_MISMATCH",
            Self::SupersedeTargetRequired => "SUPERSEDE_TARGET_REQUIRED",
            Self::IdempotencyConflict => "IDEMPOTENCY_CONFLICT",
            Self::IdempotencyStatusConflict => "IDEMPOTENCY_STATUS_CONFLICT",
            Self::PublishGateRequired => "PUBLISH_GATE_REQUIRED",
            Self::PublishGateInvalid => "PUBLISH_GATE_INVALID",
            Self::GovernanceUnavailable => "GOVERNANCE_UNAVAILABLE",
            Self::FingerprintBuildFailed => "FINGERPRINT_BUILD_FAILED",
            Self::FingerprintMismatch => "FINGERPRINT_MISMATCH",
            Self::SnapshotBuildFailed => "SNAPSHOT_BUILD_FAILED",
            Self::ObjectStorageUnavailable => "OBJECT_STORAGE_UNAVAILABLE",
            Self::BusPublishFailed => "BUS_PUBLISH_FAILED",
            Self::OutboxStatusConflict => "OUTBOX_STATUS_CONFLICT",
            Self::OutboxEventInvalid => "OUTBOX_EVENT_INVALID",
            Self::OutboxRetryNotDue => "OUTBOX_RETRY_NOT_DUE",
            Self::ProjectionNotReady => "PROJECTION_NOT_READY",
            Self::StaleProjection => "STALE_PROJECTION",
            Self::ProjectionUpdateFailed => "PROJECTION_UPDATE_FAILED",
            Self::CheckpointConflict => "CHECKPOINT_CONFLICT",
            Self::JobRequestInvalid => "JOB_REQUEST_INVALID",
            Self::JobStatusConflict => "JOB_STATUS_CONFLICT",
            Self::JobDryRunWriteForbidden => "JOB_DRY_RUN_WRITE_FORBIDDEN",
            Self::PersistenceUnavailable => "PERSISTENCE_UNAVAILABLE",
            Self::TransactionCommitFailed => "TRANSACTION_COMMIT_FAILED",
            Self::P1FeatureDisabled => "P1_FEATURE_DISABLED",
        }
    }
}

impl Display for MethodLibraryErrorCode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Shared error value carrying structured metadata for API and worker mappings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodLibraryError {
    /// Stable machine-readable error code.
    pub code: MethodLibraryErrorCode,
    /// Human-readable explanation intended for developers and operators.
    pub message: String,
    /// Whether the caller may retry the operation without changing input.
    pub retryable: bool,
    /// Structured details safe to include in responses and logs.
    pub details: BTreeMap<String, String>,
}

impl MethodLibraryError {
    /// Creates a new structured error with no additional details.
    #[must_use]
    pub fn new(code: MethodLibraryErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            details: BTreeMap::new(),
        }
    }

    /// Appends a structured detail entry to the error payload.
    #[must_use]
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    /// Creates a non-retryable validation-style error.
    #[must_use]
    pub fn validation(code: MethodLibraryErrorCode, message: impl Into<String>) -> Self {
        Self::new(code, message, false)
    }

    /// Creates a retryable dependency or infrastructure error.
    #[must_use]
    pub fn retryable(code: MethodLibraryErrorCode, message: impl Into<String>) -> Self {
        Self::new(code, message, true)
    }
}

impl Display for MethodLibraryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for MethodLibraryError {}

#[cfg(test)]
mod tests {
    use super::{MethodLibraryError, MethodLibraryErrorCode};

    #[test]
    fn exposes_stable_error_code_strings() {
        assert_eq!(
            MethodLibraryErrorCode::RevisionConflict.as_str(),
            "REVISION_CONFLICT"
        );
        assert_eq!(
            MethodLibraryErrorCode::P1FeatureDisabled.as_str(),
            "P1_FEATURE_DISABLED"
        );
    }

    #[test]
    fn stores_structured_error_details() {
        let error = MethodLibraryError::validation(
            MethodLibraryErrorCode::PayloadKindMismatch,
            "payload does not match the content kind",
        )
        .with_detail("kind", "qualification");

        assert!(!error.retryable);
        assert_eq!(
            error.details.get("kind"),
            Some(&"qualification".to_string())
        );
    }
}
