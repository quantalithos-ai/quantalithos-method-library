use method_library_application::ports::MethodAssetStoredOperationResultRepository;
use method_library_application::{
    MethodAssetAdapterAvailabilitySummary, MethodAssetCollaborationTargetSummary,
    MethodAssetDistributionHandoffCommandDispatchInput,
    MethodAssetDistributionHandoffCommandSource, MethodAssetDistributionRepository,
    MethodAssetEventCandidateAssemblyRepository, MethodAssetExpectedVersion,
    MethodAssetPublicationOutcome, MethodAssetPublicationOutcomeRepository,
    MethodAssetRelationReadAnchor, MethodAssetStoredOperationResultKind,
};
use method_library_contracts::fixtures::sample_command_shell;
use method_library_contracts::metadata::IdempotencyKey;
use method_library_contracts::{
    ConsumptionContextRef, DistributionContextRef, DownstreamConsumptionBoundaryRef,
    MethodAssetAdapterAvailabilityStateRef, MethodAssetAdapterSlotRef,
    MethodAssetAdapterSlotRefSet, MethodAssetConsumptionAvailabilityMarker,
    MethodAssetConsumptionAvailabilityMarkerSource, MethodAssetConsumptionAvailabilityTarget,
    MethodAssetDistributionAdjustmentReasonRef, MethodAssetDistributionRef,
    MethodAssetEventCandidateReasonRef, MethodAssetHandoffBindingStateRef,
    MethodAssetHandoffBoundaryMarkerRef, MethodAssetHandoffTargetRef,
    MethodAssetHandoffTargetRefSet, MethodAssetInfraSafeDiagnosticRef,
    MethodAssetPublicationBoundaryMarkerRef, MethodAssetPublisherBindingStateRef,
    MethodAssetRelationRef, MethodAssetTargetRegistryScopeRef, MethodLibraryCapabilityKind,
    MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};
use method_library_infra::InMemoryMethodAssetDistributionHandoffRuntime;
use std::sync::{Arc, Mutex};

fn typed_ref(
    kind: MethodLibraryTypedBoundaryRefKind,
    value: &str,
) -> MethodLibraryTypedBoundaryRef {
    MethodLibraryTypedBoundaryRef::from_verified_source(kind, value)
}

fn marker(kind: MethodLibraryTypedBoundaryRefKind, value: &str) -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::no_body(typed_ref(kind, value))
}

fn diagnostic(value: &str) -> MethodAssetInfraSafeDiagnosticRef {
    MethodAssetInfraSafeDiagnosticRef::new(format!("diagnostic:{value}"))
}

fn availability(
    label: &str,
    target_state: MethodAssetConsumptionAvailabilityTarget,
) -> MethodAssetConsumptionAvailabilityMarker {
    MethodAssetConsumptionAvailabilityMarker::new(
        marker(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            &format!("availability:{label}"),
        ),
        target_state,
        MethodAssetConsumptionAvailabilityMarkerSource::AvailabilityResolver,
        marker(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            &format!("availability-source:{label}"),
        ),
        None,
    )
}

fn shell(
    idempotency_key: &str,
    intent_kind: MethodLibraryTypedBoundaryRefKind,
) -> method_library_contracts::MethodLibraryCommandShell {
    let mut shell = sample_command_shell();
    shell.capability_kind = MethodLibraryCapabilityKind::RelationDistribution;
    shell.boundary_ref = typed_ref(intent_kind, &format!("intent:{idempotency_key}"));
    shell.metadata.request.idempotency_key = Some(IdempotencyKey::new(idempotency_key));
    shell
}

fn relation_anchor(label: &str) -> MethodAssetRelationReadAnchor {
    MethodAssetRelationReadAnchor {
        relation_ref: MethodAssetRelationRef::new(format!("relation:{label}")),
        distribution_context_ref: None,
    }
}

