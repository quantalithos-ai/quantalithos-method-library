//! Read-only query services for P0 method-library query flows.

use std::sync::Arc;

use method_library_contracts::{
    ActorContext, ContentRefView, ContentVersionView, DefinitionTraceView,
    ExportDefinitionSnapshotQuery, ExportDefinitionSnapshotResponse, GetDefinitionTraceQuery,
    GetDefinitionTraceResponse, GetMethodContentQuery, GetMethodContentResponse,
    GetMethodContentVersionQuery, GetMethodContentVersionResponse, ListMethodContentsQuery,
    ListMethodContentsResponse, MethodContentView, PageInfo, ReadConsistency, ReadMode, ReadSource,
    RequestMeta, ResolveViewProfileQuery, ResolveViewProfileResponse, ResolvedViewProfile,
};
use method_library_domain::content::{
    ContentRef, ContentVersion, MethodContent, MethodContentKind, PublishedContentRef,
};
use method_library_domain::definitions::MethodContentPayload;
use method_library_domain::policies::{
    ViewProfileCandidate, ViewProfileMatchPolicy, ViewProfileResolveRequest,
};
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};
use time::Duration;

use crate::ports::{
    CheckpointStatus, Clock, ContentSummaryProjectionRepository, DefinitionSnapshotRepository,
    DefinitionTraceProjectionRepository, MethodContentReferenceRepository, MethodContentRepository,
    MethodContentVersionRepository, ObjectStoragePort, PageRequest, ProjectionCheckpointRepository,
};

const API_SCHEMA_VERSION: &str = "1.0";
const CONTENT_SUMMARY_PROJECTION_CHECKPOINT: &str = "content_summary_projection";
const DEFINITION_TRACE_PROJECTION_CHECKPOINT: &str = "definition_trace_projection";
const DEFAULT_PAGE_SIZE: u32 = 50;
const MAX_PAGE_SIZE: u32 = 100;
const TRACE_TIMELINE_PAGE_SIZE: usize = DEFAULT_PAGE_SIZE as usize;

/// Application service for read-only method-content queries.
#[derive(Clone)]
pub struct MethodContentQueryService {
    method_content_repository: Arc<dyn MethodContentRepository>,
    method_content_reference_repository: Arc<dyn MethodContentReferenceRepository>,
    method_content_version_repository: Arc<dyn MethodContentVersionRepository>,
    definition_snapshot_repository: Arc<dyn DefinitionSnapshotRepository>,
    object_storage_port: Arc<dyn ObjectStoragePort>,
    content_summary_projection_repository: Arc<dyn ContentSummaryProjectionRepository>,
    definition_trace_projection_repository: Arc<dyn DefinitionTraceProjectionRepository>,
    projection_checkpoint_repository: Arc<dyn ProjectionCheckpointRepository>,
    clock: Arc<dyn Clock>,
}

