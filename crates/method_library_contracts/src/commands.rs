//! Command DTOs for the P0 write path.

use serde::{Deserialize, Serialize};

use method_library_domain::content::{
    ApprovedGateRef, CanonicalFingerprint, ContentFamilyId, ContentId, ContentRef, ContentVersion,
    LifecycleState, MethodContentKind, OutboxEventId, Revision, Timestamp,
};
use method_library_domain::definitions::MethodContentPayload;

use crate::actor::ArtifactRef;
use crate::snapshots::SnapshotRef;

/// Downstream handling hint used by retire commands and events.
pub type RetirePolicy = String;
/// Stable supersede-link identifier.
pub type SupersedeLinkId = String;

/// Command DTO for creating a draft content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateMethodContentDraftCommand {
    /// Content kind to create.
    pub kind: MethodContentKind,
    /// Display name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Definition payload.
    pub payload: MethodContentPayload,
    /// Draft references.
    pub references: Vec<ContentRef>,
    /// Source evidence references.
    pub source_refs: Vec<ArtifactRef>,
}

/// Response DTO for creating a draft content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateMethodContentDraftResponse {
    /// New content identifier.
    pub content_id: ContentId,
    /// New content family identifier.
    pub content_family_id: ContentFamilyId,
    /// Content kind.
    pub kind: MethodContentKind,
    /// Lifecycle state after creation.
    pub lifecycle_state: LifecycleState,
    /// Initial revision.
    pub revision: Revision,
}

/// Command DTO for updating a draft content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateMethodContentDraftCommand {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Caller-observed revision.
    pub expected_revision: Revision,
    /// New display name.
    pub name: String,
    /// New optional description.
    pub description: Option<String>,
    /// New payload.
    pub payload: MethodContentPayload,
    /// Replacement reference collection.
    pub references: Vec<ContentRef>,
}

/// Response DTO for updating a draft content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateMethodContentDraftResponse {
    /// Updated content identifier.
    pub content_id: ContentId,
    /// Content kind.
    pub kind: MethodContentKind,
    /// Lifecycle state after the update.
    pub lifecycle_state: LifecycleState,
    /// Saved revision.
    pub revision: Revision,
}

/// Command DTO for submitting a draft for review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitMethodContentForReviewCommand {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Caller-observed revision.
    pub expected_revision: Revision,
    /// Optional review reason.
    pub review_reason: Option<String>,
    /// Review evidence references.
    pub review_evidence_refs: Vec<ArtifactRef>,
}

/// Response DTO for submitting a draft for review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitMethodContentForReviewResponse {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Content kind.
    pub kind: MethodContentKind,
    /// Lifecycle state after submission.
    pub lifecycle_state: LifecycleState,
    /// Saved revision.
    pub revision: Revision,
}

/// Command DTO for publishing a content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishMethodContentCommand {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Caller-observed revision.
    pub expected_revision: Revision,
    /// Published version.
    pub version: ContentVersion,
    /// Approved gate reference.
    pub approved_gate_ref: ApprovedGateRef,
    /// Human-readable publish reason.
    pub publish_reason: String,
}

/// Response DTO for publishing a content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishMethodContentResponse {
    /// Published content identifier.
    pub content_id: ContentId,
    /// Content kind.
    pub kind: MethodContentKind,
    /// Lifecycle state after publish.
    pub lifecycle_state: LifecycleState,
    /// Published version.
    pub version: ContentVersion,
    /// Published fingerprint.
    pub fingerprint: CanonicalFingerprint,
    /// Snapshot reference.
    pub snapshot_ref: SnapshotRef,
    /// Outbox event identifier.
    pub outbox_event_id: OutboxEventId,
    /// Saved revision.
    pub revision: Revision,
}

/// Command DTO for deprecating a published content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeprecateMethodContentCommand {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Caller-observed revision.
    pub expected_revision: Revision,
    /// Deprecation reason.
    pub reason: String,
    /// Optional effective timestamp.
    pub effective_at: Option<Timestamp>,
}