fn prepare_source(label: &str) -> MethodAssetDistributionHandoffCommandSource {
    MethodAssetDistributionHandoffCommandSource::PrepareDistributionRef {
        relation_ref: MethodAssetRelationRef::new(format!("relation:{label}")),
        requested_distribution_ref: Some(MethodAssetDistributionRef::new(format!(
            "distribution:{label}"
        ))),
        distribution_context_ref: DistributionContextRef::new(format!("context:{label}")),
        consumption_context_ref: ConsumptionContextRef::new(format!("consumption:{label}")),
        boundary_ref: DownstreamConsumptionBoundaryRef::new(format!("boundary:{label}")),
        availability_marker: availability(label, MethodAssetConsumptionAvailabilityTarget::Ready),
        candidate_reason_ref: MethodAssetEventCandidateReasonRef::new(marker(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            &format!("candidate-reason:{label}"),
        )),
    }
}

fn seam_source(
    label: &str,
) -> method_library_application::MethodAssetDistributionHandoffSeamSource {
    method_library_application::MethodAssetDistributionHandoffSeamSource {
        target_registry_scope_ref: MethodAssetTargetRegistryScopeRef::new(format!(
            "target-scope:{label}"
        )),
        required_slot_refs: MethodAssetAdapterSlotRefSet::from_refs([
            MethodAssetAdapterSlotRef::new(format!("slot:{label}")),
        ]),
        publisher_binding_ref: MethodAssetPublisherBindingStateRef::new(format!(
            "publisher:{label}"
        )),
        handoff_binding_ref: Some(MethodAssetHandoffBindingStateRef::new(format!(
            "handoff:{label}"
        ))),
        publication_boundary_marker_ref: MethodAssetPublicationBoundaryMarkerRef::new(marker(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            &format!("publication-boundary:{label}"),
        )),
        handoff_boundary_marker_ref: Some(MethodAssetHandoffBoundaryMarkerRef::new(marker(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            &format!("handoff-boundary:{label}"),
        ))),
    }
}

fn dispatch_input(
    runtime: &InMemoryMethodAssetDistributionHandoffRuntime,
    idempotency_key: &str,
    source: MethodAssetDistributionHandoffCommandSource,
    seam_source: Option<method_library_application::MethodAssetDistributionHandoffSeamSource>,
    intent_kind: MethodLibraryTypedBoundaryRefKind,
) -> MethodAssetDistributionHandoffCommandDispatchInput {
    let support_ref_factory = runtime.support_ref_factory();
    let mut factory = support_ref_factory
        .lock()
        .expect("factory lock should be available");
    let api_entry_context_ref = factory.new_api_entry_context_ref();
    let application_dispatch_ref = factory.distribution_handoff_dispatch_ref();
    drop(factory);
    MethodAssetDistributionHandoffCommandDispatchInput {
        command_shell: shell(idempotency_key, intent_kind),
        command_source: source,
        seam_source,
        api_entry_context_ref,
        application_dispatch_ref,
    }
}

struct RecordingSupportRefFactory {
    inner: Box<dyn method_library_application::MethodAssetDistributionHandoffSupportRefFactory>,
    publication_refs: Arc<Mutex<Vec<method_library_contracts::MethodAssetPublicationOutcomeRef>>>,
}

