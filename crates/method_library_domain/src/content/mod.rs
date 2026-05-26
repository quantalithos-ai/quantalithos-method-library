//! Shared content-domain types, lifecycle rules, aggregate, and policies.

use serde_json::Value;

pub mod aggregate;
pub mod fingerprint;
pub mod kind;
pub mod lifecycle;
pub mod reference;
pub mod version;

pub use crate::definitions::MethodContentPayload;
pub use aggregate::MethodContent;
pub use fingerprint::{
    CanonicalBytes, CanonicalFingerprint, CanonicalSchemaVersion, FingerprintAlgorithm,
};
pub use kind::MethodContentKind;
pub use lifecycle::{LifecycleState, MethodContentLifecycle};
pub use reference::{ContentRef, PublishedContentRef, ReferenceState};
pub use version::{ContentVersion, VersionScheme};

/// Actor kind used in definitions, policies, and shared protocol metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    /// A human actor.
    Human,
    /// A member-like AI actor.
    AiMember,
    /// A system actor.
    System,
}

/// Approved gate reference required by publish-like operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ApprovedGateRef {
    /// Gate identifier.
    pub gate_id: String,
    /// Gate decision identifier.
    pub gate_decision_id: String,
    /// Approval timestamp.
    pub approved_at: Timestamp,
}

impl ApprovedGateRef {
    /// Returns whether the gate reference is populated enough for a publish flow.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.gate_id.trim().is_empty() && !self.gate_decision_id.trim().is_empty()
    }
}

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
pub type LeaseDuration = time::Duration;
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
pub type Timestamp = time::OffsetDateTime;
/// Distributed trace identifier.
pub type TraceId = String;
/// Worker identifier used by outbox-claim leases.
pub type WorkerId = String;

/// Returns a canonical JSON byte representation with object keys sorted recursively.
#[must_use]
pub(crate) fn canonicalize_json(value: &Value) -> Vec<u8> {
    let normalized = normalize_json(value);
    serde_json::to_vec(&normalized).expect("normalized JSON should serialize")
}

/// Recursively normalizes JSON maps into deterministic key order.
#[must_use]
pub(crate) fn normalize_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut normalized = serde_json::Map::new();
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();

            for key in keys {
                let item = map.get(key).expect("key should exist");
                normalized.insert(key.clone(), normalize_json(item));
            }

            Value::Object(normalized)
        }
        Value::Array(items) => Value::Array(items.iter().map(normalize_json).collect()),
        other => other.clone(),
    }
}

/// Returns whether a canonicalized payload contains a forbidden boundary key.
#[must_use]
pub(crate) fn contains_forbidden_boundary_key(value: &Value) -> bool {
    const FORBIDDEN_KEYS: &[&str] = &[
        "qualification_profile",
        "qualification_profile_id",
        "qualification_binding",
        "qualification_binding_id",
        "process_instance",
        "process_instance_id",
        "work_item",
        "work_item_id",
        "artifact_instance",
        "artifact_instance_id",
        "policy_enforce_result",
        "capability_access_decision",
        "ui_session_state",
    ];

    match value {
        Value::Object(map) => map.iter().any(|(key, child)| {
            FORBIDDEN_KEYS.contains(&key.as_str()) || contains_forbidden_boundary_key(child)
        }),
        Value::Array(items) => items.iter().any(contains_forbidden_boundary_key),
        _ => false,
    }
}
