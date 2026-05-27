//! Application services for outbox relay and future definition sync flows.

use std::collections::BTreeMap;
use std::sync::Arc;

use method_library_contracts::{
    DefinitionEventType, RelayOutboxEventsJobRequest, RelayOutboxEventsJobResult, RequestMeta,
};
use method_library_domain::content::LeaseDuration;
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};

use crate::ports::{
    BusPublisherPort, Clock, FailureReason, ObservabilityEvent, ObservabilityPort,
    OutboxRepository, Topic,
};

const OUTBOX_RELAY_CLAIM_EVENT: &str = "outbox_relay_claim";
const OUTBOX_RELAY_RETRYABLE_FAILURE_EVENT: &str = "outbox_relay_retryable_failure";
const OUTBOX_RELAY_DEAD_LETTER_EVENT: &str = "outbox_relay_dead_letter";
const OUTBOX_RELAY_PUBLISHED_EVENT: &str = "outbox_relay_published";

/// Topics used by the outbox relay worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxRelayTopics {
    /// Topic for definition publication and fingerprint events.
    pub definition_events: Topic,
    /// Topic for lifecycle events.
    pub lifecycle_events: Topic,
}

/// Retry policy used by the outbox relay worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutboxRelayPolicy {
    /// Maximum total publish attempts before moving an event to dead letter.
    pub max_attempts: u32,
    /// Backoff delay in milliseconds for retryable failures.
    pub retry_backoff_ms: u64,
}

/// Application service that relays claimed outbox events to the event bus.
#[derive(Clone)]
pub struct OutboxRelayService {
    outbox_repository: Arc<dyn OutboxRepository>,
    bus_publisher: Arc<dyn BusPublisherPort>,
    observability_port: Arc<dyn ObservabilityPort>,
    clock: Arc<dyn Clock>,
    topics: OutboxRelayTopics,
    policy: OutboxRelayPolicy,
}

impl OutboxRelayService {
    /// Creates an outbox relay service from port implementations and worker policy.
    #[must_use]
    pub fn new(
        outbox_repository: Arc<dyn OutboxRepository>,
        bus_publisher: Arc<dyn BusPublisherPort>,
        observability_port: Arc<dyn ObservabilityPort>,
        clock: Arc<dyn Clock>,
        topics: OutboxRelayTopics,
        policy: OutboxRelayPolicy,
    ) -> Self {
        Self {
            outbox_repository,
            bus_publisher,
            observability_port,
            clock,
            topics,
            policy,
        }
    }