impl method_library_application::MethodAssetDistributionHandoffSupportRefFactory
    for RecordingSupportRefFactory
{
    fn distribution_handoff_dispatch_ref(
        &self,
    ) -> method_library_contracts::MethodAssetApplicationDispatchRef {
        self.inner.distribution_handoff_dispatch_ref()
    }

    fn new_api_entry_context_ref(
        &mut self,
    ) -> method_library_contracts::MethodAssetApiEntryContextRef {
        self.inner.new_api_entry_context_ref()
    }

    fn build_distribution_handoff_replay_envelope(
        &mut self,
        input: method_library_application::MethodAssetDistributionHandoffReplayEnvelopeFactoryInput,
    ) -> Result<
        method_library_application::MethodAssetDistributionHandoffReplayEnvelope,
        method_library_application::MethodAssetReplayEnvelopeBuildError,
    > {
        self.inner.build_distribution_handoff_replay_envelope(input)
    }

    fn new_stored_operation_result_ref(
        &mut self,
    ) -> method_library_contracts::MethodAssetStoredOperationResultRef {
        self.inner.new_stored_operation_result_ref()
    }

    fn new_accepted_operation_summary_ref(
        &mut self,
    ) -> method_library_contracts::MethodAssetAcceptedOperationSummaryRef {
        self.inner.new_accepted_operation_summary_ref()
    }

    fn new_safe_reject_reason_ref(
        &mut self,
    ) -> method_library_contracts::MethodAssetSafeRejectReasonRef {
        self.inner.new_safe_reject_reason_ref()
    }

    fn new_safe_ignore_reason_ref(
        &mut self,
    ) -> method_library_contracts::MethodAssetSafeIgnoreReasonRef {
        self.inner.new_safe_ignore_reason_ref()
    }

    fn new_effect_summary_ref(&mut self) -> method_library_contracts::MethodAssetEffectSummaryRef {
        self.inner.new_effect_summary_ref()
    }

    fn new_replay_marker_ref(&mut self) -> method_library_contracts::MethodAssetReplayMarkerRef {
        self.inner.new_replay_marker_ref()
    }

    fn new_distribution_ref(
        &mut self,
        relation_ref: method_library_contracts::MethodAssetRelationRef,
        distribution_context_ref: method_library_contracts::DistributionContextRef,
        operation_context_ref: method_library_contracts::MethodAssetOperationContextRef,
        operation_digest_ref: method_library_contracts::MethodAssetOperationDigestRef,
        dedup_scope_ref: method_library_contracts::MethodAssetDedupScopeRef,
    ) -> method_library_contracts::MethodAssetDistributionRef {
        self.inner.new_distribution_ref(
            relation_ref,
            distribution_context_ref,
            operation_context_ref,
            operation_digest_ref,
            dedup_scope_ref,
        )
    }

    fn new_event_candidate_assembly_ref(
        &mut self,
        distribution_ref: method_library_contracts::MethodAssetDistributionRef,
        operation_context_ref: method_library_contracts::MethodAssetOperationContextRef,
        operation_digest_ref: method_library_contracts::MethodAssetOperationDigestRef,
        dedup_scope_ref: method_library_contracts::MethodAssetDedupScopeRef,
    ) -> method_library_contracts::MethodAssetEventCandidateAssemblyRef {
        self.inner.new_event_candidate_assembly_ref(
            distribution_ref,
            operation_context_ref,
            operation_digest_ref,
            dedup_scope_ref,
        )
    }

    fn new_publication_outcome_ref(
        &mut self,
        candidate_ref: method_library_contracts::MethodAssetEventCandidateAssemblyRef,
        target_ref_set: method_library_contracts::MethodAssetHandoffTargetRefSet,
    ) -> method_library_contracts::MethodAssetPublicationOutcomeRef {
        let publication_ref = self
            .inner
            .new_publication_outcome_ref(candidate_ref, target_ref_set);
        self.publication_refs
            .lock()
            .expect("publication-ref recorder lock should be available")
            .push(publication_ref.clone());
        publication_ref
    }

    fn new_handoff_marker_ref(
        &mut self,
        candidate_ref: method_library_contracts::MethodAssetEventCandidateAssemblyRef,
        target_ref: method_library_contracts::MethodAssetHandoffTargetRef,
    ) -> method_library_contracts::MethodAssetHandoffMarkerRef {
        self.inner.new_handoff_marker_ref(candidate_ref, target_ref)
    }
}

fn install_publication_ref_recorder(
    runtime: &InMemoryMethodAssetDistributionHandoffRuntime,
) -> Arc<Mutex<Vec<method_library_contracts::MethodAssetPublicationOutcomeRef>>> {
    let publication_refs = Arc::new(Mutex::new(Vec::new()));
    let factory_handle = runtime.support_ref_factory();
    let mut factory = factory_handle
        .lock()
        .expect("factory lock should be available");
    let inner = std::mem::replace(
        &mut *factory,
        Box::new(
            method_library_infra::InMemoryMethodAssetDistributionHandoffSupportRefFactory::default(
            ),
        ),
    );
    *factory = Box::new(RecordingSupportRefFactory {
        inner,
        publication_refs: Arc::clone(&publication_refs),
    });
    publication_refs
}

