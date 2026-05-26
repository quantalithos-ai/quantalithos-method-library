//! Protocol contracts shared by the API, worker, and application layers.

use serde::{Deserialize, Serialize};

pub mod actor;
pub mod commands;
pub mod errors;
pub mod events;
pub mod jobs;
pub mod metadata;
pub mod queries;
pub mod snapshots;

pub use actor::{ActorContext, ActorRef, ArtifactRef, GatewayHeaders};
pub use commands::{
    CreateMethodContentDraftCommand, CreateMethodContentDraftResponse,
    DeprecateMethodContentCommand, DeprecateMethodContentResponse, PublishMethodContentCommand,
    PublishMethodContentResponse, RetireMethodContentCommand, RetireMethodContentResponse,
    RetirePolicy, SubmitMethodContentForReviewCommand, SubmitMethodContentForReviewResponse,
    SupersedeLinkId, SupersedeMethodContentCommand, SupersedeMethodContentResponse,
    UpdateMethodContentDraftCommand, UpdateMethodContentDraftResponse,
};
pub use errors::{ErrorBody, ErrorDetails, ErrorResponse};
pub use events::{
    ContentDeprecatedPayload, ContentPublishedPayload, ContentRetiredPayload,
    DefinitionEventEnvelope, DefinitionEventId, DefinitionEventPayload, DefinitionEventType,
    EventSchemaVersion, EventTraceContext, FingerprintChangedPayload, ProducerRef,
};
pub use jobs::{
    ConsumerRef, FingerprintMismatch, JobItemFailure, JobScopeHash, ProjectionCheckpointView,
    ProjectionName, RebuildReadModelsJobRequest, RebuildReadModelsJobResult,
    RecalculateFingerprintJobRequest, RecalculateFingerprintJobResult, RelayOutboxEventsJobRequest,
    RelayOutboxEventsJobResult, ReplayDefinitionEventsJobRequest, ReplayDefinitionEventsJobResult,
    SeedAssetSet, SeedInitialMethodAssetsJobRequest, SeedInitialMethodAssetsJobResult,
};
pub use metadata::{CommandMetadata, RequestMeta};
pub use queries::{
    ApiSchemaVersion, AuditRecordView, ContentRefView, ContentSummaryView, ContentVersionView,
    DefinitionTraceView, GetDefinitionTraceQuery, GetDefinitionTraceResponse,
    GetMethodContentQuery, GetMethodContentResponse, GetMethodContentVersionQuery,
    GetMethodContentVersionResponse, LifecycleHistoryEntryView, ListMethodContentsQuery,
    ListMethodContentsResponse, MethodContentView, OutboxEventView, PageCursor, PageInfo,
    PageLimit, ReadConsistency, ReadMode, ReadSource, ResolveViewProfileQuery,
    ResolveViewProfileResponse, ResolvedViewProfile, SupersedeLinkView, ViewResolveScope,
};
pub use snapshots::{
    DefinitionSnapshot, ExportDefinitionSnapshotResponse, SnapshotBlobRef, SnapshotPayload,
    SnapshotRef, SnapshotSchemaVersion,
};

/// Minimal error response placeholder used before the full protocol surface is implemented.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceholderContract {
    /// Reserved message that proves the contracts crate is wired into the workspace.
    pub message: String,
}

impl Default for PlaceholderContract {
    fn default() -> Self {
        Self {
            message: "method-library contracts placeholder".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GatewayHeaders, PlaceholderContract};
    use method_library_domain::content::ActorKind;

    #[test]
    fn serializes_placeholder_contract() {
        let payload = serde_json::to_string(&PlaceholderContract::default())
            .expect("placeholder contract should serialize");

        assert!(payload.contains("contracts placeholder"));
    }

    #[test]
    fn reexports_gateway_headers() {
        let headers = GatewayHeaders {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            idempotency_key: Some("idem-1".to_string()),
            actor_id: "actor-1".to_string(),
            actor_kind: ActorKind::Human,
            trusted_by: "gateway".to_string(),
        };

        assert_eq!(headers.actor_id, "actor-1");
    }
}
