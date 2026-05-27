//! Read-only query services for P0 method-library query flows.

use std::sync::Arc;

use method_library_contracts::{
    ActorContext, ContentRefView, ContentVersionView, GetMethodContentQuery,
    GetMethodContentResponse, GetMethodContentVersionQuery, GetMethodContentVersionResponse,
    ListMethodContentsQuery, ListMethodContentsResponse, MethodContentView, PageInfo,
    ReadConsistency, ReadMode, ReadSource, RequestMeta,
};
use method_library_domain::content::{
    ContentRef, ContentVersion, MethodContent, PublishedContentRef,
};
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};
use time::Duration;

use crate::ports::{
    CheckpointStatus, Clock, ContentSummaryProjectionRepository, DefinitionSnapshotRepository,
    MethodContentReferenceRepository, MethodContentRepository, MethodContentVersionRepository,
    PageRequest, ProjectionCheckpointRepository,
};

const API_SCHEMA_VERSION: &str = "1.0";
const CONTENT_SUMMARY_PROJECTION_CHECKPOINT: &str = "content_summary_projection";
const DEFAULT_PAGE_SIZE: u32 = 50;
const MAX_PAGE_SIZE: u32 = 100;

/// Application service for read-only method-content queries.
#[derive(Clone)]
pub struct MethodContentQueryService {
    method_content_repository: Arc<dyn MethodContentRepository>,
    method_content_reference_repository: Arc<dyn MethodContentReferenceRepository>,
    method_content_version_repository: Arc<dyn MethodContentVersionRepository>,
    definition_snapshot_repository: Arc<dyn DefinitionSnapshotRepository>,
    content_summary_projection_repository: Arc<dyn ContentSummaryProjectionRepository>,
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
        content_summary_projection_repository: Arc<dyn ContentSummaryProjectionRepository>,
        projection_checkpoint_repository: Arc<dyn ProjectionCheckpointRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            method_content_repository,
            method_content_reference_repository,
            method_content_version_repository,
            definition_snapshot_repository,
            content_summary_projection_repository,
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

    async fn summary_projection_consistency(&self) -> Result<ReadConsistency, MethodLibraryError> {
        let checkpoint = self
            .projection_checkpoint_repository
            .get(CONTENT_SUMMARY_PROJECTION_CHECKPOINT.to_string())
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use time::macros::datetime;

    use super::MethodContentQueryService;
    use crate::ports::fakes::{
        DeterministicClock, FakeUnitOfWork, InMemoryContentSummaryProjectionRepository,
        InMemoryDefinitionSnapshotRepository, InMemoryMethodContentReferenceRepository,
        InMemoryMethodContentRepository, InMemoryMethodContentVersionRepository,
        InMemoryProjectionCheckpointRepository,
    };
    use crate::ports::{
        ContentSummaryProjectionRepository, DefinitionSnapshotRepository,
        MethodContentReferenceRepository, MethodContentRepository, MethodContentVersionRecord,
        MethodContentVersionRepository, ProjectionCheckpointRepository, UnitOfWork,
    };
    use method_library_contracts::{
        ActorContext, ActorRef, ContentSummaryView, DefinitionSnapshot, GetMethodContentQuery,
        GetMethodContentVersionQuery, ListMethodContentsQuery, ReadMode, ReadSource, RequestMeta,
    };
    use method_library_domain::MethodLibraryErrorCode;
    use method_library_domain::content::{
        ActorKind, ApprovedGateRef, CanonicalFingerprint, ContentRef, ContentVersion,
        FingerprintAlgorithm, LifecycleState, MethodContent, MethodContentKind,
        PublishedContentRef, ReferenceState,
    };
    use method_library_domain::definitions::{
        EvidenceKind, EvidenceRule, MethodContentPayload, Qualification, QualificationLevel,
        QualificationLevelModel,
    };

    struct TestHarness {
        service: MethodContentQueryService,
        content_repository: Arc<InMemoryMethodContentRepository>,
        reference_repository: Arc<InMemoryMethodContentReferenceRepository>,
        version_repository: Arc<InMemoryMethodContentVersionRepository>,
        snapshot_repository: Arc<InMemoryDefinitionSnapshotRepository>,
        summary_repository: Arc<InMemoryContentSummaryProjectionRepository>,
        checkpoint_repository: Arc<InMemoryProjectionCheckpointRepository>,
    }

    fn sample_harness() -> TestHarness {
        let content_repository = Arc::new(InMemoryMethodContentRepository::default());
        let reference_repository = Arc::new(InMemoryMethodContentReferenceRepository::default());
        let version_repository = Arc::new(InMemoryMethodContentVersionRepository::default());
        let snapshot_repository = Arc::new(InMemoryDefinitionSnapshotRepository::default());
        let summary_repository = Arc::new(InMemoryContentSummaryProjectionRepository::default());
        let checkpoint_repository = Arc::new(InMemoryProjectionCheckpointRepository::default());
        let clock = Arc::new(DeterministicClock::new(datetime!(2026-05-26 08:10:00 UTC)));

        let service = MethodContentQueryService::new(
            content_repository.clone(),
            reference_repository.clone(),
            version_repository.clone(),
            snapshot_repository.clone(),
            summary_repository.clone(),
            checkpoint_repository.clone(),
            clock,
        );

        TestHarness {
            service,
            content_repository,
            reference_repository,
            version_repository,
            snapshot_repository,
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
}