fn recorded_publication_outcome(
    runtime: &InMemoryMethodAssetDistributionHandoffRuntime,
    publication_refs: &Arc<Mutex<Vec<method_library_contracts::MethodAssetPublicationOutcomeRef>>>,
) -> method_library_application::Versioned<MethodAssetPublicationOutcome> {
    let publication_ref = publication_refs
        .lock()
        .expect("publication-ref recorder lock should be available")
        .last()
        .cloned()
        .expect("publication ref should be factory-issued");
    runtime
        .publication_repository()
        .get_publication_outcome(publication_ref)
        .expect("publication lookup should succeed")
        .expect("publication outcome should exist")
}

#[test]
fn prepare_with_seam_commits_candidate_and_duplicate_does_not_rerun_seams() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("prepare"));
    let input = dispatch_input(
        &runtime,
        "prepare-with-seam",
        prepare_source("prepare"),
        Some(seam_source("prepare")),
        MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
    );

    let first = runtime
        .facade()
        .dispatch_distribution_handoff_command(input.clone());
    assert_eq!(
        first.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    assert_eq!(runtime.builder().call_count(), 1);
    assert_eq!(runtime.publisher().call_count(), 1);
    assert_eq!(runtime.handoff().call_count(), 1);
    assert_eq!(runtime.publication_outcome_count(), 1);
    assert_eq!(runtime.handoff_outcome_count(), 1);
    assert!(
        runtime
            .candidate_repository()
            .get_event_candidate_assembly(
                method_library_contracts::MethodAssetEventCandidateAssemblyRef::new("missing")
            )
            .expect("candidate lookup should succeed")
            .is_none()
    );

    let duplicate = runtime
        .facade()
        .dispatch_distribution_handoff_command(input);
    assert_eq!(duplicate, first);
    assert_eq!(runtime.builder().call_count(), 1);
    assert_eq!(runtime.publisher().call_count(), 1);
    assert_eq!(runtime.handoff().call_count(), 1);
}

#[test]
fn prepare_without_seam_has_no_candidate_or_post_commit_outcome() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("no-seam"));
    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "prepare-no-seam",
            prepare_source("no-seam"),
            None,
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));

    assert_eq!(
        output.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    assert_eq!(runtime.publication_outcome_count(), 0);
    assert_eq!(runtime.handoff_outcome_count(), 0);
    assert_eq!(runtime.publisher().call_count(), 0);
    assert_eq!(runtime.handoff().call_count(), 0);
}

#[test]
fn adjust_requires_loaded_distribution_version_and_preserves_truth_on_conflict() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("adjust"));
    let prepare = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "prepare-adjust",
            prepare_source("adjust"),
            None,
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));
    assert_eq!(
        prepare.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    let distribution_ref = MethodAssetDistributionRef::new("distribution:adjust");
    let loaded = runtime
        .distribution_repository()
        .get_distribution_with_version(distribution_ref.clone())
        .expect("distribution lookup should succeed")
        .expect("distribution should exist");

    let source = MethodAssetDistributionHandoffCommandSource::AdjustDistributionContext {
        relation_ref: MethodAssetRelationRef::new("relation:adjust"),
        distribution_ref: distribution_ref.clone(),
        previous_context_ref: DistributionContextRef::new("context:adjust"),
        new_context_ref: DistributionContextRef::new("context:adjust-next"),
        adjustment_reason_ref: MethodAssetDistributionAdjustmentReasonRef::new(marker(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            "adjustment-reason",
        )),
        candidate_reason_ref: MethodAssetEventCandidateReasonRef::new(marker(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            "adjust-candidate-reason",
        )),
        expected_distribution_version: MethodAssetExpectedVersion::from(loaded.version),
    };
    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "adjust-distribution",
            source,
            None,
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionContextAdjustIntent,
        ));
    assert_eq!(
        output.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    let adjusted = runtime
        .distribution_repository()
        .get_distribution_with_version(distribution_ref.clone())
        .expect("distribution lookup should succeed")
        .expect("distribution should exist");
    assert_eq!(adjusted.version.0, 2);
    assert_eq!(
        adjusted.value.distribution_context_ref.as_public_ref(),
        "context:adjust-next"
    );
}

