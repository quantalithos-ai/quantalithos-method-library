mod support;

use std::collections::HashMap;

use method_library_contracts::{
    DefinitionEventEnvelope, DefinitionEventType, ResolveViewProfileQuery,
};
use method_library_domain::content::{MethodContentKind, PublishedContentRef};
use method_library_domain::definitions::{MethodContentPayload, ViewObjectKind};
use serde_json::json;
use support::{ContractHarness, PublishedFixture};

#[derive(Debug, Clone, PartialEq, Eq)]
struct CatalogEntry {
    content_id: String,
    kind: MethodContentKind,
    version: String,
    fingerprint: String,
}

#[derive(Debug, Default)]
struct IdentityConsumer {
    qualifications: HashMap<String, CatalogEntry>,
    roles: HashMap<String, CatalogEntry>,
}

impl IdentityConsumer {
    fn apply_published_event(&mut self, event: &DefinitionEventEnvelope) {
        let entry = CatalogEntry {
            content_id: event.content_ref.content_id.clone(),
            kind: event.content_ref.kind,
            version: event.content_ref.version.raw.clone(),
            fingerprint: event.content_ref.fingerprint.value.clone(),
        };
        match event.content_ref.kind {
            MethodContentKind::Qualification => {
                self.qualifications.insert(entry.content_id.clone(), entry);
            }
            MethodContentKind::RoleDefinition => {
                self.roles.insert(entry.content_id.clone(), entry);
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessEntry {
    content_id: String,
    kind: MethodContentKind,
    version: String,
    fingerprint: String,
}

#[derive(Debug, Default)]
struct ProcessConsumer {
    task_definitions: HashMap<String, ProcessEntry>,
    template_definitions: HashMap<String, ProcessEntry>,
    process_instances: Vec<String>,
}

impl ProcessConsumer {
    fn apply_published_event(&mut self, event: &DefinitionEventEnvelope) {
        let entry = ProcessEntry {
            content_id: event.content_ref.content_id.clone(),
            kind: event.content_ref.kind,
            version: event.content_ref.version.raw.clone(),
            fingerprint: event.content_ref.fingerprint.value.clone(),
        };
        match event.content_ref.kind {
            MethodContentKind::TaskDefinition => {
                self.task_definitions
                    .insert(entry.content_id.clone(), entry);
            }
            MethodContentKind::ProcessTemplateDef => {
                self.template_definitions
                    .insert(entry.content_id.clone(), entry);
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QualificationBinding {
    qualification_id: String,
    qualification_fingerprint: String,
}

#[derive(Debug, Default)]
struct CapabilityHubConsumer {
    qualification_anchors: HashMap<String, String>,
    bindings: Vec<QualificationBinding>,
    replicated_bodies: Vec<String>,
}

impl CapabilityHubConsumer {
    fn apply_snapshot(&mut self, fixture: &PublishedFixture, payload: &MethodContentPayload) {
        if matches!(payload, MethodContentPayload::Qualification(_)) {
            self.qualification_anchors.insert(
                fixture.content_id.clone(),
                fixture.content_ref.fingerprint.value.clone(),
            );
        }
    }

    fn create_binding(&mut self, qualification_ref: &PublishedContentRef) {
        self.bindings.push(QualificationBinding {
            qualification_id: qualification_ref.content_id.clone(),
            qualification_fingerprint: qualification_ref.fingerprint.value.clone(),
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactDefinitionMapping {
    work_product_key: String,
    artifact_kind: String,
    schema_uri: String,
}

#[derive(Debug, Default)]
struct ArtifactConsumer {
    mappings: HashMap<String, ArtifactDefinitionMapping>,
    artifact_instances: Vec<String>,
}

impl ArtifactConsumer {
    fn apply_snapshot(&mut self, payload: &MethodContentPayload) {
        if let MethodContentPayload::WorkProductDefinition(work_product) = payload {
            self.mappings.insert(
                work_product.work_product_key.clone(),
                ArtifactDefinitionMapping {
                    work_product_key: work_product.work_product_key.clone(),
                    artifact_kind: work_product.artifact_kind.artifact_kind.clone(),
                    schema_uri: work_product.schema_ref.schema_uri.clone(),
                },
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GovernancePolicySource {
    content_id: String,
    version: String,
    fingerprint: String,
}

#[derive(Debug, Default)]
struct GovernanceConsumer {
    policy_sources: HashMap<String, GovernancePolicySource>,
    enforce_results: Vec<String>,
}

impl GovernanceConsumer {
    fn apply_published_event(&mut self, event: &DefinitionEventEnvelope) {
        if event.content_ref.kind == MethodContentKind::AIPolicyDef {
            self.policy_sources.insert(
                event.content_ref.content_id.clone(),
                GovernancePolicySource {
                    content_id: event.content_ref.content_id.clone(),
                    version: event.content_ref.version.raw.clone(),
                    fingerprint: event.content_ref.fingerprint.value.clone(),
                },
            );
        }
    }
}

#[derive(Debug, Default)]
struct ConsoleConsumer {
    observed_profile_events: Vec<String>,
    local_authority_rules: Vec<String>,
}

impl ConsoleConsumer {
    fn observe_published_event(&mut self, event: &DefinitionEventEnvelope) {
        if event.content_ref.kind == MethodContentKind::ViewProfile {
            self.observed_profile_events
                .push(event.content_ref.content_id.clone());
        }
    }
}

#[tokio::test]
async fn syncs_identity_catalog_from_qualification_and_role_events() {
    let harness = ContractHarness::new();
    let qualification = harness.publish_qualification("identity").await;
    let role = harness
        .publish_role("identity", qualification.content_ref.clone())
        .await;
    harness.relay_pending_events("identity-worker").await;

    let counts_before = harness.truth_counts();
    let qualification_event = harness.published_event_for(&qualification.content_id);
    let role_event = harness.published_event_for(&role.content_id);

    assert_eq!(
        qualification_event.event_type,
        DefinitionEventType::ContentPublished
    );
    assert_eq!(qualification_event.schema_version, "1.0");
    assert!(qualification_event.snapshot_ref.is_some());
    assert!(!qualification_event.trace.request_id.is_empty());
    assert!(!qualification_event.trace.trace_id.is_empty());

    let mut consumer = IdentityConsumer::default();
    consumer.apply_published_event(&qualification_event);
    consumer.apply_published_event(&role_event);

    assert_eq!(consumer.qualifications.len(), 1);
    assert_eq!(consumer.roles.len(), 1);
    assert_eq!(
        consumer
            .qualifications
            .get(&qualification.content_id)
            .expect("qualification catalog entry should exist")
            .fingerprint,
        qualification.content_ref.fingerprint.value
    );
    assert_eq!(
        consumer
            .roles
            .get(&role.content_id)
            .expect("role catalog entry should exist")
            .version,
        "1.0.0"
    );
    assert_eq!(harness.truth_counts(), counts_before);
    assert!(
        harness
            .outbox_statuses()
            .into_iter()
            .all(|status| status == method_library_application::ports::OutboxStatus::Published)
    );
}

#[tokio::test]
async fn syncs_process_definition_indexes_without_creating_instances() {
    let harness = ContractHarness::new();
    let qualification = harness.publish_qualification("process").await;
    let role = harness
        .publish_role("process", qualification.content_ref.clone())
        .await;
    let work_product = harness.publish_work_product("process").await;
    let task = harness
        .publish_task("process", work_product.content_ref.clone())
        .await;
    let process_template = harness
        .publish_process_template(
            "process",
            task.content_ref.clone(),
            work_product.content_ref.clone(),
            role.content_ref.clone(),
        )
        .await;
    harness.relay_pending_events("process-worker").await;

    let counts_before = harness.truth_counts();
    let task_event = harness.published_event_for(&task.content_id);
    let template_event = harness.published_event_for(&process_template.content_id);
    let mut consumer = ProcessConsumer::default();
    consumer.apply_published_event(&task_event);
    consumer.apply_published_event(&template_event);

    assert_eq!(consumer.task_definitions.len(), 1);
    assert_eq!(consumer.template_definitions.len(), 1);
    assert!(consumer.process_instances.is_empty());
    assert_eq!(
        consumer
            .template_definitions
            .get(&process_template.content_id)
            .expect("template definition should exist")
            .fingerprint,
        process_template.content_ref.fingerprint.value
    );
    assert_eq!(harness.truth_counts(), counts_before);
}

#[tokio::test]
async fn uses_qualification_snapshots_as_capability_binding_anchors() {
    let harness = ContractHarness::new();
    let qualification = harness.publish_qualification("capability").await;

    let counts_before = harness.truth_counts();
    let export = harness.export_snapshot(&qualification).await;
    let mut consumer = CapabilityHubConsumer::default();
    let payload = export
        .payload
        .content
        .payload
        .as_ref()
        .expect("snapshot payload should include the definition payload");
    consumer.apply_snapshot(&qualification, payload);
    consumer.create_binding(&qualification.content_ref);

    assert_eq!(consumer.qualification_anchors.len(), 1);
    assert_eq!(consumer.bindings.len(), 1);
    assert!(consumer.replicated_bodies.is_empty());
    assert_eq!(
        consumer.bindings[0],
        QualificationBinding {
            qualification_id: qualification.content_id,
            qualification_fingerprint: qualification.content_ref.fingerprint.value,
        }
    );
    assert_eq!(harness.truth_counts(), counts_before);
}

#[tokio::test]
async fn syncs_artifact_definition_mappings_without_creating_instances() {
    let harness = ContractHarness::new();
    let work_product = harness.publish_work_product("artifact").await;

    let counts_before = harness.truth_counts();
    let export = harness.export_snapshot(&work_product).await;
    let payload = export
        .payload
        .content
        .payload
        .expect("snapshot payload should include the definition payload");
    let mut consumer = ArtifactConsumer::default();
    consumer.apply_snapshot(&payload);

    let mapping = consumer
        .mappings
        .get("work_product_artifact")
        .expect("artifact mapping should exist");
    assert_eq!(mapping.artifact_kind, "artifact-artifact");
    assert_eq!(mapping.schema_uri, "schema://artifact/artifact");
    assert!(consumer.artifact_instances.is_empty());
    assert_eq!(harness.truth_counts(), counts_before);
}

#[tokio::test]
async fn syncs_governance_policy_sources_without_enforce_results() {
    let harness = ContractHarness::new();
    let policy = harness.publish_ai_policy("governance").await;
    harness.relay_pending_events("governance-worker").await;

    let counts_before = harness.truth_counts();
    let policy_event = harness.published_event_for(&policy.content_id);
    let mut consumer = GovernanceConsumer::default();
    consumer.apply_published_event(&policy_event);

    assert_eq!(consumer.policy_sources.len(), 1);
    assert!(consumer.enforce_results.is_empty());
    assert_eq!(
        consumer
            .policy_sources
            .get(&policy.content_id)
            .expect("policy source should exist")
            .fingerprint,
        policy.content_ref.fingerprint.value
    );
    assert_eq!(harness.truth_counts(), counts_before);
}

#[tokio::test]
async fn resolves_console_view_profiles_without_local_authority_rules() {
    let harness = ContractHarness::new();
    let qualification = harness.publish_qualification("console").await;
    let role = harness
        .publish_role("console", qualification.content_ref.clone())
        .await;
    let view_profile = harness
        .publish_view_profile("console", role.content_ref.clone())
        .await;
    harness.relay_pending_events("console-worker").await;

    let counts_before = harness.truth_counts();
    let mut consumer = ConsoleConsumer::default();
    consumer.observe_published_event(&harness.published_event_for(&view_profile.content_id));
    let response = harness
        .resolve_view_profile(ResolveViewProfileQuery {
            role_ref: role.content_ref.clone(),
            object_kind: ViewObjectKind::WorkItem,
            scope: json!({ "project": { "type": "service" } }),
        })
        .await;

    let resolved = response
        .view_profile
        .expect("console should resolve one published profile");
    assert_eq!(
        consumer.observed_profile_events,
        vec![view_profile.content_id]
    );
    assert!(consumer.local_authority_rules.is_empty());
    assert_eq!(
        resolved.view_profile_ref.content_id,
        view_profile.content_ref.content_id
    );
    assert_eq!(resolved.scope_rules.len(), 1);
    assert_eq!(resolved.field_rules.len(), 1);
    assert_eq!(resolved.action_rules.len(), 1);
    assert_eq!(harness.truth_counts(), counts_before);
}