impl MethodContentQueryService {
    /// Creates a query service from read-only port implementations.
    #[must_use]
    pub fn new(
        method_content_repository: Arc<dyn MethodContentRepository>,
        method_content_reference_repository: Arc<dyn MethodContentReferenceRepository>,
        method_content_version_repository: Arc<dyn MethodContentVersionRepository>,
        definition_snapshot_repository: Arc<dyn DefinitionSnapshotRepository>,
        object_storage_port: Arc<dyn ObjectStoragePort>,
        content_summary_projection_repository: Arc<dyn ContentSummaryProjectionRepository>,
        definition_trace_projection_repository: Arc<dyn DefinitionTraceProjectionRepository>,
        projection_checkpoint_repository: Arc<dyn ProjectionCheckpointRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            method_content_repository,
            method_content_reference_repository,
            method_content_version_repository,
            definition_snapshot_repository,
            object_storage_port,
            content_summary_projection_repository,
            definition_trace_projection_repository,
            projection_checkpoint_repository,
            clock,
        }
    }

    /// Returns one method-content read model without mutating truth or durable logs.
    pub async fn get_method_content(
        &self,
        query: GetMethodContentQuery,
        _actor: ActorContext,
        _meta: RequestMeta,
    ) -> Result<GetMethodContentResponse, MethodLibraryError> {
        let content = self
            .method_content_repository
            .get(query.content_id.clone())
            .await?
            .ok_or_else(|| method_content_not_found(query.content_id.clone()))?;

        if query.read_mode == ReadMode::Published && !content.is_published_like() {
            return Err(method_content_not_found(content.content_id.clone()));
        }

        let references = if query.include_references {
            match query.read_mode {
                ReadMode::Published if content.is_published_like() => self
                    .method_content_reference_repository
                    .get_published_refs(content.content_id.clone())
                    .await?
                    .iter()
                    .map(build_published_content_ref_view)
                    .collect(),
                _ => content
                    .references
                    .iter()
                    .map(build_draft_content_ref_view)
                    .collect(),
            }
        } else {
            Vec::new()
        };

        Ok(GetMethodContentResponse {
            schema_version: API_SCHEMA_VERSION.to_string(),
            content: build_method_content_view(&content, query.include_payload, references),
            consistency: write_model_consistency(),
        })
    }

    /// Lists method-content summaries from the read projection with stable pagination.
    pub async fn list_method_contents(
        &self,
        query: ListMethodContentsQuery,
        _actor: ActorContext,
        _meta: RequestMeta,
    ) -> Result<ListMethodContentsResponse, MethodLibraryError> {
        validate_sort_key(query.sort.as_deref())?;
        let limit = normalize_page_limit(query.limit)?;
        let page = PageRequest {
            cursor: normalize_cursor(query.cursor.as_ref())?,
            limit: limit.saturating_add(1),
        };
        let mut items = self
            .content_summary_projection_repository
            .list(&query, &page)
            .await?;
        let has_more = items.len() > limit as usize;
        if has_more {
            items.truncate(limit as usize);
        }
        let next_cursor = has_more.then(|| {
            items
                .last()
                .expect("paged results must retain at least one item")
                .content_id
                .clone()
        });

        Ok(ListMethodContentsResponse {
            schema_version: API_SCHEMA_VERSION.to_string(),
            items,
            page: PageInfo {
                next_cursor,
                has_more,
            },
            consistency: self.summary_projection_consistency().await?,
        })
    }

    /// Returns one published version record without opening a write transaction.
    pub async fn get_method_content_version(
        &self,
        query: GetMethodContentVersionQuery,
        _actor: ActorContext,
        _meta: RequestMeta,
    ) -> Result<GetMethodContentVersionResponse, MethodLibraryError> {
        let record = self
            .method_content_version_repository
            .get(query.content_id.clone(), query.version.clone())
            .await?
            .ok_or_else(|| content_version_not_found(query.content_id.clone(), query.version))?;
        let snapshot_ref = if query.include_snapshot_ref {
            self.definition_snapshot_repository
                .get(record.snapshot_id.clone())
                .await?
                .map(|snapshot| snapshot.snapshot_ref())
        } else {
            None
        };

        Ok(GetMethodContentVersionResponse {
            schema_version: API_SCHEMA_VERSION.to_string(),
            content_version: ContentVersionView {
                content_id: record.content_id,
                content_family_id: record.content_family_id,
                version: record.version,
                fingerprint: record.fingerprint,
                snapshot_ref,
                published_at: record.published_at,
            },
            consistency: write_model_consistency(),
        })
    }

    /// Exports one immutable definition snapshot without mutating truth or durable logs.
    pub async fn export_definition_snapshot(
        &self,
        query: ExportDefinitionSnapshotQuery,
        _actor: ActorContext,
        meta: RequestMeta,
    ) -> Result<ExportDefinitionSnapshotResponse, MethodLibraryError> {
        let snapshot = self.resolve_snapshot_query(&query).await?;
        let payload = self
            .object_storage_port
            .get_snapshot_payload(snapshot.blob_ref.clone(), meta)
            .await?;
        if query.verify_fingerprint {
            verify_snapshot_payload(&snapshot, &payload)?;
        }

        Ok(ExportDefinitionSnapshotResponse {
            schema_version: API_SCHEMA_VERSION.to_string(),
            snapshot_ref: snapshot.snapshot_ref(),
            content_ref: snapshot.content_ref.clone(),
            references: snapshot.references.clone(),
            generated_at: payload.generated_at,
            payload,
        })
    }

    /// Resolves the published view profile that best matches the request.
    pub async fn resolve_view_profile(
        &self,
        query: ResolveViewProfileQuery,
        _actor: ActorContext,
        _meta: RequestMeta,
    ) -> Result<ResolveViewProfileResponse, MethodLibraryError> {
        if query.role_ref.kind != MethodContentKind::RoleDefinition {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::ReferenceInvalid,
                "view-profile resolution requires a role-definition reference",
            )
            .with_detail("role_kind", query.role_ref.kind.as_str()));
        }

        let profiles = self
            .method_content_repository
            .find_published_by_kind(MethodContentKind::ViewProfile)
            .await?;
        let candidates = profiles
            .iter()
            .map(build_view_profile_candidate)
            .collect::<Result<Vec<_>, _>>()?;
        let request = ViewProfileResolveRequest {
            role_ref: query.role_ref,
            object_kind: query.object_kind,
            scope: query.scope,
        };

        let view_profile = match ViewProfileMatchPolicy::match_profile(&request, &candidates) {
            Ok(result) => Some(ResolvedViewProfile {
                view_profile_ref: result.view_profile_ref,
                scope_rules: result.scope_rules,
                field_rules: result.field_rules,
                action_rules: result.action_rules,
            }),
            Err(error) if error.code == MethodLibraryErrorCode::MethodContentNotFound => None,
            Err(error) => return Err(error),
        };

        Ok(ResolveViewProfileResponse {
            schema_version: API_SCHEMA_VERSION.to_string(),
            view_profile,
            consistency: write_model_consistency(),
        })
    }

    /// Returns one definition-trace projection without mutating truth or durable logs.
    pub async fn get_definition_trace(
        &self,
        query: GetDefinitionTraceQuery,
        _actor: ActorContext,
        _meta: RequestMeta,
    ) -> Result<GetDefinitionTraceResponse, MethodLibraryError> {
        let Some(mut trace) = self
            .definition_trace_projection_repository
            .get(query.content_id.clone())
            .await?
        else {
            return Err(self
                .trace_projection_missing_error(query.content_id.clone())
                .await?);
        };
        let page = apply_trace_query(&query, &mut trace)?;

        Ok(GetDefinitionTraceResponse {
            schema_version: API_SCHEMA_VERSION.to_string(),
            content_id: query.content_id,
            trace,
            page,
            consistency: self
                .projection_consistency(DEFINITION_TRACE_PROJECTION_CHECKPOINT)
                .await?,
        })
    }

    async fn summary_projection_consistency(&self) -> Result<ReadConsistency, MethodLibraryError> {
        self.projection_consistency(CONTENT_SUMMARY_PROJECTION_CHECKPOINT)
            .await
    }

    async fn projection_consistency(
        &self,
        checkpoint_name: &str,
    ) -> Result<ReadConsistency, MethodLibraryError> {
        let checkpoint = self
            .projection_checkpoint_repository
            .get(checkpoint_name.to_string())
            .await?;
        let now = self.clock.now();

        let (stale, checkpoint) = checkpoint.map_or((false, None), |record| {
            let stale = record.status != CheckpointStatus::Active
                || record.updated_at + projection_stale_threshold() < now;
            (stale, record.last_processed_event_id)
        });

        Ok(ReadConsistency {
            source: ReadSource::Projection,
            stale,
            checkpoint,
        })
    }

    async fn resolve_snapshot_query(
        &self,
        query: &ExportDefinitionSnapshotQuery,
    ) -> Result<method_library_contracts::DefinitionSnapshot, MethodLibraryError> {
        match (&query.snapshot_id, &query.content_id, &query.version) {
            (Some(snapshot_id), None, None) => self
                .definition_snapshot_repository
                .get(snapshot_id.clone())
                .await?
                .ok_or_else(|| snapshot_not_found(snapshot_id.clone())),
            (None, Some(content_id), Some(version)) => self
                .definition_snapshot_repository
                .get_by_content_version(content_id.clone(), version.clone())
                .await?
                .ok_or_else(|| snapshot_not_found_for_version(content_id.clone(), version.clone())),
            (Some(_), Some(_), _) | (Some(_), None, Some(_)) => {
                Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::FilterInvalid,
                    "snapshot export requires either snapshot_id or content_id plus version",
                ))
            }
            (None, Some(_), None) | (None, None, Some(_)) | (None, None, None) => {
                Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::FilterInvalid,
                    "snapshot export requires either snapshot_id or content_id plus version",
                ))
            }
        }
    }

    async fn trace_projection_missing_error(
        &self,
        content_id: String,
    ) -> Result<MethodLibraryError, MethodLibraryError> {
        if self
            .method_content_repository
            .get(content_id.clone())
            .await?
            .is_some()
        {
            Ok(MethodLibraryError::retryable(
                MethodLibraryErrorCode::ProjectionNotReady,
                "definition trace projection is not ready for the requested content",
            )
            .with_detail("content_id", content_id))
        } else {
            Ok(method_content_not_found(content_id))
        }
    }
}

fn validate_sort_key(sort: Option<&str>) -> Result<(), MethodLibraryError> {
    match sort {
        None | Some("content_id") => Ok(()),
        Some(other) => Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::FilterInvalid,
            format!("unsupported list sort key: {other}"),
        )),
    }
}

