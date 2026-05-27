//! Worker wrapper for relaying pending outbox events.

use std::sync::Arc;

use method_library_application::ports::{
    BusPublisherPort, Clock, ObservabilityPort, OutboxRepository,
};
use method_library_application::{OutboxRelayPolicy, OutboxRelayService, OutboxRelayTopics};
use method_library_contracts::{
    RelayOutboxEventsJobRequest, RelayOutboxEventsJobResult, RequestMeta,
};
use method_library_domain::MethodLibraryError;

/// Runtime settings used by the outbox relay worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxRelaySettings {
    /// Maximum number of outbox events claimed per run.
    pub batch_size: u32,
    /// Lease duration in seconds for claimed events.
    pub lease_seconds: u64,
    /// Maximum total publish attempts before dead-lettering.
    pub max_attempts: u32,
    /// Retry backoff in milliseconds for retryable publish failures.
    pub retry_backoff_ms: u64,
    /// Topic used for definition publication and fingerprint events.
    pub definition_events_topic: String,
    /// Topic used for lifecycle events.
    pub lifecycle_events_topic: String,
}

/// Worker entrypoint that executes one outbox relay batch.
#[derive(Clone)]
pub struct OutboxRelayWorker {
    relay_service: OutboxRelayService,
    settings: OutboxRelaySettings,
}

impl OutboxRelayWorker {
    /// Creates an outbox relay worker from ports and runtime settings.
    #[must_use]
    pub fn new(
        outbox_repository: Arc<dyn OutboxRepository>,
        bus_publisher: Arc<dyn BusPublisherPort>,
        observability_port: Arc<dyn ObservabilityPort>,
        clock: Arc<dyn Clock>,
        settings: OutboxRelaySettings,
    ) -> Self {
        let relay_service = OutboxRelayService::new(
            outbox_repository,
            bus_publisher,
            observability_port,
            clock,
            OutboxRelayTopics {
                definition_events: settings.definition_events_topic.clone(),
                lifecycle_events: settings.lifecycle_events_topic.clone(),
            },
            OutboxRelayPolicy {
                max_attempts: settings.max_attempts,
                retry_backoff_ms: settings.retry_backoff_ms,
            },
        );

        Self {
            relay_service,
            settings,
        }
    }

    /// Runs one relay batch for the provided worker identifier.
    pub async fn run_once(
        &self,
        worker_id: String,
        meta: RequestMeta,
    ) -> Result<RelayOutboxEventsJobResult, MethodLibraryError> {
        self.relay_service
            .relay_pending_events(
                RelayOutboxEventsJobRequest {
                    worker_id,
                    batch_size: self.settings.batch_size,
                    lease_seconds: self.settings.lease_seconds,
                },
                meta,
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use method_library_application::ports::fakes::{
        DeterministicClock, FakeUnitOfWork, InMemoryOutboxRepository, RecordingBusPublisher,
        RecordingObservabilityPort,
    };
    use method_library_application::ports::{OutboxEvent, OutboxRepository, UnitOfWork};
    use method_library_contracts::{
        ContentPublishedPayload, DefinitionEventEnvelope, DefinitionEventPayload,
        DefinitionEventType, EventTraceContext, RequestMeta,
    };
    use method_library_domain::content::{
        ApprovedGateRef, CanonicalFingerprint, ContentVersion, FingerprintAlgorithm,
        MethodContentKind, PublishedContentRef,
    };
    use time::macros::datetime;

    use super::{OutboxRelaySettings, OutboxRelayWorker};

    #[tokio::test]
    async fn runs_one_relay_batch_from_worker_settings() {
        let unit_of_work = Arc::new(FakeUnitOfWork);
        let outbox_repository = Arc::new(InMemoryOutboxRepository::default());
        let bus_publisher = Arc::new(RecordingBusPublisher::default());
        let observability = Arc::new(RecordingObservabilityPort::default());
        let worker = OutboxRelayWorker::new(
            outbox_repository.clone(),
            bus_publisher.clone(),
            observability,
            Arc::new(DeterministicClock::new(datetime!(2026-05-27 02:00:00 UTC))),
            OutboxRelaySettings {
                batch_size: 5,
                lease_seconds: 90,
                max_attempts: 3,
                retry_backoff_ms: 30_000,
                definition_events_topic: "method-library.definition.events".to_string(),
                lifecycle_events_topic: "method-library.lifecycle.events".to_string(),
            },
        );

        let mut tx = unit_of_work
            .begin(sample_meta())
            .await
            .expect("transaction should begin");
        outbox_repository
            .append(&mut tx, sample_event("evt-worker"))
            .await
            .expect("outbox event should append");
        tx.commit().await.expect("transaction should commit");

        let result = worker
            .run_once("worker-a".to_string(), sample_meta())
            .await
            .expect("worker batch should succeed");

        assert_eq!(result.worker_id, "worker-a");
        assert_eq!(result.published_count, 1);
        assert_eq!(result.retryable_failure_count, 0);
        assert_eq!(result.dead_letter_count, 0);

        let published = bus_publisher
            .published_events()
            .expect("published events should be readable");
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0, "method-library.definition.events");
    }

    fn sample_meta() -> RequestMeta {
        RequestMeta {
            request_id: "req-worker".to_string(),
            trace_id: "trace-worker".to_string(),
            idempotency_key: None,
            request_hash: "worker-hash".to_string(),
            received_at: datetime!(2026-05-27 02:00:00 UTC),
        }
    }

    fn sample_event(event_id: &str) -> OutboxEvent {
        OutboxEvent::new_pending(
            event_id.to_string(),
            "content-worker".to_string(),
            DefinitionEventEnvelope {
                event_id: event_id.to_string(),
                event_type: DefinitionEventType::ContentPublished,
                schema_version: "1.0".to_string(),
                occurred_at: datetime!(2026-05-27 01:55:00 UTC),
                producer: "L3-method-library".to_string(),
                content_ref: PublishedContentRef {
                    content_id: "content-worker".to_string(),
                    kind: MethodContentKind::Qualification,
                    version: ContentVersion::new("1.0.0").expect("version should be valid"),
                    fingerprint: CanonicalFingerprint::new(
                        FingerprintAlgorithm::Sha256,
                        "worker",
                        "1.0",
                    )
                    .expect("fingerprint should be valid"),
                },
                snapshot_ref: None,
                trace: EventTraceContext {
                    request_id: "req-source".to_string(),
                    trace_id: "trace-source".to_string(),
                },
                payload: DefinitionEventPayload::ContentPublished(ContentPublishedPayload {
                    gate_ref: ApprovedGateRef {
                        gate_id: "gate-1".to_string(),
                        gate_decision_id: "decision-1".to_string(),
                        approved_at: datetime!(2026-05-27 01:50:00 UTC),
                    },
                    version: ContentVersion::new("1.0.0").expect("version should be valid"),
                    fingerprint: CanonicalFingerprint::new(
                        FingerprintAlgorithm::Sha256,
                        "worker",
                        "1.0",
                    )
                    .expect("fingerprint should be valid"),
                }),
            },
            "payload-hash-worker".to_string(),
            Some("idem-worker".to_string()),
        )
        .expect("worker outbox fixture should be valid")
    }
}
