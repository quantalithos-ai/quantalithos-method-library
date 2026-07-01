use method_library_contracts::{
    MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};
use method_library_domain::{
    ConsistencyProtectionJudgement, ConsistencyProtectionPolicy, DefinitionUseBoundaryGuard,
    DefinitionUseBoundaryGuardState, DownstreamConsumptionBoundary,
    DownstreamConsumptionBoundaryState, ExternalBodyBoundaryRule, ExternalBodyBoundaryState,
    MethodLibraryDomainErrorKind, RelationIntegrityJudgement, RelationIntegrityRule,
};

fn sample_ref(
    kind: MethodLibraryTypedBoundaryRefKind,
    value: &str,
) -> MethodLibraryTypedBoundaryRef {
    MethodLibraryTypedBoundaryRef::from_verified_source(kind, value)
}

fn sample_marker(value: &str) -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::boundary(sample_ref(
        MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
        value,
    ))
}

fn sample_no_body_marker(value: &str) -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::no_body(sample_ref(
        MethodLibraryTypedBoundaryRefKind::ArtifactArchiveRef,
        value,
    ))
}

fn sample_definition_guard() -> DefinitionUseBoundaryGuard {
    DefinitionUseBoundaryGuard::try_new(
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:guard:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
            "ml:def:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            "ml:formal-version:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
            "ml:context:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::DistributionContextRef,
            "ml:boundary:1",
        )),
        Some(sample_marker("ml:marker:guard")),
        DefinitionUseBoundaryGuardState::Monitoring,
    )
    .expect("guard shell should be valid")
}

fn sample_downstream_boundary() -> DownstreamConsumptionBoundary {
    DownstreamConsumptionBoundary::try_new(
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::DistributionContextRef,
            "ml:boundary:2",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
            "ml:context:2",
        )),
        Some(sample_marker("ml:marker:boundary")),
        DownstreamConsumptionBoundaryState::Registered,
    )
    .expect("boundary shell should be valid")
}

fn sample_consistency_policy() -> ConsistencyProtectionPolicy {
    ConsistencyProtectionPolicy::try_new(
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:policy:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            "ml:version:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::ConsumptionImpactSourceRef,
            "ml:impact:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:trace:1",
        )),
        Some(sample_marker("ml:marker:consistency")),
        ConsistencyProtectionJudgement::UnknownImpactPending,
    )
    .expect("consistency shell should be valid")
}

fn sample_relation_rule() -> RelationIntegrityRule {
    RelationIntegrityRule::try_new(
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:rule:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::RelatedMethodAssetRef,
            "ml:relation:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
            "ml:def:source",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
            "ml:def:target",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRef,
            "ml:distribution:1",
        )),
        Some(sample_marker("ml:marker:relation")),
        RelationIntegrityJudgement::IntegrityPending,
    )
    .expect("relation shell should be valid")
}

fn sample_external_rule(state: ExternalBodyBoundaryState) -> ExternalBodyBoundaryRule {
    ExternalBodyBoundaryRule::try_new(
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:rule:external",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::ExternalSourceRef,
            "ml:external:1",
        )),
        None,
        None,
        Some(sample_no_body_marker("ml:marker:external")),
        state,
    )
    .expect("external shell should be valid")
}

#[test]
fn policy_shells_require_current_boundary_typed_carriers() {
    let definition_error = DefinitionUseBoundaryGuard::try_new(
        None,
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
            "ml:def:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            "ml:formal-version:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
            "ml:context:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::DistributionContextRef,
            "ml:boundary:1",
        )),
        Some(sample_marker("ml:marker:guard")),
        DefinitionUseBoundaryGuardState::Monitoring,
    )
    .expect_err("guard shell must reject missing guard_ref");
    assert_eq!(
        definition_error.kind(),
        MethodLibraryDomainErrorKind::MissingRequiredTypedInput
    );

    let boundary_error = DownstreamConsumptionBoundary::try_new(
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::DistributionContextRef,
            "ml:boundary:2",
        )),
        None,
        Some(sample_marker("ml:marker:boundary")),
        DownstreamConsumptionBoundaryState::Registered,
    )
    .expect_err("boundary shell must reject missing context_ref");
    assert_eq!(
        boundary_error.kind(),
        MethodLibraryDomainErrorKind::MissingRequiredTypedInput
    );

    let consistency_error = ConsistencyProtectionPolicy::try_new(
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:policy:1",
        )),
        None,
        None,
        None,
        Some(sample_marker("ml:marker:consistency")),
        ConsistencyProtectionJudgement::InputRejected,
    )
    .expect_err("consistency shell must reject missing protected_version_ref");
    assert_eq!(
        consistency_error.kind(),
        MethodLibraryDomainErrorKind::MissingRequiredTypedInput
    );

    let relation_error = RelationIntegrityRule::try_new(
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:rule:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::RelatedMethodAssetRef,
            "ml:relation:1",
        )),
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
            "ml:def:source",
        )),
        None,
        None,
        Some(sample_marker("ml:marker:relation")),
        RelationIntegrityJudgement::IntegrityPending,
    )
    .expect_err("relation shell must reject missing target_definition_ref");
    assert_eq!(
        relation_error.kind(),
        MethodLibraryDomainErrorKind::MissingRequiredTypedInput
    );

    let external_error = ExternalBodyBoundaryRule::try_new(
        None,
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::ExternalSourceRef,
            "ml:external:1",
        )),
        None,
        None,
        Some(sample_no_body_marker("ml:marker:external")),
        ExternalBodyBoundaryState::InvalidCandidate,
    )
    .expect_err("external rule must reject missing rule_ref");
    assert_eq!(
        external_error.kind(),
        MethodLibraryDomainErrorKind::MissingRequiredTypedInput
    );
}