#[test]
fn availability_mark_requires_mapper_owned_degraded_decision() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("availability"));
    let prepare = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "prepare-availability",
            prepare_source("availability"),
            None,
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));
    assert_eq!(
        prepare.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );

    let availability_marker = availability(
        "availability-degraded",
        MethodAssetConsumptionAvailabilityTarget::Unavailable,
    );
    let source = MethodAssetDistributionHandoffCommandSource::MarkDistributionAvailability {
        relation_ref: MethodAssetRelationRef::new("relation:availability"),
        distribution_ref: MethodAssetDistributionRef::new("distribution:availability"),
        distribution_context_ref: DistributionContextRef::new("context:availability"),
        availability_marker: availability_marker.clone(),
        candidate_reason_ref: MethodAssetEventCandidateReasonRef::new(marker(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            "availability-candidate-reason",
        )),
    };
    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "mark-availability",
            source,
            None,
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionAvailabilityMarkIntent,
        ));
    assert_eq!(
        output.result_kind,
        MethodAssetStoredOperationResultKind::Rejected
    );
    let stored = runtime
        .distribution_repository()
        .get_distribution_with_version(MethodAssetDistributionRef::new("distribution:availability"))
        .expect("distribution lookup should succeed")
        .expect("distribution should exist");
    assert_eq!(
        stored.value.availability_marker.target_state,
        MethodAssetConsumptionAvailabilityTarget::Ready
    );
}

#[test]
fn disabled_target_does_not_call_publisher_or_handoff() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("disabled"));
    let publication_refs = install_publication_ref_recorder(&runtime);
    let reason_ref = marker(
        MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
        "target-disabled",
    );
    let diagnostic_ref = diagnostic("target-disabled");
    runtime
        .target_registry()
        .set_summary(MethodAssetCollaborationTargetSummary::Disabled {
            target_ref_set: MethodAssetHandoffTargetRefSet::from_refs([
                MethodAssetHandoffTargetRef::new("target:disabled"),
            ]),
            reason_ref: reason_ref.clone(),
            diagnostic_ref: diagnostic_ref.clone(),
        });
    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "prepare-disabled-target",
            prepare_source("disabled"),
            Some(seam_source("disabled")),
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));
    assert_eq!(
        output.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    assert_eq!(runtime.publisher().call_count(), 0);
    assert_eq!(runtime.handoff().call_count(), 0);
    assert_eq!(runtime.target_registry().call_count(), 1);
    assert_eq!(runtime.publication_outcome_count(), 1);
    let outcome = recorded_publication_outcome(&runtime, &publication_refs).value;
    let candidate_ref = match outcome {
        MethodAssetPublicationOutcome::Blocked {
            candidate_ref,
            reason_ref: actual_reason_ref,
            diagnostic_ref: actual_diagnostic_ref,
            ..
        } => {
            assert_eq!(actual_reason_ref, reason_ref);
            assert_eq!(actual_diagnostic_ref, diagnostic_ref);
            candidate_ref
        }
        other => panic!("expected blocked publication outcome, got {other:?}"),
    };
    assert!(
        runtime
            .candidate_repository()
            .get_event_candidate_assembly(candidate_ref)
            .expect("candidate lookup should succeed")
            .is_some()
    );
    assert!(
        runtime
            .distribution_repository()
            .get_distribution_with_version(MethodAssetDistributionRef::new("distribution:disabled"))
            .expect("distribution lookup should succeed")
            .is_some()
    );
}

