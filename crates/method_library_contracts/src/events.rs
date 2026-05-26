//! Outbound event contracts shared by outbox, relay, and replay flows.

use serde::{Deserialize, Serialize};

use method_library_domain::content::{
    ApprovedGateRef, CanonicalFingerprint, ContentVersion, PublishedContentRef, Timestamp,
};

use crate::snapshots::SnapshotRef;

/// Stable event identifier.
pub type DefinitionEventId = String;
/// Stable event schema version label.
pub type EventSchemaVersion = String;
/// Stable producer identifier.
pub type ProducerRef = String;

/// Outbound definition event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DefinitionEventType {
    /// A definition was published.
    #[serde(rename = "method_library.content.published")]
    ContentPublished,
    /// A definition was deprecated.
    #[serde(rename = "method_library.content.deprecated")]
    ContentDeprecated,
    /// A definition was retired.
    #[serde(rename = "method_library.content.retired")]
    ContentRetired,
    /// A definition fingerprint changed.
    #[serde(rename = "method_library.content.fingerprint_changed")]
    FingerprintChanged,
}

/// Request and trace identifiers attached to an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventTraceContext {
    /// Request identifier that created the event.
    pub request_id: String,
    /// Trace identifier that created the event.
    pub trace_id: String,
}

/// Published-content event payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentPublishedPayload {
    /// Approved gate reference.
    pub gate_ref: ApprovedGateRef,
    /// Published version.
    pub version: ContentVersion,
    /// Published fingerprint.
    pub fingerprint: CanonicalFingerprint,
}

/// Deprecated-content event payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentDeprecatedPayload {
    /// Deprecation reason.
    pub reason: String,
    /// Effective timestamp.
    pub effective_at: Timestamp,
}

/// Retired-content event payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentRetiredPayload {
    /// Retirement reason.
    pub reason: String,
    /// Downstream handling hint.
    pub retire_policy: String,
}

/// Fingerprint-changed event payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FingerprintChangedPayload {
    /// Previous fingerprint.
    pub old_fingerprint: CanonicalFingerprint,
    /// New fingerprint.
    pub new_fingerprint: CanonicalFingerprint,
    /// Reason for the change.
    pub change_reason: String,
}

/// Variant payload for outbound definition events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionEventPayload {
    /// Published-content payload.
    ContentPublished(ContentPublishedPayload),
    /// Deprecated-content payload.
    ContentDeprecated(ContentDeprecatedPayload),
    /// Retired-content payload.
    ContentRetired(ContentRetiredPayload),
    /// Fingerprint-changed payload.
    FingerprintChanged(FingerprintChangedPayload),
}

/// Event envelope persisted in the outbox and published to the bus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionEventEnvelope {
    /// Event identifier.
    pub event_id: DefinitionEventId,
    /// Event type.
    pub event_type: DefinitionEventType,
    /// Event schema version.
    pub schema_version: EventSchemaVersion,
    /// Event occurrence timestamp.
    pub occurred_at: Timestamp,
    /// Producer identifier.
    pub producer: ProducerRef,
    /// Frozen published content reference.
    pub content_ref: PublishedContentRef,
    /// Optional frozen snapshot reference.
    pub snapshot_ref: Option<SnapshotRef>,
    /// Request and trace identifiers.
    pub trace: EventTraceContext,
    /// Typed event payload.
    pub payload: DefinitionEventPayload,
}

impl DefinitionEventType {
    /// Returns the stable dotted-string event type.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ContentPublished => "method_library.content.published",
            Self::ContentDeprecated => "method_library.content.deprecated",
            Self::ContentRetired => "method_library.content.retired",
            Self::FingerprintChanged => "method_library.content.fingerprint_changed",
        }
    }
}

impl DefinitionEventPayload {
    /// Returns the matching event type for the payload variant.
    #[must_use]
    pub const fn event_type(&self) -> DefinitionEventType {
        match self {
            Self::ContentPublished(_) => DefinitionEventType::ContentPublished,
            Self::ContentDeprecated(_) => DefinitionEventType::ContentDeprecated,
            Self::ContentRetired(_) => DefinitionEventType::ContentRetired,
            Self::FingerprintChanged(_) => DefinitionEventType::FingerprintChanged,
        }
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::{
        ContentPublishedPayload, DefinitionEventEnvelope, DefinitionEventPayload,
        DefinitionEventType, EventTraceContext,
    };
    use method_library_domain::content::{
        ApprovedGateRef, CanonicalFingerprint, ContentVersion, FingerprintAlgorithm,
        MethodContentKind, PublishedContentRef,
    };

    #[test]
    fn keeps_event_type_and_payload_in_sync() {
        let payload = DefinitionEventPayload::ContentPublished(ContentPublishedPayload {
            gate_ref: ApprovedGateRef {
                gate_id: "gate-1".to_string(),
                gate_decision_id: "decision-1".to_string(),
                approved_at: datetime!(2026-05-26 08:00:00 UTC),
            },
            version: ContentVersion::new("1.0.0").expect("version should be valid"),
            fingerprint: CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "abc123", "1.0")
                .expect("fingerprint should be valid"),
        });

        let envelope = DefinitionEventEnvelope {
            event_id: "evt-1".to_string(),
            event_type: payload.event_type(),
            schema_version: "1.0".to_string(),
            occurred_at: datetime!(2026-05-26 09:00:00 UTC),
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
                request_id: "req-1".to_string(),
                trace_id: "trace-1".to_string(),
            },
            payload,
        };

        assert_eq!(envelope.event_type, DefinitionEventType::ContentPublished);
        assert_eq!(
            envelope.event_type.as_str(),
            "method_library.content.published"
        );
    }
}