#[test]
fn guard_state_tracks_legal_and_illegal_transitions() {
    let recorded = DefinitionUseBoundaryGuardState::Monitoring
        .record_violation(true, false)
        .expect("monitoring branch should allow a safe recorded transition");
    assert_eq!(recorded, DefinitionUseBoundaryGuardState::ViolationRecorded);

    let rejected = DefinitionUseBoundaryGuardState::Monitoring
        .record_violation(false, true)
        .expect("monitoring branch should allow a rejected candidate transition");
    assert_eq!(rejected, DefinitionUseBoundaryGuardState::RejectedCandidate);

    let transition_error = DefinitionUseBoundaryGuardState::ViolationRecorded
        .record_violation(true, false)
        .expect_err("recorded state cannot re-enter the monitoring transition");
    assert_eq!(
        transition_error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );
}

#[test]
fn rejected_guard_branch_surfaces_policy_rejected() {
    let guard = sample_definition_guard();
    let error = guard
        .evaluate_violation_candidate(None, true)
        .expect_err("missing safe reason or raw body should surface policy rejection");
    assert_eq!(error.kind(), MethodLibraryDomainErrorKind::PolicyRejected);
}

#[test]
fn downstream_boundary_adjustments_stay_within_allowed_labels() {
    let registered = DownstreamConsumptionBoundaryState::register(false);
    let constrained = registered.adjust(DownstreamConsumptionBoundaryState::Constrained);
    let unavailable = constrained.adjust(DownstreamConsumptionBoundaryState::Unavailable);

    assert_eq!(registered, DownstreamConsumptionBoundaryState::Registered);
    assert_eq!(constrained, DownstreamConsumptionBoundaryState::Constrained);
    assert_eq!(unavailable, DownstreamConsumptionBoundaryState::Unavailable);

    let updated =
        sample_downstream_boundary().adjust(DownstreamConsumptionBoundaryState::Constrained);
    assert_eq!(
        updated.state,
        DownstreamConsumptionBoundaryState::Constrained
    );
}

#[test]
fn consistency_and_relation_judgements_reject_illegal_follow_up() {
    let policy = sample_consistency_policy()
        .reconcile(ConsistencyProtectionJudgement::ProtectionEstablished)
        .expect("pending protection can reconcile to established");
    assert_eq!(
        policy.state,
        ConsistencyProtectionJudgement::ProtectionEstablished
    );

    let consistency_error = ConsistencyProtectionJudgement::ProtectionEstablished
        .reconcile(ConsistencyProtectionJudgement::ProtectionConstrained)
        .expect_err("established protection cannot re-enter the constrained branch");
    assert_eq!(
        consistency_error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );

    let violated = sample_relation_rule()
        .mark_violation()
        .expect("pending integrity can mark a violation");
    assert_eq!(violated.state, RelationIntegrityJudgement::ViolationMarked);

    let relation_error = RelationIntegrityJudgement::IntegrityRejected
        .mark_violation()
        .expect_err("rejected integrity cannot mark a later violation");
    assert_eq!(
        relation_error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );
}

#[test]
fn external_body_rule_enforces_body_free_redline_and_invariants() {
    let asserted = sample_external_rule(ExternalBodyBoundaryState::InvalidCandidate)
        .assert_summary_body_free(false)
        .expect("body-free candidate should remain asserted");
    assert_eq!(asserted, ExternalBodyBoundaryState::AssertedBodyFree);

    let redline_error = sample_external_rule(ExternalBodyBoundaryState::InvalidCandidate)
        .assert_summary_body_free(true)
        .expect_err("raw body candidate must be rejected at the pure-domain boundary");
    assert_eq!(
        redline_error.kind(),
        MethodLibraryDomainErrorKind::BodyFreeBoundaryViolation
    );

    let rejected = sample_external_rule(ExternalBodyBoundaryState::AssertedBodyFree)
        .reject_candidate(true)
        .expect("asserted body-free candidate can move to rejected");
    assert_eq!(
        rejected.state,
        ExternalBodyBoundaryState::BodyCandidateRejected
    );

    let transition_error = ExternalBodyBoundaryState::InvalidCandidate
        .reject_candidate(true, true)
        .expect_err("invalid candidates cannot jump into the rejection branch");
    assert_eq!(
        transition_error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );

    let invalid_rule = ExternalBodyBoundaryRule::try_new(
        Some(sample_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:rule:external-invalid",
        )),
        None,
        None,
        None,
        Some(sample_no_body_marker("ml:marker:external-invalid")),
        ExternalBodyBoundaryState::AssertedBodyFree,
    )
    .expect("manual shell construction is allowed before invariant validation");
    let invariant_error = invalid_rule
        .assert_current_boundary_invariant()
        .expect_err("asserted body-free state requires a candidate ref");
    assert_eq!(
        invariant_error.kind(),
        MethodLibraryDomainErrorKind::InvariantViolation
    );
}
