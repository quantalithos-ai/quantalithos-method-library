use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use method_library_application::ports::fakes::{
    DeterministicClock, DeterministicFingerprintHasher, DeterministicIdGenerator,
    InMemoryObjectStorage, RecordingBusPublisher, RecordingObservabilityPort, StaticGovernancePort,
};
use method_library_application::{
    MethodContentCommandService, MethodContentQueryService, OutboxRelayPolicy, OutboxRelayService,
    OutboxRelayTopics,
};
use method_library_contracts::{
    ActorContext, ActorRef, CreateMethodContentDraftCommand, GetMethodContentQuery,
    PublishMethodContentCommand, ReadMode, RelayOutboxEventsJobRequest, RequestMeta,
    ResolveViewProfileQuery, SubmitMethodContentForReviewCommand,
};
use method_library_domain::content::{
    ActorKind, ApprovedGateRef, ContentVersion, MethodContentKind, PublishedContentRef,
};
use method_library_domain::definitions::{
    ActionAvailability, EvidenceKind, EvidenceRule, FieldVisibility, MethodContentPayload,
    Qualification, QualificationLevel, QualificationLevelModel, RoleDefinition, ViewActionRule,
    ViewCondition, ViewConditionOperator, ViewFieldRule, ViewObjectKind, ViewProfile,
    ViewScopeRule,
};
use method_library_infra::persistence::postgres::{PostgresPersistence, PostgresTestDatabase};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use time::macros::datetime;

const METHOD_CONTENTS_TABLE: &str = "method_contents";
const METHOD_CONTENT_REFERENCES_TABLE: &str = "method_content_references";
const METHOD_CONTENT_VERSIONS_TABLE: &str = "method_content_versions";
const SUPERSEDE_LINKS_TABLE: &str = "supersede_links";
const LIFECYCLE_HISTORY_TABLE: &str = "lifecycle_history_entries";
const AUDIT_RECORDS_TABLE: &str = "audit_records";
const OUTBOX_TABLE: &str = "outbox_events";
const IDEMPOTENCY_TABLE: &str = "idempotency_records";
const SNAPSHOT_TABLE: &str = "definition_snapshots";
const SUMMARY_TABLE: &str = "content_summary_projection";
const TRACE_TABLE: &str = "definition_trace_projection";
const CHECKPOINT_TABLE: &str = "projection_checkpoints";
const DEAD_LETTER_TABLE: &str = "inbound_dead_letters";
const JOB_RUNS_TABLE: &str = "job_runs";

const WARMUP_SAMPLES: usize = 5;
const MEASURED_SAMPLES: usize = 40;

static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct BenchmarkHarness {
    command_service: MethodContentCommandService,
    query_service: MethodContentQueryService,
    relay_service: OutboxRelayService,
    bus: Arc<RecordingBusPublisher>,
}

#[derive(Debug, Clone)]
struct PublishedFixture {
    content_id: String,
    published_ref: PublishedContentRef,
}

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[tokio::test]
async fn get_method_content_pg_p95_is_below_threshold() {
    let _guard = test_guard();
    let harness = benchmark_harness().await;
    let fixture = harness.publish_qualification("nf-get").await;

    for attempt in 0..WARMUP_SAMPLES {
        let _ = harness
            .query_service
            .get_method_content(
                GetMethodContentQuery {
                    content_id: fixture.content_id.clone(),
                    read_mode: ReadMode::Published,
                    include_payload: true,
                    include_references: true,
                },
                sample_actor(),
                read_meta(&format!("get-warmup-{attempt}")),
            )
            .await
            .expect("warmup get should succeed");
    }

    let mut samples = Vec::with_capacity(MEASURED_SAMPLES);
    for attempt in 0..MEASURED_SAMPLES {
        let started_at = Instant::now();
        let response = harness
            .query_service
            .get_method_content(
                GetMethodContentQuery {
                    content_id: fixture.content_id.clone(),
                    read_mode: ReadMode::Published,
                    include_payload: true,
                    include_references: true,
                },
                sample_actor(),
                read_meta(&format!("get-measure-{attempt}")),
            )
            .await
            .expect("measured get should succeed");
        samples.push(started_at.elapsed());
        assert_eq!(response.content.content_id, fixture.content_id);
    }

    let p95_ms = percentile_ms(&samples, 95);
    println!("BENCHMARK GetMethodContent PG P95: {p95_ms:.3} ms");
    assert!(p95_ms < 200.0, "GetMethodContent PG P95 exceeded threshold");
}

