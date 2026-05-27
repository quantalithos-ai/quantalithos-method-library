mod support;

use std::collections::HashMap;

use method_library_contracts::DefinitionEventEnvelope;
use method_library_domain::content::MethodContentKind;
use method_library_domain::definitions::MethodContentPayload;
use support::{ContractHarness, PublishedFixture};

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecoveryCatalogEntry {
    content_id: String,
    version: String,
    fingerprint: String,
}

#[derive(Debug, Default)]
struct RecoveryIdentityConsumer {
    definitions: HashMap<String, RecoveryCatalogEntry>,
}

impl RecoveryIdentityConsumer {
    fn apply_event(&mut self, event: &DefinitionEventEnvelope) {
        self.definitions.insert(
            event.content_ref.content_id.clone(),
            RecoveryCatalogEntry {
                content_id: event.content_ref.content_id.clone(),
                version: event.content_ref.version.raw.clone(),
                fingerprint: event.content_ref.fingerprint.value.clone(),
            },
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SnapshotArtifactMapping {
    artifact_kind: String,
    schema_uri: String,
    fingerprint: String,
}

#[derive(Debug, Default)]
struct SnapshotArtifactConsumer {
    mappings: HashMap<String, SnapshotArtifactMapping>,
}

impl SnapshotArtifactConsumer {
    fn apply_snapshot(&mut self, fixture: &PublishedFixture, payload: &MethodContentPayload) {
        if let MethodContentPayload::WorkProductDefinition(work_product) = payload {
            self.mappings.insert(
                fixture.content_id.clone(),
                SnapshotArtifactMapping {
                    artifact_kind: work_product.artifact_kind.artifact_kind.clone(),
                    schema_uri: work_product.schema_ref.schema_uri.clone(),
                    fingerprint: fixture.content_ref.fingerprint.value.clone(),
                },
            );
        }
    }
}

#[tokio::test]
async fn recovers_missed_events_through_replay_without_rewriting_truth() {
    let harness = ContractHarness::new();
    let qualification = harness.publish_qualification("replay").await;
    let role = harness
        .publish_role("replay", qualification.content_ref.clone())
        .await;

    let counts_before = harness.truth_counts();
    let result = harness
        .replay_definition_events("identity-recovery", None)
        .await;
    let replayed_events = harness.replay_bus_events();

    assert_eq!(result.replayed_count, 2);
    assert_eq!(replayed_events.len(), 2);
    assert_eq!(
        result.next_cursor,
        replayed_events
            .last()
            .map(|(_, event, _)| event.event_id.clone())
    );
    assert_eq!(
        harness
            .checkpoint_cursor("replay_consumer:identity-recovery")
            .await,
        result.next_cursor
    );

    let mut consumer = RecoveryIdentityConsumer::default();
    for (_, event, _) in &replayed_events {
        consumer.apply_event(event);
    }

    assert_eq!(consumer.definitions.len(), 2);
    assert_eq!(
        consumer
            .definitions
            .get(&qualification.content_id)
            .expect("qualification should replay")
            .fingerprint,
        qualification.content_ref.fingerprint.value
    );
    assert_eq!(
        consumer
            .definitions
            .get(&role.content_id)
            .expect("role should replay")
            .version,
        "1.0.0"
    );
    assert_eq!(harness.truth_counts(), counts_before);
}

#[tokio::test]
async fn resyncs_downstream_indexes_from_snapshots_without_rewriting_truth() {
    let harness = ContractHarness::new();
    let work_product = harness.publish_work_product("snapshot").await;

    let counts_before = harness.truth_counts();
    let export = harness.export_snapshot(&work_product).await;
    let payload = export
        .payload
        .content
        .payload
        .as_ref()
        .expect("snapshot payload should include the definition payload");
    let mut consumer = SnapshotArtifactConsumer::default();
    consumer.apply_snapshot(&work_product, payload);

    let mapping = consumer
        .mappings
        .get(&work_product.content_id)
        .expect("snapshot resync should restore the artifact mapping");
    assert_eq!(mapping.artifact_kind, "artifact-snapshot");
    assert_eq!(mapping.schema_uri, "schema://artifact/snapshot");
    assert_eq!(
        mapping.fingerprint,
        work_product.content_ref.fingerprint.value
    );
    assert_eq!(
        export.content_ref.kind,
        MethodContentKind::WorkProductDefinition
    );
    assert_eq!(harness.truth_counts(), counts_before);
}