#[test]
fn disabled_adapter_persists_blocked_outcome_with_port_diagnostic() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("adapter-disabled"));
    let publication_refs = install_publication_ref_recorder(&runtime);
    let reason_ref = marker(
        MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
        "adapter-disabled",
    );
    let diagnostic_ref = diagnostic("adapter-disabled");
    runtime
        .adapter_availability()
        .set_summary(MethodAssetAdapterAvailabilitySummary::Disabled {
            availability_state_ref: MethodAssetAdapterAvailabilityStateRef::new(
                "adapter-state:disabled",
            ),
            marker_ref: marker(
                MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
                "adapter-marker:disabled",
            ),
            reason_ref: reason_ref.clone(),
            diagnostic_ref: diagnostic_ref.clone(),
        });
    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "prepare-disabled-adapter",
            prepare_source("adapter-disabled"),
            Some(seam_source("adapter-disabled")),
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));
    assert_eq!(
        output.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    assert_eq!(runtime.target_registry().call_count(), 1);
    assert_eq!(runtime.publisher().call_count(), 0);
    assert_eq!(runtime.handoff().call_count(), 0);
    let outcome = recorded_publication_outcome(&runtime, &publication_refs).value;
    let candidate_ref = match outcome {
        MethodAssetPublicationOutcome::Blocked {
            candidate_ref,
            reason_ref: actual_reason_ref,
            diagnostic_ref: actual_diagnostic_ref,
            ..
        } => {
            assert_eq!(actual_reason_ref, reason_ref);
            assert_eq!(actual_diagnostic_ref, diagnostic_ref);
            candidate_ref
        }
        other => panic!("expected blocked publication outcome, got {other:?}"),
    };
    assert!(
        runtime
            .candidate_repository()
            .get_event_candidate_assembly(candidate_ref)
            .expect("candidate lookup should succeed")
            .is_some()
    );
    assert!(
        runtime
            .stored_result_repository()
            .get_stored_operation_result(output.stored_result_ref)
            .expect("stored-result lookup should succeed")
            .is_some()
    );
}

#[test]
fn unavailable_adapter_maps_to_unavailable_outcome_without_delivery() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("adapter-unavailable"));
    let publication_refs = install_publication_ref_recorder(&runtime);
    let reason_ref = marker(
        MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
        "adapter-unavailable",
    );
    let diagnostic_ref = diagnostic("adapter-unavailable");
    runtime.adapter_availability().set_summary(
        MethodAssetAdapterAvailabilitySummary::Unavailable {
            availability_state_ref: MethodAssetAdapterAvailabilityStateRef::new(
                "adapter-state:unavailable",
            ),
            marker_ref: reason_ref.clone(),
            diagnostic_ref: diagnostic_ref.clone(),
        },
    );
    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "prepare-unavailable-adapter",
            prepare_source("adapter-unavailable"),
            Some(seam_source("adapter-unavailable")),
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));
    assert_eq!(
        output.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    assert_eq!(runtime.publisher().call_count(), 0);
    assert_eq!(runtime.handoff().call_count(), 0);
    let outcome = recorded_publication_outcome(&runtime, &publication_refs).value;
    match outcome {
        MethodAssetPublicationOutcome::Unavailable {
            reason_ref: actual_reason_ref,
            diagnostic_ref: actual_diagnostic_ref,
            ..
        } => {
            assert_eq!(actual_reason_ref, reason_ref);
            assert_eq!(actual_diagnostic_ref, diagnostic_ref);
        }
        other => panic!("expected unavailable publication outcome, got {other:?}"),
    }
}

#[test]
fn degraded_adapter_maps_to_blocked_outcome_with_marker_and_diagnostic() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("adapter-degraded"));
    let publication_refs = install_publication_ref_recorder(&runtime);
    let marker_ref = marker(
        MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
        "adapter-degraded-marker",
    );
    let diagnostic_ref = diagnostic("adapter-degraded");
    runtime
        .adapter_availability()
        .set_summary(MethodAssetAdapterAvailabilitySummary::Degraded {
            availability_state_ref: MethodAssetAdapterAvailabilityStateRef::new(
                "adapter-state:degraded",
            ),
            marker_ref: marker_ref.clone(),
            diagnostic_ref: diagnostic_ref.clone(),
        });
    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "prepare-degraded-adapter",
            prepare_source("adapter-degraded"),
            Some(seam_source("adapter-degraded")),
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));
    assert_eq!(
        output.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    assert_eq!(runtime.target_registry().call_count(), 1);
    assert_eq!(runtime.publisher().call_count(), 0);
    assert_eq!(runtime.handoff().call_count(), 0);
    let outcome = recorded_publication_outcome(&runtime, &publication_refs).value;
    assert!(matches!(
        outcome,
        MethodAssetPublicationOutcome::Blocked {
            reason_ref: actual_reason_ref,
            diagnostic_ref: actual_diagnostic_ref,
            ..
        } if actual_reason_ref == marker_ref && actual_diagnostic_ref == diagnostic_ref
    ));
}