fn projection_stale_threshold() -> Duration {
    Duration::minutes(5)
}

fn normalize_page_limit(limit: u32) -> Result<u32, MethodLibraryError> {
    let normalized = if limit == 0 { DEFAULT_PAGE_SIZE } else { limit };
    if normalized > MAX_PAGE_SIZE {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::PageLimitExceeded,
            format!("page size must not exceed {MAX_PAGE_SIZE}"),
        )
        .with_detail("max_page_size", MAX_PAGE_SIZE.to_string())
        .with_detail("requested_page_size", normalized.to_string()));
    }

    Ok(normalized)
}

fn normalize_cursor(cursor: Option<&String>) -> Result<Option<String>, MethodLibraryError> {
    match cursor {
        None => Ok(None),
        Some(cursor) if cursor.trim().is_empty() => Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::FilterInvalid,
            "cursor must not be empty when provided",
        )),
        Some(cursor) => Ok(Some(cursor.clone())),
    }
}

fn build_method_content_view(
    content: &MethodContent,
    include_payload: bool,
    references: Vec<ContentRefView>,
) -> MethodContentView {
    MethodContentView {
        content_id: content.content_id.clone(),
        content_family_id: content.content_family_id.clone(),
        kind: content.kind,
        name: content.name.clone(),
        description: content.description.clone(),
        lifecycle_state: content.lifecycle.state,
        version: content.version.clone(),
        fingerprint: content.fingerprint.clone(),
        payload: include_payload.then(|| content.payload.clone()),
        references,
        revision: content.revision,
    }
}

fn build_draft_content_ref_view(reference: &ContentRef) -> ContentRefView {
    ContentRefView {
        target_content_id: reference.target_content_id.clone(),
        target_kind: reference.target_kind,
        target_version: None,
        target_fingerprint: None,
    }
}

fn build_published_content_ref_view(reference: &PublishedContentRef) -> ContentRefView {
    ContentRefView {
        target_content_id: reference.content_id.clone(),
        target_kind: reference.kind,
        target_version: Some(reference.version.clone()),
        target_fingerprint: Some(reference.fingerprint.clone()),
    }
}

fn write_model_consistency() -> ReadConsistency {
    ReadConsistency {
        source: ReadSource::WriteModel,
        stale: false,
        checkpoint: None,
    }
}

fn method_content_not_found(content_id: String) -> MethodLibraryError {
    MethodLibraryError::validation(
        MethodLibraryErrorCode::MethodContentNotFound,
        "method content does not exist",
    )
    .with_detail("content_id", content_id)
}

fn content_version_not_found(content_id: String, version: ContentVersion) -> MethodLibraryError {
    MethodLibraryError::validation(
        MethodLibraryErrorCode::ContentVersionNotFound,
        "method content version does not exist",
    )
    .with_detail("content_id", content_id)
    .with_detail("version", version.raw)
}

fn snapshot_not_found(snapshot_id: String) -> MethodLibraryError {
    MethodLibraryError::validation(
        MethodLibraryErrorCode::SnapshotNotFound,
        "definition snapshot does not exist",
    )
    .with_detail("snapshot_id", snapshot_id)
}

fn snapshot_not_found_for_version(
    content_id: String,
    version: ContentVersion,
) -> MethodLibraryError {
    MethodLibraryError::validation(
        MethodLibraryErrorCode::SnapshotNotFound,
        "definition snapshot does not exist for the requested content version",
    )
    .with_detail("content_id", content_id)
    .with_detail("version", version.raw)
}

fn verify_snapshot_payload(
    snapshot: &method_library_contracts::DefinitionSnapshot,
    payload: &method_library_contracts::SnapshotPayload,
) -> Result<(), MethodLibraryError> {
    if payload.schema_version != snapshot.schema_version {
        return Err(snapshot_fingerprint_mismatch(
            "payload.schema_version",
            snapshot.schema_version.clone(),
            payload.schema_version.clone(),
        ));
    }
    if payload.content.content_id != snapshot.content_id {
        return Err(snapshot_fingerprint_mismatch(
            "payload.content.content_id",
            snapshot.content_id.clone(),
            payload.content.content_id.clone(),
        ));
    }
    if payload.content.version.as_ref() != Some(&snapshot.version) {
        return Err(snapshot_fingerprint_mismatch(
            "payload.content.version",
            snapshot.version.raw.clone(),
            payload
                .content
                .version
                .as_ref()
                .map(|version| version.raw.clone())
                .unwrap_or_default(),
        ));
    }
    if payload.content.fingerprint.as_ref() != Some(&snapshot.fingerprint) {
        return Err(snapshot_fingerprint_mismatch(
            "payload.content.fingerprint",
            snapshot.fingerprint.value.clone(),
            payload
                .content
                .fingerprint
                .as_ref()
                .map(|fingerprint| fingerprint.value.clone())
                .unwrap_or_default(),
        ));
    }
    if payload.references != snapshot.references {
        return Err(snapshot_fingerprint_mismatch(
            "payload.references",
            "metadata references".to_string(),
            "payload references".to_string(),
        ));
    }

    let payload_reference_views = snapshot
        .references
        .iter()
        .map(build_published_content_ref_view)
        .collect::<Vec<_>>();
    if payload.content.references != payload_reference_views {
        return Err(snapshot_fingerprint_mismatch(
            "payload.content.references",
            "published reference views".to_string(),
            "mismatched content reference views".to_string(),
        ));
    }

    Ok(())
}

fn snapshot_fingerprint_mismatch(
    field: &str,
    expected: String,
    actual: String,
) -> MethodLibraryError {
    MethodLibraryError::validation(
        MethodLibraryErrorCode::FingerprintMismatch,
        "snapshot payload does not match its stored fingerprint metadata",
    )
    .with_detail("field", field.to_string())
    .with_detail("expected", expected)
    .with_detail("actual", actual)
}

fn build_view_profile_candidate(
    content: &MethodContent,
) -> Result<ViewProfileCandidate, MethodLibraryError> {
    let MethodContentPayload::ViewProfile(profile) = &content.payload else {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::PayloadKindMismatch,
            "published view-profile content must carry a view-profile payload",
        )
        .with_detail("content_id", content.content_id.clone()));
    };

    Ok(ViewProfileCandidate {
        content_ref: PublishedContentRef {
            content_id: content.content_id.clone(),
            kind: content.kind,
            version: content.version.clone().ok_or_else(|| {
                MethodLibraryError::validation(
                    MethodLibraryErrorCode::PublishedContentImmutable,
                    "published view profile is missing its version",
                )
                .with_detail("content_id", content.content_id.clone())
            })?,
            fingerprint: content.fingerprint.clone().ok_or_else(|| {
                MethodLibraryError::validation(
                    MethodLibraryErrorCode::PublishedContentImmutable,
                    "published view profile is missing its fingerprint",
                )
                .with_detail("content_id", content.content_id.clone())
            })?,
        },
        profile: profile.clone(),
    })
}