    /// Claims pending outbox events, publishes them, and advances outbox status.
    pub async fn relay_pending_events(
        &self,
        request: RelayOutboxEventsJobRequest,
        meta: RequestMeta,
    ) -> Result<RelayOutboxEventsJobResult, MethodLibraryError> {
        validate_relay_request(&request, &self.topics, self.policy)?;
        let now = self.clock.now();
        let lease = lease_from_request(&request)?;
        let retry_backoff = retry_backoff_from_policy(self.policy)?;
        let claimed = self
            .outbox_repository
            .claim_pending(request.batch_size, request.worker_id.clone(), now, lease)
            .await?;

        record_observability(
            &*self.observability_port,
            OUTBOX_RELAY_CLAIM_EVENT,
            &meta,
            BTreeMap::from([
                ("worker_id".to_string(), request.worker_id.clone()),
                ("batch_size".to_string(), request.batch_size.to_string()),
                ("claimed_count".to_string(), claimed.len().to_string()),
                (
                    "lease_timeout_ms".to_string(),
                    request.lease_seconds.saturating_mul(1000).to_string(),
                ),
            ]),
        );

        let mut published_count = 0_u32;
        let mut retryable_failure_count = 0_u32;
        let mut dead_letter_count = 0_u32;

        for event in claimed {
            let topic = self.topic_for_event_type(event.envelope.event_type);
            match self
                .bus_publisher
                .publish(topic.clone(), event.envelope.clone(), meta.clone())
                .await
            {
                Ok(_) => {
                    let published_at = self.clock.now();
                    self.outbox_repository
                        .mark_published(
                            event.event_id.clone(),
                            request.worker_id.clone(),
                            published_at,
                        )
                        .await?;
                    published_count += 1;
                    record_observability(
                        &*self.observability_port,
                        OUTBOX_RELAY_PUBLISHED_EVENT,
                        &meta,
                        BTreeMap::from([
                            ("event_id".to_string(), event.event_id),
                            (
                                "event_type".to_string(),
                                event.envelope.event_type.as_str().to_string(),
                            ),
                            ("topic".to_string(), topic),
                            ("result".to_string(), "published".to_string()),
                        ]),
                    );
                }
                Err(error) => {
                    let failure_reason = FailureReason::from_error(&error);
                    let attempt_count = event.retry_count.saturating_add(1);
                    if failure_reason.retryable && attempt_count < self.policy.max_attempts {
                        let next_retry_at = self.clock.now() + retry_backoff;
                        self.outbox_repository
                            .mark_retryable_failure(
                                event.event_id.clone(),
                                request.worker_id.clone(),
                                failure_reason.clone(),
                                next_retry_at,
                            )
                            .await?;
                        retryable_failure_count += 1;
                        record_observability(
                            &*self.observability_port,
                            OUTBOX_RELAY_RETRYABLE_FAILURE_EVENT,
                            &meta,
                            BTreeMap::from([
                                ("event_id".to_string(), event.event_id),
                                (
                                    "event_type".to_string(),
                                    event.envelope.event_type.as_str().to_string(),
                                ),
                                ("topic".to_string(), topic),
                                ("attempt_count".to_string(), attempt_count.to_string()),
                                ("error_code".to_string(), failure_reason.code.to_string()),
                                ("next_retry_at".to_string(), format_timestamp(next_retry_at)),
                            ]),
                        );
                    } else {
                        let dead_lettered_at = self.clock.now();
                        self.outbox_repository
                            .mark_dead_lettered(
                                event.event_id.clone(),
                                request.worker_id.clone(),
                                failure_reason.clone(),
                                dead_lettered_at,
                            )
                            .await?;
                        dead_letter_count += 1;
                        record_observability(
                            &*self.observability_port,
                            OUTBOX_RELAY_DEAD_LETTER_EVENT,
                            &meta,
                            BTreeMap::from([
                                ("event_id".to_string(), event.event_id),
                                (
                                    "event_type".to_string(),
                                    event.envelope.event_type.as_str().to_string(),
                                ),
                                ("topic".to_string(), topic),
                                ("attempt_count".to_string(), attempt_count.to_string()),
                                ("error_code".to_string(), failure_reason.code.to_string()),
                            ]),
                        );
                    }
                }
            }
        }

        Ok(RelayOutboxEventsJobResult {
            worker_id: request.worker_id,
            published_count,
            retryable_failure_count,
            dead_letter_count,
        })
    }

    fn topic_for_event_type(&self, event_type: DefinitionEventType) -> Topic {
        match event_type {
            DefinitionEventType::ContentPublished | DefinitionEventType::FingerprintChanged => {
                self.topics.definition_events.clone()
            }
            DefinitionEventType::ContentDeprecated | DefinitionEventType::ContentRetired => {
                self.topics.lifecycle_events.clone()
            }
        }
    }
}

fn validate_relay_request(
    request: &RelayOutboxEventsJobRequest,
    topics: &OutboxRelayTopics,
    policy: OutboxRelayPolicy,
) -> Result<(), MethodLibraryError> {
    if request.worker_id.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "relay requests require a worker identifier",
        ));
    }
    if request.batch_size == 0 {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "relay requests require a positive batch size",
        ));
    }
    if request.lease_seconds == 0 {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "relay requests require a positive lease duration",
        ));
    }
    if policy.max_attempts == 0 {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "relay policy requires at least one publish attempt",
        ));
    }
    if topics.definition_events.trim().is_empty() || topics.lifecycle_events.trim().is_empty() {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "relay topics must be configured before running the worker",
        ));
    }

    Ok(())
}