#[test]
fn blocked_target_maps_to_blocked_outcome_without_delivery() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("target-blocked"));
    let publication_refs = install_publication_ref_recorder(&runtime);
    let reason_ref = marker(
        MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
        "target-blocked",
    );
    let diagnostic_ref = diagnostic("target-blocked");
    runtime
        .target_registry()
        .set_summary(MethodAssetCollaborationTargetSummary::Blocked {
            target_ref_set: MethodAssetHandoffTargetRefSet::from_refs([
                MethodAssetHandoffTargetRef::new("target:blocked"),
            ]),
            reason_ref: reason_ref.clone(),
            diagnostic_ref: diagnostic_ref.clone(),
        });
    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "prepare-blocked-target",
            prepare_source("target-blocked"),
            Some(seam_source("target-blocked")),
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));
    assert_eq!(
        output.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    assert_eq!(runtime.publisher().call_count(), 0);
    assert_eq!(runtime.handoff().call_count(), 0);
    let outcome = recorded_publication_outcome(&runtime, &publication_refs).value;
    assert!(matches!(
        outcome,
        MethodAssetPublicationOutcome::Blocked {
            reason_ref: actual_reason_ref,
            diagnostic_ref: actual_diagnostic_ref,
            ..
        } if actual_reason_ref == reason_ref && actual_diagnostic_ref == diagnostic_ref
    ));
}

#[test]
fn unavailable_target_maps_to_unavailable_outcome_without_delivery() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("target-unavailable"));
    let publication_refs = install_publication_ref_recorder(&runtime);
    let reason_ref = marker(
        MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
        "target-unavailable",
    );
    let diagnostic_ref = diagnostic("target-unavailable");
    runtime
        .target_registry()
        .set_summary(MethodAssetCollaborationTargetSummary::Unavailable {
            target_ref_set: MethodAssetHandoffTargetRefSet::from_refs([
                MethodAssetHandoffTargetRef::new("target:unavailable"),
            ]),
            reason_ref: reason_ref.clone(),
            diagnostic_ref: diagnostic_ref.clone(),
        });
    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "prepare-unavailable-target",
            prepare_source("target-unavailable"),
            Some(seam_source("target-unavailable")),
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));
    assert_eq!(
        output.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    assert_eq!(runtime.publisher().call_count(), 0);
    assert_eq!(runtime.handoff().call_count(), 0);
    let outcome = recorded_publication_outcome(&runtime, &publication_refs).value;
    assert!(matches!(
        outcome,
        MethodAssetPublicationOutcome::Unavailable {
            reason_ref: actual_reason_ref,
            diagnostic_ref: actual_diagnostic_ref,
            ..
        } if actual_reason_ref == reason_ref && actual_diagnostic_ref == diagnostic_ref
    ));
}

#[test]
fn selector_source_mismatch_is_safe_rejected_before_builder_or_mutation() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("mismatch"));
    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "selector-source-mismatch",
            MethodAssetDistributionHandoffCommandSource::AdjustDistributionContext {
                relation_ref: MethodAssetRelationRef::new("relation:mismatch"),
                distribution_ref: MethodAssetDistributionRef::new("distribution:mismatch"),
                previous_context_ref: DistributionContextRef::new("context:mismatch"),
                new_context_ref: DistributionContextRef::new("context:mismatch-next"),
                adjustment_reason_ref: MethodAssetDistributionAdjustmentReasonRef::new(marker(
                    MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
                    "mismatch-adjustment",
                )),
                candidate_reason_ref: MethodAssetEventCandidateReasonRef::new(marker(
                    MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
                    "mismatch-candidate",
                )),
                expected_distribution_version: MethodAssetExpectedVersion(
                    method_library_application::MethodAssetRepositoryVersion(1),
                ),
            },
            None,
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));
    assert_eq!(
        output.result_kind,
        MethodAssetStoredOperationResultKind::Rejected
    );
    assert_eq!(runtime.builder().call_count(), 0);
    assert_eq!(
        runtime
            .distribution_repository()
            .get_distribution_with_version(MethodAssetDistributionRef::new("distribution:mismatch"))
            .expect("distribution lookup should succeed"),
        None
    );
}