#[tokio::test]
async fn resolve_view_profile_pg_p95_is_below_threshold() {
    let _guard = test_guard();
    let harness = benchmark_harness().await;
    let qualification = harness.publish_qualification("nf-resolve").await;
    let role = harness
        .publish_role("nf-resolve", qualification.published_ref.clone())
        .await;
    let _view_profile = harness
        .publish_view_profile("nf-resolve", role.published_ref.clone())
        .await;

    for attempt in 0..WARMUP_SAMPLES {
        let _ = harness
            .query_service
            .resolve_view_profile(
                ResolveViewProfileQuery {
                    role_ref: role.published_ref.clone(),
                    object_kind: ViewObjectKind::WorkItem,
                    scope: json!({ "project": { "type": "service" } }),
                },
                sample_actor(),
                read_meta(&format!("resolve-warmup-{attempt}")),
            )
            .await
            .expect("warmup resolve should succeed");
    }

    let mut samples = Vec::with_capacity(MEASURED_SAMPLES);
    for attempt in 0..MEASURED_SAMPLES {
        let started_at = Instant::now();
        let response = harness
            .query_service
            .resolve_view_profile(
                ResolveViewProfileQuery {
                    role_ref: role.published_ref.clone(),
                    object_kind: ViewObjectKind::WorkItem,
                    scope: json!({ "project": { "type": "service" } }),
                },
                sample_actor(),
                read_meta(&format!("resolve-measure-{attempt}")),
            )
            .await
            .expect("measured resolve should succeed");
        samples.push(started_at.elapsed());
        assert!(response.view_profile.is_some());
    }

    let p95_ms = percentile_ms(&samples, 95);
    println!("BENCHMARK ResolveViewProfile P95: {p95_ms:.3} ms");
    assert!(p95_ms < 30.0, "ResolveViewProfile P95 exceeded threshold");
}

#[tokio::test]
async fn publish_method_content_pg_p95_is_below_threshold() {
    let _guard = test_guard();
    let harness = benchmark_harness().await;

    for attempt in 0..WARMUP_SAMPLES {
        let content_id = harness
            .create_in_review_qualification(&format!("publish-warmup-{attempt}"))
            .await;
        let _ = harness
            .command_service
            .publish(
                publish_command(content_id, 2, &format!("publish-warmup-{attempt}")),
                sample_actor(),
                command_meta("publish", &format!("warmup-{attempt}")),
            )
            .await
            .expect("warmup publish should succeed");
    }

    let mut samples = Vec::with_capacity(MEASURED_SAMPLES);
    for attempt in 0..MEASURED_SAMPLES {
        let content_id = harness
            .create_in_review_qualification(&format!("publish-measure-{attempt}"))
            .await;
        let started_at = Instant::now();
        let response = harness
            .command_service
            .publish(
                publish_command(content_id, 2, &format!("publish-measure-{attempt}")),
                sample_actor(),
                command_meta("publish", &format!("measure-{attempt}")),
            )
            .await
            .expect("measured publish should succeed");
        samples.push(started_at.elapsed());
        assert_eq!(response.lifecycle_state.as_str(), "published");
    }

    let p95_ms = percentile_ms(&samples, 95);
    println!("BENCHMARK PublishMethodContent P95: {p95_ms:.3} ms");
    assert!(
        p95_ms < 500.0,
        "PublishMethodContent P95 exceeded threshold"
    );
}