/// Response DTO for deprecating a published content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeprecateMethodContentResponse {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Content kind.
    pub kind: MethodContentKind,
    /// Lifecycle state after deprecation.
    pub lifecycle_state: LifecycleState,
    /// Current published version.
    pub version: ContentVersion,
    /// Current published fingerprint.
    pub fingerprint: CanonicalFingerprint,
    /// Outbox event identifier.
    pub outbox_event_id: OutboxEventId,
    /// Saved revision.
    pub revision: Revision,
}

/// Command DTO for retiring a content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetireMethodContentCommand {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Caller-observed revision.
    pub expected_revision: Revision,
    /// Retirement reason.
    pub reason: String,
    /// Downstream handling hint.
    pub retire_policy: RetirePolicy,
}

/// Response DTO for retiring a content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetireMethodContentResponse {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Content kind.
    pub kind: MethodContentKind,
    /// Lifecycle state after retirement.
    pub lifecycle_state: LifecycleState,
    /// Current published version.
    pub version: ContentVersion,
    /// Current published fingerprint.
    pub fingerprint: CanonicalFingerprint,
    /// Outbox event identifier.
    pub outbox_event_id: OutboxEventId,
    /// Saved revision.
    pub revision: Revision,
}

/// Command DTO for superseding one content aggregate with another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupersedeMethodContentCommand {
    /// Old content identifier.
    pub old_content_id: ContentId,
    /// Caller-observed revision of the old content.
    pub old_expected_revision: Revision,
    /// New content identifier.
    pub new_content_id: ContentId,
    /// Caller-observed revision of the new content.
    pub new_expected_revision: Revision,
    /// New published version.
    pub new_version: ContentVersion,
    /// Approved gate reference for the new publish.
    pub approved_gate_ref: ApprovedGateRef,
    /// Human-readable supersede reason.
    pub reason: String,
}

/// Response DTO for superseding one content aggregate with another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupersedeMethodContentResponse {
    /// Old content identifier.
    pub old_content_id: ContentId,
    /// Old lifecycle state after supersede.
    pub old_lifecycle_state: LifecycleState,
    /// New content identifier.
    pub new_content_id: ContentId,
    /// New lifecycle state after publish.
    pub new_lifecycle_state: LifecycleState,
    /// New published version.
    pub new_version: ContentVersion,
    /// New published fingerprint.
    pub new_fingerprint: CanonicalFingerprint,
    /// Supersede-link identifier.
    pub supersede_link_id: SupersedeLinkId,
    /// New snapshot reference.
    pub snapshot_ref: SnapshotRef,
    /// Related outbox event identifiers.
    pub outbox_event_ids: Vec<OutboxEventId>,
}

#[cfg(test)]
mod tests {
    use super::{CreateMethodContentDraftCommand, PublishMethodContentResponse};
    use method_library_domain::content::{
        CanonicalFingerprint, ContentVersion, FingerprintAlgorithm, LifecycleState,
        MethodContentKind,
    };
    use method_library_domain::definitions::{
        EvidenceKind, EvidenceRule, MethodContentPayload, Qualification, QualificationLevel,
        QualificationLevelModel,
    };

    #[test]
    fn serializes_create_and_publish_contracts() {
        let create = CreateMethodContentDraftCommand {
            kind: MethodContentKind::Qualification,
            name: "Quality".to_string(),
            description: None,
            payload: MethodContentPayload::Qualification(Qualification {
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
            references: Vec::new(),
            source_refs: Vec::new(),
        };
        let publish = PublishMethodContentResponse {
            content_id: "content-1".to_string(),
            kind: MethodContentKind::Qualification,
            lifecycle_state: LifecycleState::Published,
            version: ContentVersion::new("1.0.0").expect("version should be valid"),
            fingerprint: CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "abc123", "1.0")
                .expect("fingerprint should be valid"),
            snapshot_ref: crate::snapshots::SnapshotRef {
                snapshot_id: "snap-1".to_string(),
                schema_version: "1.0".to_string(),
                blob_ref: "object://snap-1".to_string(),
            },
            outbox_event_id: "evt-1".to_string(),
            revision: 3,
        };

        let create_json = serde_json::to_string(&create).expect("create command should serialize");
        let publish_json =
            serde_json::to_string(&publish).expect("publish response should serialize");

        assert!(create_json.contains("\"kind\":\"qualification\""));
        assert!(publish_json.contains("\"outbox_event_id\":\"evt-1\""));
    }
}