#[test]
fn seam_source_is_part_of_digest_and_commit_unknown_replays_post_commit_once() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("digest"));
    runtime.simulate_commit_unknown_once();
    let input = dispatch_input(
        &runtime,
        "same-idempotency-different-seam",
        prepare_source("digest"),
        Some(seam_source("digest")),
        MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
    );
    let first = runtime
        .facade()
        .dispatch_distribution_handoff_command(input.clone());
    assert_eq!(
        first.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    assert_eq!(runtime.publisher().call_count(), 1);

    let changed = dispatch_input(
        &runtime,
        "same-idempotency-different-seam",
        prepare_source("digest"),
        None,
        MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
    );
    let conflict = runtime
        .facade()
        .dispatch_distribution_handoff_command(changed);
    assert_eq!(
        conflict.result_kind,
        MethodAssetStoredOperationResultKind::Conflict
    );
    assert_eq!(runtime.builder().call_count(), 1);
    assert_eq!(runtime.publisher().call_count(), 1);
}

#[test]
fn publication_failure_does_not_roll_back_accepted_distribution() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("failure"));
    runtime.publisher().set_failure(Some((
        marker(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            "publisher-failed",
        ),
        MethodAssetInfraSafeDiagnosticRef::new("publisher-diagnostic"),
    )));
    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "prepare-publisher-failure",
            prepare_source("failure"),
            Some(seam_source("failure")),
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));
    assert_eq!(
        output.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );
    assert!(
        runtime
            .distribution_repository()
            .get_distribution_with_version(MethodAssetDistributionRef::new("distribution:failure"))
            .expect("distribution lookup should succeed")
            .is_some()
    );
    assert_eq!(runtime.publisher().call_count(), 1);
    assert_eq!(runtime.handoff().call_count(), 0);
}

#[test]
fn publication_outcome_persistence_failure_stops_handoff_without_rolling_back_truth() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("publication-save-failure"));
    runtime.fail_next_publication_outcome_save();

    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "publication-outcome-save-failure",
            prepare_source("publication-save-failure"),
            Some(seam_source("publication-save-failure")),
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));

    assert_eq!(
        output.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    assert!(
        runtime
            .distribution_repository()
            .get_distribution_with_version(MethodAssetDistributionRef::new(
                "distribution:publication-save-failure"
            ))
            .expect("distribution lookup should succeed")
            .is_some()
    );
    assert_eq!(runtime.publisher().call_count(), 1);
    assert_eq!(runtime.handoff().call_count(), 0);
    assert_eq!(runtime.publication_outcome_count(), 0);
    assert_eq!(runtime.handoff_outcome_count(), 0);
}

#[test]
fn handoff_outcome_persistence_failure_stops_later_targets_without_rollback() {
    let runtime = InMemoryMethodAssetDistributionHandoffRuntime::new();
    runtime.seed_relation_anchor(relation_anchor("handoff-save-failure"));
    runtime.fail_next_handoff_outcome_save();
    runtime
        .target_registry()
        .set_summary(MethodAssetCollaborationTargetSummary::Enabled {
            target_ref_set: MethodAssetHandoffTargetRefSet::from_refs([
                MethodAssetHandoffTargetRef::new("target:first"),
                MethodAssetHandoffTargetRef::new("target:second"),
            ]),
            summary_marker_ref: marker(
                MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
                "targets-enabled",
            ),
        });

    let output = runtime
        .facade()
        .dispatch_distribution_handoff_command(dispatch_input(
            &runtime,
            "handoff-outcome-save-failure",
            prepare_source("handoff-save-failure"),
            Some(seam_source("handoff-save-failure")),
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent,
        ));

    assert_eq!(
        output.result_kind,
        MethodAssetStoredOperationResultKind::Accepted
    );
    assert!(
        runtime
            .distribution_repository()
            .get_distribution_with_version(MethodAssetDistributionRef::new(
                "distribution:handoff-save-failure"
            ))
            .expect("distribution lookup should succeed")
            .is_some()
    );
    assert_eq!(runtime.publisher().call_count(), 1);
    assert_eq!(runtime.handoff().call_count(), 1);
    assert_eq!(runtime.publication_outcome_count(), 1);
    assert_eq!(runtime.handoff_outcome_count(), 0);
}