fn apply_trace_query(
    query: &GetDefinitionTraceQuery,
    trace: &mut DefinitionTraceView,
) -> Result<Option<PageInfo>, MethodLibraryError> {
    if !query.include_audit {
        trace.audit_records.clear();
    }
    if !query.include_events {
        trace.outbox_events.clear();
    }

    if !query.include_audit && !query.include_events {
        if query.cursor.is_some() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::FilterInvalid,
                "trace cursor is only supported when audit or event summaries are included",
            ));
        }

        return Ok(None);
    }

    let start = normalize_trace_cursor(query.cursor.as_ref())?;
    let mut timeline = trace
        .audit_records
        .drain(..)
        .map(TraceTimelineItem::Audit)
        .chain(trace.outbox_events.drain(..).map(TraceTimelineItem::Event))
        .collect::<Vec<_>>();
    timeline.sort_by(|left, right| right.sort_key().cmp(&left.sort_key()));

    if start > timeline.len() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::FilterInvalid,
            "trace cursor is outside the available range",
        )
        .with_detail("cursor", start.to_string()));
    }

    let end = (start + TRACE_TIMELINE_PAGE_SIZE).min(timeline.len());
    let has_more = end < timeline.len();
    let next_cursor = has_more.then(|| end.to_string());
    for item in timeline
        .into_iter()
        .skip(start)
        .take(TRACE_TIMELINE_PAGE_SIZE)
    {
        match item {
            TraceTimelineItem::Audit(record) => trace.audit_records.push(record),
            TraceTimelineItem::Event(event) => trace.outbox_events.push(event),
        }
    }

    Ok(Some(PageInfo {
        next_cursor,
        has_more,
    }))
}

fn normalize_trace_cursor(cursor: Option<&String>) -> Result<usize, MethodLibraryError> {
    match cursor {
        None => Ok(0),
        Some(cursor) if cursor.trim().is_empty() => Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::FilterInvalid,
            "trace cursor must not be empty when provided",
        )),
        Some(cursor) => cursor.parse::<usize>().map_err(|_| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::FilterInvalid,
                "trace cursor must be a non-negative integer offset",
            )
            .with_detail("cursor", cursor.clone())
        }),
    }
}

enum TraceTimelineItem {
    Audit(method_library_contracts::AuditRecordView),
    Event(method_library_contracts::OutboxEventView),
}

