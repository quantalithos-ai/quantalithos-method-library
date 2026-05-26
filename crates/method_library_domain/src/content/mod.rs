//! Shared content-domain types, lifecycle rules, and value objects.

pub mod fingerprint;
pub mod kind;
pub mod lifecycle;
pub mod reference;
pub mod version;

use time::{Duration, OffsetDateTime};

pub use fingerprint::{CanonicalFingerprint, CanonicalSchemaVersion, FingerprintAlgorithm};
pub use kind::MethodContentKind;
pub use lifecycle::{LifecycleState, MethodContentLifecycle};
pub use reference::{ContentRef, PublishedContentRef, ReferenceState};
pub use version::{ContentVersion, VersionScheme};

/// Identifier of the actor recorded in lifecycle, audit, or job metadata.
pub type ActorId = String;
/// Size used by worker and replay batch operations.
pub type BatchSize = u32;
/// Stable family identifier shared by versions in the same content lineage.
pub type ContentFamilyId = String;
/// Stable identifier of a single method-content definition aggregate.
pub type ContentId = String;
/// Idempotency key provided by callers or jobs.
pub type IdempotencyKey = String;
/// Operations job name.
pub type JobName = String;
/// Stable identifier of a job run.
pub type JobRunId = String;
/// Lease duration used by outbox workers.
pub type LeaseDuration = Duration;
/// Stable identifier of an outbox event.
pub type OutboxEventId = String;
/// Canonical request hash used for idempotency conflict detection.
pub type RequestHash = String;
/// Gateway request identifier.
pub type RequestId = String;
/// Optimistic-lock revision stored in the write model.
pub type Revision = i64;
/// Stable identifier of a snapshot metadata record.
pub type SnapshotId = String;
/// Timestamp stored in domain aggregates and lifecycle events.
pub type Timestamp = OffsetDateTime;
/// Distributed trace identifier.
pub type TraceId = String;
/// Worker identifier used by outbox-claim leases.
pub type WorkerId = String;
