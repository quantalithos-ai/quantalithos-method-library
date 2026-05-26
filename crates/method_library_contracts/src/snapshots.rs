//! Snapshot metadata and export contracts.

use serde::{Deserialize, Serialize};

use method_library_domain::content::{
    CanonicalFingerprint, ContentId, ContentVersion, PublishedContentRef, SnapshotId, Timestamp,
};

use crate::queries::MethodContentView;

/// Stable snapshot schema version label.
pub type SnapshotSchemaVersion = String;
/// Stable object-storage blob reference.
pub type SnapshotBlobRef = String;

/// Snapshot reference shared across responses and events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRef {
    /// Snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Snapshot schema version.
    pub schema_version: SnapshotSchemaVersion,
    /// Blob reference in object storage.
    pub blob_ref: SnapshotBlobRef,
}

/// Exported snapshot payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotPayload {
    /// Frozen content view.
    pub content: MethodContentView,
    /// Frozen published references.
    pub references: Vec<PublishedContentRef>,
    /// Payload generation timestamp.
    pub generated_at: Timestamp,
    /// Payload schema version.
    pub schema_version: SnapshotSchemaVersion,
}

/// Snapshot metadata stored with published content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionSnapshot {
    /// Snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// Content identifier.
    pub content_id: ContentId,
    /// Published content version.
    pub version: ContentVersion,
    /// Published fingerprint.
    pub fingerprint: CanonicalFingerprint,
    /// Snapshot schema version.
    pub schema_version: SnapshotSchemaVersion,
    /// Blob reference in object storage.
    pub blob_ref: SnapshotBlobRef,
    /// Creation timestamp.
    pub created_at: Timestamp,
    /// Frozen content reference.
    pub content_ref: PublishedContentRef,
    /// Frozen published references.
    pub references: Vec<PublishedContentRef>,
}

/// Response DTO for exporting a definition snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportDefinitionSnapshotResponse {
    /// Logical snapshot reference.
    pub snapshot_ref: SnapshotRef,
    /// Frozen published content reference.
    pub content_ref: PublishedContentRef,
    /// Snapshot payload.
    pub payload: SnapshotPayload,
    /// Frozen published references.
    pub references: Vec<PublishedContentRef>,
}

impl DefinitionSnapshot {
    /// Returns the logical snapshot reference derived from the metadata record.
    #[must_use]
    pub fn snapshot_ref(&self) -> SnapshotRef {
        SnapshotRef {
            snapshot_id: self.snapshot_id.clone(),
            schema_version: self.schema_version.clone(),
            blob_ref: self.blob_ref.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::DefinitionSnapshot;
    use method_library_domain::content::{
        CanonicalFingerprint, ContentVersion, FingerprintAlgorithm, MethodContentKind,
    };

    #[test]
    fn derives_snapshot_refs_from_metadata() {
        let snapshot = DefinitionSnapshot {
            snapshot_id: "snap-1".to_string(),
            content_id: "content-1".to_string(),
            version: ContentVersion::new("1.0.0").expect("version should be valid"),
            fingerprint: CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "abc123", "1.0")
                .expect("fingerprint should be valid"),
            schema_version: "1.0".to_string(),
            blob_ref: "object://snap-1".to_string(),
            created_at: datetime!(2026-05-26 08:00:00 UTC),
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
            references: Vec::new(),
        };

        assert_eq!(snapshot.snapshot_ref().snapshot_id, "snap-1");
    }
}