#[tokio::test]
async fn event_propagation_samples_succeed_end_to_end() {
    let _guard = test_guard();
    let harness = benchmark_harness().await;

    let mut samples = Vec::with_capacity(MEASURED_SAMPLES);
    for attempt in 0..MEASURED_SAMPLES {
        let content_id = harness
            .create_in_review_qualification(&format!("propagation-{attempt}"))
            .await;
        let published_before = harness
            .bus
            .published_events()
            .expect("published events should be readable")
            .len();

        let started_at = Instant::now();
        let publish_response = harness
            .command_service
            .publish(
                publish_command(content_id, 2, &format!("propagation-{attempt}")),
                sample_actor(),
                command_meta("publish", &format!("propagation-{attempt}")),
            )
            .await
            .expect("propagation publish should succeed");
        let relay_result = harness
            .relay_service
            .relay_pending_events(
                RelayOutboxEventsJobRequest {
                    worker_id: format!("worker-{attempt}"),
                    batch_size: 1,
                    lease_seconds: 60,
                },
                relay_meta(attempt),
            )
            .await
            .expect("relay should succeed");
        samples.push(started_at.elapsed());

        assert_eq!(relay_result.published_count, 1);
        assert_eq!(relay_result.retryable_failure_count, 0);
        assert_eq!(relay_result.dead_letter_count, 0);
        assert_eq!(
            harness
                .bus
                .published_events()
                .expect("published events should be readable")
                .len(),
            published_before + 1
        );
        assert!(!publish_response.outbox_event_id.is_empty());
    }

    let p95_ms = percentile_ms(&samples, 95);
    println!(
        "BENCHMARK Event propagation success rate: 100.0%, observed end-to-end P95: {p95_ms:.3} ms"
    );
}

impl BenchmarkHarness {
    async fn publish_qualification(&self, suffix: &str) -> PublishedFixture {
        self.publish_definition(
            suffix,
            MethodContentKind::Qualification,
            format!("Qualification {suffix}"),
            MethodContentPayload::Qualification(Qualification {
                qualification_key: format!("qualification_{suffix}"),
                name: format!("Qualification {suffix}"),
                description: Some(format!("Qualification definition for {suffix}")),
                level_model: QualificationLevelModel {
                    levels: vec![QualificationLevel {
                        level_key: "baseline".to_string(),
                        name: "Baseline".to_string(),
                        order: 1,
                        description: None,
                    }],
                    default_level_key: Some("baseline".to_string()),
                },
                evidence_rules: vec![EvidenceRule {
                    evidence_kind: EvidenceKind::Document,
                    required: true,
                    description: "Provide evidence".to_string(),
                }],
            }),
        )
        .await
    }

    async fn publish_role(
        &self,
        suffix: &str,
        qualification_ref: PublishedContentRef,
    ) -> PublishedFixture {
        self.publish_definition(
            suffix,
            MethodContentKind::RoleDefinition,
            format!("Role {suffix}"),
            MethodContentPayload::RoleDefinition(RoleDefinition {
                role_key: format!("role_{suffix}"),
                responsibilities: vec!["Coordinate delivery".to_string()],
                qualification_refs: vec![qualification_ref],
                default_view_profile_refs: Vec::new(),
            }),
        )
        .await
    }