fn lease_from_request(
    request: &RelayOutboxEventsJobRequest,
) -> Result<LeaseDuration, MethodLibraryError> {
    let lease_seconds = i64::try_from(request.lease_seconds).map_err(|_| {
        MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "relay lease duration exceeds the supported range",
        )
    })?;
    Ok(time::Duration::seconds(lease_seconds))
}

fn retry_backoff_from_policy(
    policy: OutboxRelayPolicy,
) -> Result<time::Duration, MethodLibraryError> {
    let retry_backoff_ms = i64::try_from(policy.retry_backoff_ms).map_err(|_| {
        MethodLibraryError::validation(
            MethodLibraryErrorCode::JobRequestInvalid,
            "relay retry backoff exceeds the supported range",
        )
    })?;
    Ok(time::Duration::milliseconds(retry_backoff_ms))
}

fn format_timestamp(timestamp: time::OffsetDateTime) -> String {
    timestamp
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| timestamp.unix_timestamp().to_string())
}

fn record_observability(
    observability_port: &dyn ObservabilityPort,
    name: &str,
    meta: &RequestMeta,
    attributes: BTreeMap<String, String>,
) {
    let _ = observability_port.record_event(ObservabilityEvent {
        name: name.to_string(),
        trace_id: Some(meta.trace_id.clone()),
        attributes,
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use method_library_contracts::{
        ContentDeprecatedPayload, ContentPublishedPayload, ContentRetiredPayload,
        DefinitionEventEnvelope, DefinitionEventPayload, DefinitionEventType, EventTraceContext,
        FingerprintChangedPayload, RelayOutboxEventsJobRequest, RequestMeta,
    };
    use method_library_domain::content::{
        ApprovedGateRef, CanonicalFingerprint, ContentVersion, FingerprintAlgorithm,
        MethodContentKind, PublishedContentRef, Timestamp,
    };
    use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};
    use time::macros::datetime;

    use super::{OutboxRelayPolicy, OutboxRelayService, OutboxRelayTopics};
    use crate::ports::fakes::{
        DeterministicClock, FakeUnitOfWork, InMemoryOutboxRepository, RecordingBusPublisher,
        RecordingObservabilityPort,
    };
    use crate::ports::{OutboxEvent, OutboxRepository, OutboxStatus, UnitOfWork};

    struct RelayHarness {
        service: OutboxRelayService,
        unit_of_work: Arc<FakeUnitOfWork>,
        outbox_repository: Arc<InMemoryOutboxRepository>,
        bus_publisher: Arc<RecordingBusPublisher>,
        observability: Arc<RecordingObservabilityPort>,
    }

    impl RelayHarness {
        fn new(max_attempts: u32, retry_backoff_ms: u64) -> Self {
            let unit_of_work = Arc::new(FakeUnitOfWork);
            let outbox_repository = Arc::new(InMemoryOutboxRepository::default());
            let bus_publisher = Arc::new(RecordingBusPublisher::default());
            let observability = Arc::new(RecordingObservabilityPort::default());
            let service = OutboxRelayService::new(
                outbox_repository.clone(),
                bus_publisher.clone(),
                observability.clone(),
                Arc::new(DeterministicClock::new(datetime!(2026-05-27 01:00:00 UTC))),
                OutboxRelayTopics {
                    definition_events: "method-library.definition.events".to_string(),
                    lifecycle_events: "method-library.lifecycle.events".to_string(),
                },
                OutboxRelayPolicy {
                    max_attempts,
                    retry_backoff_ms,
                },
            );

            Self {
                service,
                unit_of_work,
                outbox_repository,
                bus_publisher,
                observability,
            }
        }

        async fn append_event(&self, event: OutboxEvent) {
            let mut tx = self
                .unit_of_work
                .begin(sample_meta())
                .await
                .expect("transaction should begin");
            self.outbox_repository
                .append(&mut tx, event)
                .await
                .expect("outbox event should append");
            tx.commit().await.expect("transaction should commit");
        }
    }

    #[tokio::test]
    async fn relays_definition_events_to_the_definition_topic() {
        let harness = RelayHarness::new(3, 60_000);
        harness
            .append_event(sample_event(
                "evt-published",
                DefinitionEventType::ContentPublished,
            ))
            .await;

        let result = harness
            .service
            .relay_pending_events(sample_request(), sample_meta())
            .await
            .expect("relay should succeed");

        assert_eq!(result.published_count, 1);
        assert_eq!(result.retryable_failure_count, 0);
        assert_eq!(result.dead_letter_count, 0);

        let published = harness
            .bus_publisher
            .published_events()
            .expect("published events should be readable");
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0, "method-library.definition.events");
        assert_eq!(published[0].1.event_id, "evt-published");

        let stored = harness
            .outbox_repository
            .events()
            .expect("outbox events should be readable");
        let event = stored
            .get("evt-published")
            .expect("published event should exist");
        assert_eq!(event.status, OutboxStatus::Published);
    }

    #[tokio::test]
    async fn relays_lifecycle_events_to_the_lifecycle_topic() {
        let harness = RelayHarness::new(3, 60_000);
        harness
            .append_event(sample_event(
                "evt-retired",
                DefinitionEventType::ContentRetired,
            ))
            .await;

        harness
            .service
            .relay_pending_events(sample_request(), sample_meta())
            .await
            .expect("relay should succeed");

        let published = harness
            .bus_publisher
            .published_events()
            .expect("published events should be readable");
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0, "method-library.lifecycle.events");
        assert_eq!(published[0].1.event_id, "evt-retired");
    }

    #[tokio::test]
    async fn marks_retryable_failures_and_sets_the_next_retry_timestamp() {
        let harness = RelayHarness::new(3, 120_000);
        harness
            .bus_publisher
            .set_failure(Some(MethodLibraryError::retryable(
                MethodLibraryErrorCode::BusPublishFailed,
                "temporary bus outage",
            )))
            .expect("failure should be configurable");
        harness
            .append_event(sample_event(
                "evt-deprecated",
                DefinitionEventType::ContentDeprecated,
            ))
            .await;

        let result = harness
            .service
            .relay_pending_events(sample_request(), sample_meta())
            .await
            .expect("retryable failures should not abort the relay");

        assert_eq!(result.published_count, 0);
        assert_eq!(result.retryable_failure_count, 1);
        assert_eq!(result.dead_letter_count, 0);

        let stored = harness
            .outbox_repository
            .events()
            .expect("outbox events should be readable");
        let event = stored
            .get("evt-deprecated")
            .expect("failed event should exist");
        assert_eq!(event.status, OutboxStatus::RetryableFailed);
        assert_eq!(
            event.next_retry_at,
            Some(datetime!(2026-05-27 01:02:00 UTC))
        );

        let observability = harness
            .observability
            .events()
            .expect("observability events should be readable");
        assert!(observability.iter().any(|event| {
            event.name == "outbox_relay_retryable_failure"
                && event.attributes.get("topic")
                    == Some(&"method-library.lifecycle.events".to_string())
        }));
    }

    #[tokio::test]
    async fn dead_letters_retryable_failures_after_the_last_allowed_attempt() {
        let harness = RelayHarness::new(3, 60_000);
        harness
            .bus_publisher
            .set_failure(Some(MethodLibraryError::retryable(
                MethodLibraryErrorCode::BusPublishFailed,
                "temporary bus outage",
            )))
            .expect("failure should be configurable");
        harness
            .append_event(sample_retryable_due_event(
                "evt-fingerprint",
                DefinitionEventType::FingerprintChanged,
                2,
                datetime!(2026-05-27 00:30:00 UTC),
            ))
            .await;

        let result = harness
            .service
            .relay_pending_events(sample_request(), sample_meta())
            .await
            .expect("relay should mark the event dead lettered");

        assert_eq!(result.published_count, 0);
        assert_eq!(result.retryable_failure_count, 0);
        assert_eq!(result.dead_letter_count, 1);

        let stored = harness
            .outbox_repository
            .events()
            .expect("outbox events should be readable");
        let event = stored
            .get("evt-fingerprint")
            .expect("dead-lettered event should exist");
        assert_eq!(event.status, OutboxStatus::DeadLettered);
        assert_eq!(event.retry_count, 2);

        let observability = harness
            .observability
            .events()
            .expect("observability events should be readable");
        assert!(observability.iter().any(|event| {
            event.name == "outbox_relay_dead_letter"
                && event.attributes.get("event_id") == Some(&"evt-fingerprint".to_string())
        }));
    }

    fn sample_request() -> RelayOutboxEventsJobRequest {
        RelayOutboxEventsJobRequest {
            worker_id: "worker-1".to_string(),
            batch_size: 10,
            lease_seconds: 300,
        }
    }

    fn sample_meta() -> RequestMeta {
        RequestMeta {
            request_id: "req-relay".to_string(),
            trace_id: "trace-relay".to_string(),
            idempotency_key: None,
            request_hash: "relay-hash".to_string(),
            received_at: datetime!(2026-05-27 01:00:00 UTC),
        }
    }

    fn sample_event(event_id: &str, event_type: DefinitionEventType) -> OutboxEvent {
        OutboxEvent::new_pending(
            event_id.to_string(),
            "content-1".to_string(),
            DefinitionEventEnvelope {
                event_id: event_id.to_string(),
                event_type,
                schema_version: "1.0".to_string(),
                occurred_at: datetime!(2026-05-27 00:55:00 UTC),
                producer: "L3-method-library".to_string(),
                content_ref: PublishedContentRef {
                    content_id: "content-1".to_string(),
                    kind: MethodContentKind::Qualification,
                    version: ContentVersion::new("1.0.0").expect("version should be valid"),
                    fingerprint: sample_fingerprint("stored"),
                },
                snapshot_ref: None,
                trace: EventTraceContext {
                    request_id: "req-source".to_string(),
                    trace_id: "trace-source".to_string(),
                },
                payload: sample_payload(event_type),
            },
            format!("payload-hash-{event_id}"),
            Some("idem-relay".to_string()),
        )
        .expect("event fixture should be valid")
    }

    fn sample_retryable_due_event(
        event_id: &str,
        event_type: DefinitionEventType,
        retry_count: u32,
        next_retry_at: Timestamp,
    ) -> OutboxEvent {
        let mut event = sample_event(event_id, event_type);
        event.status = OutboxStatus::RetryableFailed;
        event.retry_count = retry_count;
        event.next_retry_at = Some(next_retry_at);
        event
    }

    fn sample_payload(event_type: DefinitionEventType) -> DefinitionEventPayload {
        match event_type {
            DefinitionEventType::ContentPublished => {
                DefinitionEventPayload::ContentPublished(ContentPublishedPayload {
                    gate_ref: ApprovedGateRef {
                        gate_id: "gate-1".to_string(),
                        gate_decision_id: "decision-1".to_string(),
                        approved_at: datetime!(2026-05-27 00:50:00 UTC),
                    },
                    version: ContentVersion::new("1.0.0").expect("version should be valid"),
                    fingerprint: sample_fingerprint("published"),
                })
            }
            DefinitionEventType::ContentDeprecated => {
                DefinitionEventPayload::ContentDeprecated(ContentDeprecatedPayload {
                    reason: "Deprecated by policy".to_string(),
                    effective_at: datetime!(2026-05-27 00:54:00 UTC),
                })
            }
            DefinitionEventType::ContentRetired => {
                DefinitionEventPayload::ContentRetired(ContentRetiredPayload {
                    reason: "Retired by policy".to_string(),
                    retire_policy: "block_new_usage".to_string(),
                })
            }
            DefinitionEventType::FingerprintChanged => {
                DefinitionEventPayload::FingerprintChanged(FingerprintChangedPayload {
                    old_fingerprint: sample_fingerprint("old"),
                    new_fingerprint: sample_fingerprint("new"),
                    change_reason: "Canonicalization updated".to_string(),
                })
            }
        }
    }

    fn sample_fingerprint(suffix: &str) -> CanonicalFingerprint {
        CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, suffix, "1.0")
            .expect("fingerprint should be valid")
    }
}
