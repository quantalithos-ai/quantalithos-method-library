//! Query DTOs and read-model contracts.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use method_library_domain::content::{
    CanonicalFingerprint, ContentFamilyId, ContentId, ContentVersion, LifecycleState,
    MethodContentKind, OutboxEventId, PublishedContentRef, Revision, SnapshotId, Timestamp,
};
use method_library_domain::definitions::{
    MethodContentPayload, ViewActionRule, ViewFieldRule, ViewObjectKind, ViewScopeRule,
};

use crate::actor::ActorRef;

/// Stable API schema version label.
pub type ApiSchemaVersion = String;
/// Stable page cursor.
pub type PageCursor = String;
/// Stable page-size value.
pub type PageLimit = u32;
/// Structured view-resolution scope.
pub type ViewResolveScope = JsonValue;

/// Read mode requested by a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadMode {
    /// Read published or published-like data.
    Published,
    /// Read authoring data including drafts.
    Authoring,
}

/// Source from which read data was served.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadSource {
    /// Data was read from the write model.
    WriteModel,
    /// Data was read from a projection.
    Projection,
    /// Data was read from object storage.
    ObjectStorage,
}

/// Pagination metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageInfo {
    /// Cursor for the next page if more data exists.
    pub next_cursor: Option<PageCursor>,
    /// Whether another page exists.
    pub has_more: bool,
}

/// Read-consistency marker returned by queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadConsistency {
    /// Physical source used by the query.
    pub source: ReadSource,
    /// Whether the data may be stale.
    pub stale: bool,
    /// Last processed outbox event if known.
    pub checkpoint: Option<OutboxEventId>,
}

/// View of a content reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentRefView {
    /// Target content identifier.
    pub target_content_id: ContentId,
    /// Target content kind.
    pub target_kind: MethodContentKind,
    /// Optional target version.
    pub target_version: Option<ContentVersion>,
    /// Optional target fingerprint.
    pub target_fingerprint: Option<CanonicalFingerprint>,
}

/// Read model of a method-content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodContentView {
    /// Content identifier.
    pub content_id: ContentId,
    /// Content family identifier.
    pub content_family_id: ContentFamilyId,
    /// Content kind.
    pub kind: MethodContentKind,
    /// Display name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Lifecycle state.
    pub lifecycle_state: LifecycleState,
    /// Optional published version.
    pub version: Option<ContentVersion>,
    /// Optional published fingerprint.
    pub fingerprint: Option<CanonicalFingerprint>,
    /// Optional payload.
    pub payload: Option<MethodContentPayload>,
    /// Reference view collection.
    pub references: Vec<ContentRefView>,
    /// Current optimistic-lock revision.
    pub revision: Revision,
}

/// Summary read model used by list queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentSummaryView {
    /// Content identifier.
    pub content_id: ContentId,
    /// Content kind.
    pub kind: MethodContentKind,
    /// Display name.
    pub name: String,
    /// Lifecycle state.
    pub lifecycle_state: LifecycleState,
    /// Optional published version.
    pub version: Option<ContentVersion>,
    /// Last update time.
    pub updated_at: Timestamp,
}

/// Published version read model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentVersionView {
    /// Content identifier.
    pub content_id: ContentId,
    /// Content family identifier.
    pub content_family_id: ContentFamilyId,
    /// Published version.
    pub version: ContentVersion,
    /// Published fingerprint.
    pub fingerprint: CanonicalFingerprint,
    /// Optional snapshot reference.
    pub snapshot_ref: Option<crate::snapshots::SnapshotRef>,
    /// Publish timestamp.
    pub published_at: Timestamp,
}

/// Lifecycle history entry read model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleHistoryEntryView {
    /// Previous lifecycle state.
    pub from_state: Option<LifecycleState>,
    /// New lifecycle state.
    pub to_state: LifecycleState,
    /// Actor that caused the transition.
    pub actor_id: String,
    /// Optional reason.
    pub reason: Option<String>,
    /// Creation timestamp.
    pub created_at: Timestamp,
}

/// Audit-record summary view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecordView {
    /// Request identifier.
    pub request_id: String,
    /// Trace identifier.
    pub trace_id: String,
    /// Actor reference.
    pub actor_ref: ActorRef,
    /// Audit action name.
    pub action: String,
    /// Result label.
    pub result: String,
    /// Occurrence timestamp.
    pub occurred_at: Timestamp,
}

/// Outbox-event summary view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxEventView {
    /// Event identifier.
    pub event_id: OutboxEventId,
    /// Event type.
    pub event_type: crate::events::DefinitionEventType,
    /// Event occurrence timestamp.
    pub occurred_at: Timestamp,
    /// Optional snapshot reference.
    pub snapshot_ref: Option<crate::snapshots::SnapshotRef>,
}

/// Supersede-link summary view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupersedeLinkView {
    /// Old content identifier.
    pub old_content_id: ContentId,
    /// New content identifier.
    pub new_content_id: ContentId,
    /// Supersede reason.
    pub reason: String,
    /// Creation timestamp.
    pub created_at: Timestamp,
}