    async fn publish_view_profile(
        &self,
        suffix: &str,
        role_ref: PublishedContentRef,
    ) -> PublishedFixture {
        self.publish_definition(
            suffix,
            MethodContentKind::ViewProfile,
            format!("View Profile {suffix}"),
            MethodContentPayload::ViewProfile(ViewProfile {
                role_ref,
                object_kind: ViewObjectKind::WorkItem,
                scope_rules: vec![ViewScopeRule {
                    scope_key: format!("scope_{suffix}"),
                    conditions: vec![ViewCondition {
                        field_path: "project.type".to_string(),
                        operator: ViewConditionOperator::Eq,
                        value_json: Some(json!("service")),
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
        )
        .await
    }

    async fn create_in_review_qualification(&self, suffix: &str) -> String {
        let create_response = self
            .command_service
            .create_draft(
                CreateMethodContentDraftCommand {
                    kind: MethodContentKind::Qualification,
                    name: format!("Qualification {suffix}"),
                    description: Some(format!("Qualification definition for {suffix}")),
                    payload: MethodContentPayload::Qualification(Qualification {
                        qualification_key: format!("qualification_{suffix}"),
                        name: format!("Qualification {suffix}"),
                        description: Some(format!("Qualification definition for {suffix}")),
                        level_model: QualificationLevelModel {
                            levels: vec![QualificationLevel {
                                level_key: "baseline".to_string(),
                                name: "Baseline".to_string(),
                                order: 1,
                                description: None,
                            }],
                            default_level_key: Some("baseline".to_string()),
                        },
                        evidence_rules: vec![EvidenceRule {
                            evidence_kind: EvidenceKind::Document,
                            required: true,
                            description: "Provide evidence".to_string(),
                        }],
                    }),
                    references: Vec::new(),
                    source_refs: Vec::new(),
                },
                sample_actor(),
                command_meta("create", &format!("{suffix}-qualification")),
            )
            .await
            .expect("draft creation should succeed");

        self.command_service
            .submit_for_review(
                SubmitMethodContentForReviewCommand {
                    content_id: create_response.content_id.clone(),
                    expected_revision: create_response.revision,
                    review_reason: Some(format!("Submit {suffix} for review")),
                    review_evidence_refs: Vec::new(),
                },
                sample_actor(),
                command_meta("submit", &format!("{suffix}-qualification")),
            )
            .await
            .expect("submit for review should succeed");

        create_response.content_id
    }

    async fn publish_definition(
        &self,
        suffix: &str,
        kind: MethodContentKind,
        name: String,
        payload: MethodContentPayload,
    ) -> PublishedFixture {
        let meta_suffix = format!("{suffix}-{}", kind.as_str());
        let create_response = self
            .command_service
            .create_draft(
                CreateMethodContentDraftCommand {
                    kind,
                    name: name.clone(),
                    description: Some(format!("Definition {suffix}")),
                    payload,
                    references: Vec::new(),
                    source_refs: Vec::new(),
                },
                sample_actor(),
                command_meta("create", &meta_suffix),
            )
            .await
            .expect("draft creation should succeed");
        let submit_response = self
            .command_service
            .submit_for_review(
                SubmitMethodContentForReviewCommand {
                    content_id: create_response.content_id.clone(),
                    expected_revision: create_response.revision,
                    review_reason: Some(format!("Submit {suffix} for review")),
                    review_evidence_refs: Vec::new(),
                },
                sample_actor(),
                command_meta("submit", &meta_suffix),
            )
            .await
            .expect("submit for review should succeed");
        let publish_response = self
            .command_service
            .publish(
                publish_command(
                    submit_response.content_id.clone(),
                    submit_response.revision,
                    &meta_suffix,
                ),
                sample_actor(),
                command_meta("publish", &meta_suffix),
            )
            .await
            .expect("publish should succeed");

        let stored = self
            .query_service
            .get_method_content(
                GetMethodContentQuery {
                    content_id: publish_response.content_id.clone(),
                    read_mode: ReadMode::Published,
                    include_payload: false,
                    include_references: false,
                },
                sample_actor(),
                read_meta("published-ref"),
            )
            .await
            .expect("published content should be queryable");

        PublishedFixture {
            content_id: publish_response.content_id.clone(),
            published_ref: PublishedContentRef {
                content_id: publish_response.content_id,
                kind: publish_response.kind,
                version: stored
                    .content
                    .version
                    .expect("published content should retain a version"),
                fingerprint: stored
                    .content
                    .fingerprint
                    .expect("published content should retain a fingerprint"),
            },
        }
    }
}

async fn benchmark_harness() -> BenchmarkHarness {
    let persistence = reset_database().await;
    let object_storage = Arc::new(InMemoryObjectStorage::default());
    let bus = Arc::new(RecordingBusPublisher::default());
    let clock = Arc::new(DeterministicClock::new(datetime!(2026-05-27 16:00:00 UTC)));
    let command_service = MethodContentCommandService::new(
        Arc::new(persistence.unit_of_work()),
        Arc::new(persistence.method_content_repository()),
        Arc::new(persistence.method_content_reference_repository()),
        Arc::new(persistence.method_content_version_repository()),
        Arc::new(persistence.snapshot_repository()),
        Arc::new(persistence.supersede_link_repository()),
        Arc::new(persistence.outbox_repository()),
        Arc::new(persistence.lifecycle_history_repository()),
        Arc::new(persistence.audit_repository()),
        Arc::new(persistence.idempotency_repository()),
        Arc::new(StaticGovernancePort::new(
            true,
            datetime!(2026-05-27 16:00:00 UTC),
        )),
        object_storage.clone(),
        Arc::new(DeterministicFingerprintHasher::default()),
        clock.clone(),
        Arc::new(DeterministicIdGenerator::default()),
    );
    let query_service = MethodContentQueryService::new(
        Arc::new(persistence.method_content_repository()),
        Arc::new(persistence.method_content_reference_repository()),
        Arc::new(persistence.method_content_version_repository()),
        Arc::new(persistence.snapshot_repository()),
        object_storage,
        Arc::new(persistence.content_summary_projection_repository()),
        Arc::new(persistence.definition_trace_projection_repository()),
        Arc::new(persistence.projection_checkpoint_repository()),
        clock.clone(),
    );
    let relay_service = OutboxRelayService::new(
        Arc::new(persistence.outbox_repository()),
        bus.clone(),
        Arc::new(RecordingObservabilityPort::default()),
        clock,
        OutboxRelayTopics {
            definition_events: "method-library.definition.events".to_string(),
            lifecycle_events: "method-library.lifecycle.events".to_string(),
        },
        OutboxRelayPolicy {
            max_attempts: 3,
            retry_backoff_ms: 30_000,
        },
    );

    BenchmarkHarness {
        command_service,
        query_service,
        relay_service,
        bus,
    }
}

async fn reset_database() -> PostgresPersistence {
    PostgresTestDatabase::ensure_database()
        .await
        .expect("test database should exist");
    let database_url = PostgresTestDatabase::database_url();
    let persistence = PostgresPersistence::connect_and_migrate(&database_url)
        .await
        .expect("postgres persistence should connect");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("benchmark database should connect");
    sqlx::query(&format!(
        "truncate table {METHOD_CONTENTS_TABLE}, {METHOD_CONTENT_REFERENCES_TABLE}, {METHOD_CONTENT_VERSIONS_TABLE}, {SUPERSEDE_LINKS_TABLE}, {LIFECYCLE_HISTORY_TABLE}, {AUDIT_RECORDS_TABLE}, {OUTBOX_TABLE}, {IDEMPOTENCY_TABLE}, {SNAPSHOT_TABLE}, {SUMMARY_TABLE}, {TRACE_TABLE}, {CHECKPOINT_TABLE}, {DEAD_LETTER_TABLE}, {JOB_RUNS_TABLE} restart identity cascade"
    ))
    .execute(&pool)
    .await
    .expect("benchmark tables should truncate");
    persistence
}

fn publish_command(
    content_id: String,
    expected_revision: i64,
    suffix: &str,
) -> PublishMethodContentCommand {
    PublishMethodContentCommand {
        content_id,
        expected_revision,
        version: ContentVersion::new("1.0.0").expect("benchmark version should be valid"),
        approved_gate_ref: ApprovedGateRef {
            gate_id: format!("gate-{suffix}"),
            gate_decision_id: format!("decision-{suffix}"),
            approved_at: datetime!(2026-05-27 16:05:00 UTC),
        },
        publish_reason: format!("Benchmark publish {suffix}"),
    }
}

fn percentile_ms(samples: &[Duration], percentile: u32) -> f64 {
    let mut ordered = samples
        .iter()
        .map(Duration::as_secs_f64)
        .collect::<Vec<_>>();
    ordered.sort_by(f64::total_cmp);
    let rank = ((ordered.len() * percentile as usize).div_ceil(100)).saturating_sub(1);
    ordered.get(rank).copied().unwrap_or_default() * 1_000.0
}

fn sample_actor() -> ActorContext {
    ActorContext {
        actor_id: "actor-benchmark".to_string(),
        actor_kind: ActorKind::Human,
        actor_ref: ActorRef {
            actor_id: "actor-benchmark".to_string(),
            actor_kind: ActorKind::Human,
        },
    }
}

fn command_meta(operation: &str, suffix: &str) -> RequestMeta {
    RequestMeta {
        request_id: format!("req-{operation}-{suffix}"),
        trace_id: "trace-benchmark".to_string(),
        idempotency_key: Some(format!("idem-{operation}-{suffix}")),
        request_hash: format!("hash-{operation}-{suffix}"),
        received_at: datetime!(2026-05-27 16:00:00 UTC),
    }
}

fn read_meta(suffix: &str) -> RequestMeta {
    RequestMeta {
        request_id: format!("req-read-{suffix}"),
        trace_id: "trace-benchmark".to_string(),
        idempotency_key: None,
        request_hash: format!("hash-read-{suffix}"),
        received_at: datetime!(2026-05-27 16:00:00 UTC),
    }
}

fn relay_meta(attempt: usize) -> RequestMeta {
    RequestMeta {
        request_id: format!("req-relay-{attempt}"),
        trace_id: "trace-benchmark".to_string(),
        idempotency_key: None,
        request_hash: format!("hash-relay-{attempt}"),
        received_at: datetime!(2026-05-27 16:00:00 UTC),
    }
}