impl TraceTimelineItem {
    fn sort_key(&self) -> (method_library_domain::content::Timestamp, &str, &str) {
        match self {
            Self::Audit(record) => (record.occurred_at, "audit", record.request_id.as_str()),
            Self::Event(event) => (event.occurred_at, "event", event.event_id.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use time::macros::datetime;

    use super::MethodContentQueryService;
    use crate::ports::fakes::{
        DeterministicClock, FakeUnitOfWork, InMemoryContentSummaryProjectionRepository,
        InMemoryDefinitionSnapshotRepository, InMemoryDefinitionTraceProjectionRepository,
        InMemoryMethodContentReferenceRepository, InMemoryMethodContentRepository,
        InMemoryMethodContentVersionRepository, InMemoryObjectStorage,
        InMemoryProjectionCheckpointRepository,
    };
    use crate::ports::{
        ContentSummaryProjectionRepository, DefinitionSnapshotRepository,
        DefinitionTraceProjectionRepository, MethodContentReferenceRepository,
        MethodContentRepository, MethodContentVersionRecord, MethodContentVersionRepository,
        ProjectionCheckpointRepository, UnitOfWork,
    };
    use method_library_contracts::{
        ActorContext, ActorRef, AuditRecordView, ContentSummaryView, DefinitionEventType,
        DefinitionSnapshot, DefinitionTraceView, ExportDefinitionSnapshotQuery,
        GetDefinitionTraceQuery, GetMethodContentQuery, GetMethodContentVersionQuery,
        LifecycleHistoryEntryView, ListMethodContentsQuery, MethodContentView, OutboxEventView,
        ReadMode, ReadSource, RequestMeta, ResolveViewProfileQuery, SnapshotPayload, SnapshotRef,
        SupersedeLinkView,
    };
    use method_library_domain::MethodLibraryErrorCode;
    use method_library_domain::content::{
        ActorKind, ApprovedGateRef, CanonicalFingerprint, ContentRef, ContentVersion,
        FingerprintAlgorithm, LifecycleState, MethodContent, MethodContentKind,
        PublishedContentRef, ReferenceState,
    };
    use method_library_domain::definitions::{
        ActionAvailability, EvidenceKind, EvidenceRule, FieldVisibility, MethodContentPayload,
        Qualification, QualificationLevel, QualificationLevelModel, ViewActionRule, ViewCondition,
        ViewConditionOperator, ViewFieldRule, ViewObjectKind, ViewProfile, ViewScopeRule,
    };

    struct TestHarness {
        service: MethodContentQueryService,
        content_repository: Arc<InMemoryMethodContentRepository>,
        reference_repository: Arc<InMemoryMethodContentReferenceRepository>,
        version_repository: Arc<InMemoryMethodContentVersionRepository>,
        snapshot_repository: Arc<InMemoryDefinitionSnapshotRepository>,
        trace_repository: Arc<InMemoryDefinitionTraceProjectionRepository>,
        object_storage: Arc<InMemoryObjectStorage>,
        summary_repository: Arc<InMemoryContentSummaryProjectionRepository>,
        checkpoint_repository: Arc<InMemoryProjectionCheckpointRepository>,
    }

    fn sample_harness() -> TestHarness {
        let content_repository = Arc::new(InMemoryMethodContentRepository::default());
        let reference_repository = Arc::new(InMemoryMethodContentReferenceRepository::default());
        let version_repository = Arc::new(InMemoryMethodContentVersionRepository::default());
        let snapshot_repository = Arc::new(InMemoryDefinitionSnapshotRepository::default());
        let trace_repository = Arc::new(InMemoryDefinitionTraceProjectionRepository::default());
        let object_storage = Arc::new(InMemoryObjectStorage::default());
        let summary_repository = Arc::new(InMemoryContentSummaryProjectionRepository::default());
        let checkpoint_repository = Arc::new(InMemoryProjectionCheckpointRepository::default());
        let clock = Arc::new(DeterministicClock::new(datetime!(2026-05-26 08:10:00 UTC)));

        let service = MethodContentQueryService::new(
            content_repository.clone(),
            reference_repository.clone(),
            version_repository.clone(),
            snapshot_repository.clone(),
            object_storage.clone(),
            summary_repository.clone(),
            trace_repository.clone(),
            checkpoint_repository.clone(),
            clock,
        );

        TestHarness {
            service,
            content_repository,
            reference_repository,
            version_repository,
            snapshot_repository,
            trace_repository,
            object_storage,
            summary_repository,
            checkpoint_repository,
        }
    }

    fn sample_actor() -> ActorContext {
        ActorContext {
            actor_id: "actor-1".to_string(),
            actor_kind: ActorKind::Human,
            actor_ref: ActorRef {
                actor_id: "actor-1".to_string(),
                actor_kind: ActorKind::Human,
            },
        }
    }

    fn sample_meta() -> RequestMeta {
        RequestMeta {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            idempotency_key: None,
            request_hash: "hash-1".to_string(),
            received_at: datetime!(2026-05-26 08:10:00 UTC),
        }
    }

    fn sample_payload() -> MethodContentPayload {
        MethodContentPayload::Qualification(Qualification {
            qualification_key: "quality-1".to_string(),
            name: "Quality".to_string(),
            description: Some("Definition".to_string()),
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
        })
    }

    async fn insert_content(
        repository: &Arc<InMemoryMethodContentRepository>,
        content: MethodContent,
        idempotency_key: &str,
    ) {
        let mut tx = FakeUnitOfWork
            .begin(RequestMeta {
                request_id: format!("req-{idempotency_key}"),
                trace_id: format!("trace-{idempotency_key}"),
                idempotency_key: Some(idempotency_key.to_string()),
                request_hash: format!("hash-{idempotency_key}"),
                received_at: datetime!(2026-05-26 08:00:00 UTC),
            })
            .await
            .expect("fixture transaction should open");
        repository
            .insert(&mut tx, content)
            .await
            .expect("fixture content should insert");
        tx.commit()
            .await
            .expect("fixture transaction should commit");
    }

    async fn insert_published_refs(
        repository: &Arc<InMemoryMethodContentReferenceRepository>,
        source_content_id: &str,
        refs: Vec<PublishedContentRef>,
        idempotency_key: &str,
    ) {
        let mut tx = FakeUnitOfWork
            .begin(RequestMeta {
                request_id: format!("req-{idempotency_key}"),
                trace_id: format!("trace-{idempotency_key}"),
                idempotency_key: Some(idempotency_key.to_string()),
                request_hash: format!("hash-{idempotency_key}"),
                received_at: datetime!(2026-05-26 08:00:00 UTC),
            })
            .await
            .expect("fixture transaction should open");
        repository
            .replace_published_refs(&mut tx, source_content_id.to_string(), refs)
            .await
            .expect("fixture refs should insert");
        tx.commit()
            .await
            .expect("fixture transaction should commit");
    }

    async fn insert_version_record(
        repository: &Arc<InMemoryMethodContentVersionRepository>,
        record: MethodContentVersionRecord,
        idempotency_key: &str,
    ) {
        let mut tx = FakeUnitOfWork
            .begin(RequestMeta {
                request_id: format!("req-{idempotency_key}"),
                trace_id: format!("trace-{idempotency_key}"),
                idempotency_key: Some(idempotency_key.to_string()),
                request_hash: format!("hash-{idempotency_key}"),
                received_at: datetime!(2026-05-26 08:00:00 UTC),
            })
            .await
            .expect("fixture transaction should open");
        repository
            .insert(&mut tx, record)
            .await
            .expect("fixture version should insert");
        tx.commit()
            .await
            .expect("fixture transaction should commit");
    }

    async fn insert_snapshot(
        repository: &Arc<InMemoryDefinitionSnapshotRepository>,
        snapshot: DefinitionSnapshot,
        idempotency_key: &str,
    ) {
        let mut tx = FakeUnitOfWork
            .begin(RequestMeta {
                request_id: format!("req-{idempotency_key}"),
                trace_id: format!("trace-{idempotency_key}"),
                idempotency_key: Some(idempotency_key.to_string()),
                request_hash: format!("hash-{idempotency_key}"),
                received_at: datetime!(2026-05-26 08:00:00 UTC),
            })
            .await
            .expect("fixture transaction should open");
        repository
            .insert(&mut tx, snapshot)
            .await
            .expect("fixture snapshot should insert");
        tx.commit()
            .await
            .expect("fixture transaction should commit");
    }

    fn sample_role_ref(content_id: &str) -> PublishedContentRef {
        PublishedContentRef {
            content_id: content_id.to_string(),
            kind: MethodContentKind::RoleDefinition,
            version: ContentVersion::new("1.0.0").expect("version should be valid"),
            fingerprint: CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "role123", "1.0")
                .expect("fingerprint should be valid"),
        }
    }

    fn sample_draft_content(content_id: &str) -> MethodContent {
        let mut content = MethodContent::create_draft(
            content_id.to_string(),
            format!("family-{content_id}"),
            MethodContentKind::Qualification,
            "Quality".to_string(),
            Some("Definition".to_string()),
            sample_payload(),
            "actor-1".to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("draft should build");
        content
            .replace_references(
                vec![ContentRef {
                    target_content_id: "content-ref-1".to_string(),
                    target_kind: MethodContentKind::Qualification,
                    required_state: ReferenceState::Published,
                }],
                "actor-1".to_string(),
                datetime!(2026-05-26 08:01:00 UTC),
            )
            .expect("draft refs should update");
        content
    }

    fn sample_published_content(content_id: &str) -> MethodContent {
        let mut content = sample_draft_content(content_id);
        content
            .submit_for_review("actor-1".to_string(), datetime!(2026-05-26 08:02:00 UTC))
            .expect("draft should enter review");
        content
            .publish(
                ApprovedGateRef {
                    gate_id: "gate-1".to_string(),
                    gate_decision_id: "decision-1".to_string(),
                    approved_at: datetime!(2026-05-26 08:03:00 UTC),
                },
                ContentVersion::new("1.0.0").expect("version should be valid"),
                CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "abc123", "1.0")
                    .expect("fingerprint should be valid"),
                "actor-1".to_string(),
                datetime!(2026-05-26 08:03:00 UTC),
            )
            .expect("reviewed content should publish");
        content
    }

    fn sample_view_profile_content(
        content_id: &str,
        scope_key: &str,
        project_type: &str,
    ) -> MethodContent {
        let mut content = MethodContent::create_draft(
            content_id.to_string(),
            format!("family-{content_id}"),
            MethodContentKind::ViewProfile,
            format!("View Profile {content_id}"),
            Some("View profile".to_string()),
            MethodContentPayload::ViewProfile(ViewProfile {
                role_ref: sample_role_ref("role-1"),
                object_kind: ViewObjectKind::WorkItem,
                scope_rules: vec![ViewScopeRule {
                    scope_key: scope_key.to_string(),
                    conditions: vec![ViewCondition {
                        field_path: "project.type".to_string(),
                        operator: ViewConditionOperator::Eq,
                        value_json: Some(json!(project_type)),
                    }],
                }],
                field_rules: vec![ViewFieldRule {
                    field_path: "summary".to_string(),
                    visibility: FieldVisibility::Visible,
                }],
                action_rules: vec![ViewActionRule {
                    action_key: "edit".to_string(),
                    availability: ActionAvailability::Available,
                }],
            }),
            "actor-1".to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("view profile should build");
        content
            .submit_for_review("actor-1".to_string(), datetime!(2026-05-26 08:01:00 UTC))
            .expect("view profile should enter review");
        content
            .publish(
                ApprovedGateRef {
                    gate_id: "gate-1".to_string(),
                    gate_decision_id: "decision-1".to_string(),
                    approved_at: datetime!(2026-05-26 08:02:00 UTC),
                },
                ContentVersion::new("1.0.0").expect("version should be valid"),
                CanonicalFingerprint::new(
                    FingerprintAlgorithm::Sha256,
                    format!("vp-{content_id}"),
                    "1.0",
                )
                .expect("fingerprint should be valid"),
                "actor-1".to_string(),
                datetime!(2026-05-26 08:02:00 UTC),
            )
            .expect("view profile should publish");
        content
    }

    fn sample_trace_view(content_id: &str) -> DefinitionTraceView {
        DefinitionTraceView {
            content_id: content_id.to_string(),
            versions: vec![method_library_contracts::ContentVersionView {
                content_id: content_id.to_string(),
                content_family_id: format!("family-{content_id}"),
                version: ContentVersion::new("1.0.0").expect("version should be valid"),
                fingerprint: CanonicalFingerprint::new(
                    FingerprintAlgorithm::Sha256,
                    "trace123",
                    "1.0",
                )
                .expect("fingerprint should be valid"),
                snapshot_ref: Some(SnapshotRef {
                    snapshot_id: format!("snapshot-{content_id}"),
                    schema_version: "1.0".to_string(),
                    blob_ref: format!("object://snapshot-{content_id}"),
                }),
                published_at: datetime!(2026-05-26 08:02:00 UTC),
            }],
            lifecycle_history: vec![LifecycleHistoryEntryView {
                from_state: Some(LifecycleState::InReview),
                to_state: LifecycleState::Published,
                actor_id: "actor-1".to_string(),
                reason: Some("Published".to_string()),
                created_at: datetime!(2026-05-26 08:02:00 UTC),
            }],
            audit_records: vec![AuditRecordView {
                request_id: "req-trace".to_string(),
                trace_id: "trace-trace".to_string(),
                actor_ref: ActorRef {
                    actor_id: "actor-1".to_string(),
                    actor_kind: ActorKind::Human,
                },
                action: "publish".to_string(),
                result: "succeeded".to_string(),
                occurred_at: datetime!(2026-05-26 08:02:00 UTC),
            }],
            outbox_events: vec![OutboxEventView {
                event_id: "evt-trace".to_string(),
                event_type: DefinitionEventType::ContentPublished,
                occurred_at: datetime!(2026-05-26 08:02:00 UTC),
                snapshot_ref: Some(SnapshotRef {
                    snapshot_id: format!("snapshot-{content_id}"),
                    schema_version: "1.0".to_string(),
                    blob_ref: format!("object://snapshot-{content_id}"),
                }),
            }],
            supersede_chain: vec![SupersedeLinkView {
                old_content_id: content_id.to_string(),
                new_content_id: "content-next".to_string(),
                reason: "Replaced".to_string(),
                created_at: datetime!(2026-05-26 08:05:00 UTC),
            }],
        }
    }

    #[tokio::test]
    async fn gets_published_content_from_the_write_model() {
        let harness = sample_harness();
        insert_content(
            &harness.content_repository,
            sample_published_content("content-1"),
            "seed-content",
        )
        .await;
        insert_published_refs(
            &harness.reference_repository,
            "content-1",
            vec![PublishedContentRef {
                content_id: "content-ref-1".to_string(),
                kind: MethodContentKind::Qualification,
                version: ContentVersion::new("1.0.0").expect("version should be valid"),
                fingerprint: CanonicalFingerprint::new(
                    FingerprintAlgorithm::Sha256,
                    "ref123",
                    "1.0",
                )
                .expect("fingerprint should be valid"),
            }],
            "seed-refs",
        )
        .await;

        let response = harness
            .service
            .get_method_content(
                GetMethodContentQuery {
                    content_id: "content-1".to_string(),
                    read_mode: ReadMode::Published,
                    include_payload: true,
                    include_references: true,
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect("published content should load");

        assert_eq!(response.schema_version, "1.0");
        assert_eq!(response.consistency.source, ReadSource::WriteModel);
        assert_eq!(response.content.lifecycle_state, LifecycleState::Published);
        assert!(response.content.payload.is_some());
        assert_eq!(response.content.references.len(), 1);
        assert_eq!(
            response.content.references[0]
                .target_version
                .as_ref()
                .map(|version| version.raw.as_str()),
            Some("1.0.0")
        );
    }

    #[tokio::test]
    async fn gets_draft_content_in_authoring_mode() {
        let harness = sample_harness();
        insert_content(
            &harness.content_repository,
            sample_draft_content("content-1"),
            "seed-draft",
        )
        .await;

        let response = harness
            .service
            .get_method_content(
                GetMethodContentQuery {
                    content_id: "content-1".to_string(),
                    read_mode: ReadMode::Authoring,
                    include_payload: false,
                    include_references: true,
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect("draft should load for authoring reads");

        assert_eq!(response.content.lifecycle_state, LifecycleState::Draft);
        assert!(response.content.payload.is_none());
        assert_eq!(response.content.references.len(), 1);
    }

    #[tokio::test]
    async fn hides_draft_content_from_published_reads() {
        let harness = sample_harness();
        insert_content(
            &harness.content_repository,
            sample_draft_content("content-1"),
            "seed-draft",
        )
        .await;

        let error = harness
            .service
            .get_method_content(
                GetMethodContentQuery {
                    content_id: "content-1".to_string(),
                    read_mode: ReadMode::Published,
                    include_payload: true,
                    include_references: true,
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect_err("drafts should be hidden from published reads");

        assert_eq!(error.code, MethodLibraryErrorCode::MethodContentNotFound);
    }

    #[tokio::test]
    async fn lists_projection_items_with_pagination_and_consistency() {
        let harness = sample_harness();
        harness
            .summary_repository
            .upsert(ContentSummaryView {
                content_id: "content-1".to_string(),
                kind: MethodContentKind::Qualification,
                name: "Alpha".to_string(),
                lifecycle_state: LifecycleState::Published,
                version: Some(ContentVersion::new("1.0.0").expect("version should be valid")),
                updated_at: datetime!(2026-05-26 08:00:00 UTC),
            })
            .await
            .expect("first projection should upsert");
        harness
            .summary_repository
            .upsert(ContentSummaryView {
                content_id: "content-2".to_string(),
                kind: MethodContentKind::Qualification,
                name: "Beta".to_string(),
                lifecycle_state: LifecycleState::Deprecated,
                version: Some(ContentVersion::new("1.1.0").expect("version should be valid")),
                updated_at: datetime!(2026-05-26 08:01:00 UTC),
            })
            .await
            .expect("second projection should upsert");
        harness
            .checkpoint_repository
            .advance_if_current(
                "content_summary_projection".to_string(),
                None,
                "evt-2".to_string(),
                datetime!(2026-05-26 08:08:00 UTC),
            )
            .await
            .expect("checkpoint should advance");

        let response = harness
            .service
            .list_method_contents(
                ListMethodContentsQuery {
                    kind: Some(MethodContentKind::Qualification),
                    lifecycle_state: None,
                    read_mode: ReadMode::Published,
                    cursor: None,
                    limit: 1,
                    sort: Some("content_id".to_string()),
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect("list query should succeed");

        assert_eq!(response.schema_version, "1.0");
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].content_id, "content-1");
        assert!(response.page.has_more);
        assert_eq!(response.page.next_cursor.as_deref(), Some("content-1"));
        assert_eq!(response.consistency.source, ReadSource::Projection);
        assert_eq!(response.consistency.checkpoint.as_deref(), Some("evt-2"));
        assert!(!response.consistency.stale);
    }

    #[tokio::test]
    async fn rejects_page_limits_above_the_configured_maximum() {
        let harness = sample_harness();

        let error = harness
            .service
            .list_method_contents(
                ListMethodContentsQuery {
                    kind: None,
                    lifecycle_state: None,
                    read_mode: ReadMode::Published,
                    cursor: None,
                    limit: 101,
                    sort: None,
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect_err("oversized page requests should fail");

        assert_eq!(error.code, MethodLibraryErrorCode::PageLimitExceeded);
    }

    #[tokio::test]
    async fn gets_published_version_history() {
        let harness = sample_harness();
        insert_version_record(
            &harness.version_repository,
            MethodContentVersionRecord {
                content_version_id: "version-1".to_string(),
                content_id: "content-1".to_string(),
                content_family_id: "family-1".to_string(),
                version: ContentVersion::new("1.0.0").expect("version should be valid"),
                fingerprint: CanonicalFingerprint::new(
                    FingerprintAlgorithm::Sha256,
                    "abc123",
                    "1.0",
                )
                .expect("fingerprint should be valid"),
                snapshot_id: "snapshot-1".to_string(),
                published_at: datetime!(2026-05-26 08:03:00 UTC),
            },
            "seed-version",
        )
        .await;
        insert_snapshot(
            &harness.snapshot_repository,
            DefinitionSnapshot {
                snapshot_id: "snapshot-1".to_string(),
                content_id: "content-1".to_string(),
                version: ContentVersion::new("1.0.0").expect("version should be valid"),
                fingerprint: CanonicalFingerprint::new(
                    FingerprintAlgorithm::Sha256,
                    "abc123",
                    "1.0",
                )
                .expect("fingerprint should be valid"),
                schema_version: "1.0".to_string(),
                blob_ref: "object://snapshot-1".to_string(),
                created_at: datetime!(2026-05-26 08:03:00 UTC),
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
                references: Vec::new(),
            },
            "seed-snapshot",
        )
        .await;

        let response = harness
            .service
            .get_method_content_version(
                GetMethodContentVersionQuery {
                    content_id: "content-1".to_string(),
                    version: ContentVersion::new("1.0.0").expect("version should be valid"),
                    include_snapshot_ref: true,
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect("version query should succeed");

        assert_eq!(response.schema_version, "1.0");
        assert_eq!(response.consistency.source, ReadSource::WriteModel);
        assert_eq!(response.content_version.version.raw, "1.0.0");
        assert_eq!(
            response
                .content_version
                .snapshot_ref
                .as_ref()
                .map(|snapshot| snapshot.snapshot_id.as_str()),
            Some("snapshot-1")
        );
    }

    #[tokio::test]
    async fn exports_definition_snapshots_from_object_storage() {
        let harness = sample_harness();
        let content = sample_published_content("content-snapshot");
        let published_refs = vec![PublishedContentRef {
            content_id: "content-ref-1".to_string(),
            kind: MethodContentKind::Qualification,
            version: ContentVersion::new("1.0.0").expect("version should be valid"),
            fingerprint: CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "ref123", "1.0")
                .expect("fingerprint should be valid"),
        }];
        let snapshot = DefinitionSnapshot {
            snapshot_id: "snapshot-export".to_string(),
            content_id: content.content_id.clone(),
            version: content.version.clone().expect("version should exist"),
            fingerprint: content
                .fingerprint
                .clone()
                .expect("fingerprint should exist"),
            schema_version: "1.0".to_string(),
            blob_ref: "object://snapshot-export".to_string(),
            created_at: datetime!(2026-05-26 08:03:00 UTC),
            content_ref: PublishedContentRef {
                content_id: content.content_id.clone(),
                kind: content.kind,
                version: content.version.clone().expect("version should exist"),
                fingerprint: content
                    .fingerprint
                    .clone()
                    .expect("fingerprint should exist"),
            },
            references: published_refs.clone(),
        };
        insert_snapshot(
            &harness.snapshot_repository,
            snapshot.clone(),
            "seed-export-snapshot",
        )
        .await;
        harness
            .object_storage
            .insert_blob(
                snapshot.blob_ref.clone(),
                SnapshotPayload {
                    content: MethodContentView {
                        content_id: content.content_id.clone(),
                        content_family_id: content.content_family_id.clone(),
                        kind: content.kind,
                        name: content.name.clone(),
                        description: content.description.clone(),
                        lifecycle_state: LifecycleState::Published,
                        version: content.version.clone(),
                        fingerprint: content.fingerprint.clone(),
                        payload: Some(content.payload.clone()),
                        references: published_refs
                            .iter()
                            .map(super::build_published_content_ref_view)
                            .collect(),
                        revision: content.revision,
                    },
                    references: published_refs.clone(),
                    generated_at: datetime!(2026-05-26 08:03:00 UTC),
                    schema_version: "1.0".to_string(),
                },
            )
            .expect("snapshot blob should insert");

        let response = harness
            .service
            .export_definition_snapshot(
                ExportDefinitionSnapshotQuery {
                    snapshot_id: Some("snapshot-export".to_string()),
                    content_id: None,
                    version: None,
                    verify_fingerprint: true,
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect("snapshot export should succeed");

        assert_eq!(response.schema_version, "1.0");
        assert_eq!(response.snapshot_ref.snapshot_id, "snapshot-export");
        assert_eq!(response.generated_at, datetime!(2026-05-26 08:03:00 UTC));
        assert_eq!(response.references.len(), 1);
        assert_eq!(response.payload.content.content_id, "content-snapshot");
    }

    #[tokio::test]
    async fn rejects_snapshot_exports_with_fingerprint_mismatch() {
        let harness = sample_harness();
        let snapshot = DefinitionSnapshot {
            snapshot_id: "snapshot-mismatch".to_string(),
            content_id: "content-snapshot".to_string(),
            version: ContentVersion::new("1.0.0").expect("version should be valid"),
            fingerprint: CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "abc123", "1.0")
                .expect("fingerprint should be valid"),
            schema_version: "1.0".to_string(),
            blob_ref: "object://snapshot-mismatch".to_string(),
            created_at: datetime!(2026-05-26 08:03:00 UTC),
            content_ref: PublishedContentRef {
                content_id: "content-snapshot".to_string(),
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
        insert_snapshot(
            &harness.snapshot_repository,
            snapshot.clone(),
            "seed-mismatch-snapshot",
        )
        .await;
        harness
            .object_storage
            .insert_blob(
                snapshot.blob_ref.clone(),
                SnapshotPayload {
                    content: MethodContentView {
                        content_id: "content-snapshot".to_string(),
                        content_family_id: "family-content-snapshot".to_string(),
                        kind: MethodContentKind::Qualification,
                        name: "Snapshot".to_string(),
                        description: None,
                        lifecycle_state: LifecycleState::Published,
                        version: Some(
                            ContentVersion::new("1.0.0").expect("version should be valid"),
                        ),
                        fingerprint: Some(
                            CanonicalFingerprint::new(
                                FingerprintAlgorithm::Sha256,
                                "different",
                                "1.0",
                            )
                            .expect("fingerprint should be valid"),
                        ),
                        payload: Some(sample_payload()),
                        references: Vec::new(),
                        revision: 2,
                    },
                    references: Vec::new(),
                    generated_at: datetime!(2026-05-26 08:03:00 UTC),
                    schema_version: "1.0".to_string(),
                },
            )
            .expect("snapshot blob should insert");

        let error = harness
            .service
            .export_definition_snapshot(
                ExportDefinitionSnapshotQuery {
                    snapshot_id: Some("snapshot-mismatch".to_string()),
                    content_id: None,
                    version: None,
                    verify_fingerprint: true,
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect_err("mismatched snapshot payload should fail");

        assert_eq!(error.code, MethodLibraryErrorCode::FingerprintMismatch);
    }

    #[tokio::test]
    async fn resolves_unique_view_profiles_from_published_content() {
        let harness = sample_harness();
        insert_content(
            &harness.content_repository,
            sample_view_profile_content("view-profile-1", "default", "software_delivery"),
            "seed-view-profile",
        )
        .await;

        let response = harness
            .service
            .resolve_view_profile(
                ResolveViewProfileQuery {
                    role_ref: sample_role_ref("role-1"),
                    object_kind: ViewObjectKind::WorkItem,
                    scope: json!({
                        "project": {
                            "type": "software_delivery"
                        }
                    }),
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect("view profile should resolve");

        assert_eq!(response.schema_version, "1.0");
        assert_eq!(response.consistency.source, ReadSource::WriteModel);
        assert_eq!(
            response
                .view_profile
                .as_ref()
                .map(|profile| profile.view_profile_ref.content_id.as_str()),
            Some("view-profile-1")
        );
    }

    #[tokio::test]
    async fn rejects_ambiguous_view_profile_matches() {
        let harness = sample_harness();
        insert_content(
            &harness.content_repository,
            sample_view_profile_content("view-profile-1", "default", "software_delivery"),
            "seed-view-profile-1",
        )
        .await;
        insert_content(
            &harness.content_repository,
            sample_view_profile_content("view-profile-2", "secondary", "software_delivery"),
            "seed-view-profile-2",
        )
        .await;

        let error = harness
            .service
            .resolve_view_profile(
                ResolveViewProfileQuery {
                    role_ref: sample_role_ref("role-1"),
                    object_kind: ViewObjectKind::WorkItem,
                    scope: json!({
                        "project": {
                            "type": "software_delivery"
                        }
                    }),
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect_err("ambiguous profiles should fail");

        assert_eq!(error.code, MethodLibraryErrorCode::ViewProfileAmbiguous);
    }

    #[tokio::test]
    async fn gets_definition_traces_with_projection_consistency() {
        let harness = sample_harness();
        insert_content(
            &harness.content_repository,
            sample_published_content("content-trace"),
            "seed-content-trace",
        )
        .await;
        harness
            .trace_repository
            .upsert(sample_trace_view("content-trace"))
            .await
            .expect("trace projection should upsert");
        harness
            .checkpoint_repository
            .advance_if_current(
                "definition_trace_projection".to_string(),
                None,
                "evt-trace".to_string(),
                datetime!(2026-05-26 08:01:00 UTC),
            )
            .await
            .expect("trace checkpoint should advance");

        let response = harness
            .service
            .get_definition_trace(
                GetDefinitionTraceQuery {
                    content_id: "content-trace".to_string(),
                    include_audit: true,
                    include_events: true,
                    cursor: None,
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect("trace query should succeed");

        assert_eq!(response.schema_version, "1.0");
        assert_eq!(response.content_id, "content-trace");
        assert_eq!(response.trace.audit_records.len(), 1);
        assert_eq!(response.trace.outbox_events.len(), 1);
        assert_eq!(response.consistency.source, ReadSource::Projection);
        assert!(response.consistency.stale);
        assert_eq!(
            response.consistency.checkpoint.as_deref(),
            Some("evt-trace")
        );
    }

    #[tokio::test]
    async fn returns_projection_not_ready_when_trace_projection_is_missing() {
        let harness = sample_harness();
        insert_content(
            &harness.content_repository,
            sample_published_content("content-trace"),
            "seed-content-trace",
        )
        .await;

        let error = harness
            .service
            .get_definition_trace(
                GetDefinitionTraceQuery {
                    content_id: "content-trace".to_string(),
                    include_audit: true,
                    include_events: true,
                    cursor: None,
                },
                sample_actor(),
                sample_meta(),
            )
            .await
            .expect_err("missing trace projection should fail");

        assert_eq!(error.code, MethodLibraryErrorCode::ProjectionNotReady);
    }
}