/// Resolved view-profile DTO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedViewProfile {
    /// Published view-profile reference.
    pub view_profile_ref: PublishedContentRef,
    /// Resolved scope rules.
    pub scope_rules: Vec<ViewScopeRule>,
    /// Resolved field rules.
    pub field_rules: Vec<ViewFieldRule>,
    /// Resolved action rules.
    pub action_rules: Vec<ViewActionRule>,
}

/// Trace read model for a method-content definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionTraceView {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Published version history.
    pub versions: Vec<ContentVersionView>,
    /// Lifecycle history.
    pub lifecycle_history: Vec<LifecycleHistoryEntryView>,
    /// Audit summaries.
    pub audit_records: Vec<AuditRecordView>,
    /// Outbox-event summaries.
    pub outbox_events: Vec<OutboxEventView>,
    /// Supersede chain summaries.
    pub supersede_chain: Vec<SupersedeLinkView>,
}

/// Query DTO for retrieving one content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetMethodContentQuery {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Requested read mode.
    pub read_mode: ReadMode,
    /// Whether to include the payload.
    pub include_payload: bool,
    /// Whether to include references.
    pub include_references: bool,
}

/// Response DTO for retrieving one content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetMethodContentResponse {
    /// Response schema version.
    pub schema_version: ApiSchemaVersion,
    /// Content read model.
    pub content: MethodContentView,
    /// Read consistency marker.
    pub consistency: ReadConsistency,
}

/// Query DTO for listing content aggregates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListMethodContentsQuery {
    /// Optional kind filter.
    pub kind: Option<MethodContentKind>,
    /// Optional lifecycle filter.
    pub lifecycle_state: Option<LifecycleState>,
    /// Requested read mode.
    pub read_mode: ReadMode,
    /// Optional page cursor.
    pub cursor: Option<PageCursor>,
    /// Page size.
    pub limit: PageLimit,
    /// Sort key.
    pub sort: Option<String>,
}

/// Response DTO for listing content aggregates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListMethodContentsResponse {
    /// Summary items.
    pub items: Vec<ContentSummaryView>,
    /// Pagination metadata.
    pub page: PageInfo,
    /// Read consistency marker.
    pub consistency: ReadConsistency,
}

/// Query DTO for retrieving a published content version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetMethodContentVersionQuery {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Target published version.
    pub version: ContentVersion,
    /// Whether to include the snapshot reference.
    pub include_snapshot_ref: bool,
}

/// Response DTO for retrieving a published content version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetMethodContentVersionResponse {
    /// Published version view.
    pub content_version: ContentVersionView,
}

/// Query DTO for exporting a definition snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportDefinitionSnapshotQuery {
    /// Optional snapshot identifier.
    pub snapshot_id: Option<SnapshotId>,
    /// Optional content identifier.
    pub content_id: Option<ContentId>,
    /// Optional content version.
    pub version: Option<ContentVersion>,
    /// Whether to verify fingerprint integrity.
    pub verify_fingerprint: bool,
}

/// Response DTO for resolving a view profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveViewProfileResponse {
    /// Resolved view profile if one matched.
    pub view_profile: Option<ResolvedViewProfile>,
    /// Read consistency marker.
    pub consistency: ReadConsistency,
}

/// Query DTO for resolving a view profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveViewProfileQuery {
    /// Role reference used to select the profile.
    pub role_ref: PublishedContentRef,
    /// Target object kind.
    pub object_kind: ViewObjectKind,
    /// Resolution scope input.
    pub scope: ViewResolveScope,
}

/// Query DTO for retrieving a definition trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetDefinitionTraceQuery {
    /// Target content identifier.
    pub content_id: ContentId,
    /// Whether to include audit summaries.
    pub include_audit: bool,
    /// Whether to include event summaries.
    pub include_events: bool,
    /// Optional page cursor.
    pub cursor: Option<PageCursor>,
}

/// Response DTO for retrieving a definition trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetDefinitionTraceResponse {
    /// Trace read model.
    pub trace: DefinitionTraceView,
    /// Optional pagination metadata.
    pub page: Option<PageInfo>,
}

#[cfg(test)]
mod tests {
    use super::{GetMethodContentResponse, MethodContentView, ReadConsistency, ReadSource};
    use method_library_domain::content::{LifecycleState, MethodContentKind};

    #[test]
    fn serializes_content_view_responses() {
        let response = GetMethodContentResponse {
            schema_version: "1.0".to_string(),
            content: MethodContentView {
                content_id: "content-1".to_string(),
                content_family_id: "family-1".to_string(),
                kind: MethodContentKind::Qualification,
                name: "Quality".to_string(),
                description: None,
                lifecycle_state: LifecycleState::Draft,
                version: None,
                fingerprint: None,
                payload: None,
                references: Vec::new(),
                revision: 1,
            },
            consistency: ReadConsistency {
                source: ReadSource::Projection,
                stale: false,
                checkpoint: Some("evt-1".to_string()),
            },
        };

        let payload = serde_json::to_string(&response).expect("response should serialize");

        assert!(payload.contains("\"schema_version\":\"1.0\""));
        assert!(payload.contains("\"checkpoint\":\"evt-1\""));
    }
}
