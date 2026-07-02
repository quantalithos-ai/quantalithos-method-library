use method_library_contracts::{
    FormalMethodAssetVersionRef, FormalMethodAssetVersionState, FormalVersionBoundarySummary,
    FormalizationBasisSafeSummary, FormalizationBasisSummaryRef, FormalizationBasisSummaryRefSet,
    FormalizationStateReasonSummary, FormalizationStateRef, GovernanceBasisRef,
    MethodAssetCatalogEntryRef, MethodAssetDefinitionRef, MethodLibrarySafeMarker,
    MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};
use method_library_domain::{
    FormalMethodAssetVersion, FormalizationBasisSummary, FormalizationState,
    MethodLibraryDomainErrorKind,
};

fn typed_ref(
    kind: MethodLibraryTypedBoundaryRefKind,
    value: &str,
) -> MethodLibraryTypedBoundaryRef {
    MethodLibraryTypedBoundaryRef::from_verified_source(kind, value)
}

fn no_body_marker(value: &str) -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::no_body(typed_ref(
        MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
        value,
    ))
}

fn boundary_marker(value: &str) -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::boundary(typed_ref(
        MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
        value,
    ))
}

fn sample_reason_summary() -> FormalizationStateReasonSummary {
    FormalizationStateReasonSummary::new(
        boundary_marker("ml:reason:1"),
        FormalizationBasisSummaryRefSet::from_refs([FormalizationBasisSummaryRef::new(
            "ml:basis:1",
        )]),
        None,
    )
}

fn sample_boundary_summary() -> FormalVersionBoundarySummary {
    FormalVersionBoundarySummary::new(
        boundary_marker("ml:boundary:1"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        MethodAssetCatalogEntryRef::new("ml:catalog:1"),
        FormalizationStateRef::new("ml:state:1"),
        FormalizationBasisSummaryRefSet::from_refs([FormalizationBasisSummaryRef::new(
            "ml:basis:1",
        )]),
    )
}

#[test]
fn basis_summary_rejects_missing_kind_compatible_source() {
    let external_summary = FormalizationBasisSummary::from_external_summary(
        FormalizationBasisSummaryRef::new("ml:basis:external"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        Some(MethodAssetCatalogEntryRef::new("ml:catalog:1")),
        method_library_contracts::ExternalSourceSummaryRef::new("ml:external-summary:1"),
        FormalizationBasisSafeSummary::new(no_body_marker("ml:marker:external"), None, None, None),
    );
    let error = external_summary
        .assert_body_free()
        .expect_err("external basis requires a matching external summary ref");
    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::BodyFreeBoundaryViolation
    );

    let governance_summary = FormalizationBasisSummary::from_governance_basis(
        FormalizationBasisSummaryRef::new("ml:basis:governance"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        None,
        GovernanceBasisRef::new("ml:governance:1"),
        FormalizationBasisSafeSummary::new(
            no_body_marker("ml:marker:governance"),
            None,
            None,
            None,
        ),
    );
    let error = governance_summary
        .assert_body_free()
        .expect_err("governance basis requires a matching governance basis ref");
    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::BodyFreeBoundaryViolation
    );
}

#[test]
fn basis_summary_accepts_matching_safe_sources() {
    let basis_summary = FormalizationBasisSummary::from_basis_reassessment(
        FormalizationBasisSummaryRef::new("ml:basis:reassess"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        Some(MethodAssetCatalogEntryRef::new("ml:catalog:1")),
        FormalizationBasisSafeSummary::new(
            no_body_marker("ml:marker:reassess"),
            Some(method_library_contracts::ExternalSourceSummaryRef::new(
                "ml:external-summary:1",
            )),
            Some(GovernanceBasisRef::new("ml:governance:1")),
            Some(boundary_marker("ml:marker:reassess:next")),
        ),
    );

    assert!(basis_summary.assert_body_free().is_ok());
    assert!(
        basis_summary
            .assert_applicable_to_definition(&MethodAssetDefinitionRef::new("ml:def:1"))
            .is_ok()
    );
}

#[test]
fn formalization_state_rejects_invalid_transitions_and_silent_overwrite() {
    let mut state = FormalizationState::pending_for_definition(
        FormalizationStateRef::new("ml:state:1"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        MethodAssetCatalogEntryRef::new("ml:catalog:1"),
        sample_reason_summary(),
    );

    state
        .mark_eligible(FormalizationBasisSummaryRefSet::from_refs([
            FormalizationBasisSummaryRef::new("ml:basis:1"),
            FormalizationBasisSummaryRef::new("ml:basis:1"),
        ]))
        .expect("pending state should become eligible");
    assert_eq!(
        state.basis_summary_refs.refs,
        vec![FormalizationBasisSummaryRef::new("ml:basis:1")]
    );

    state
        .mark_formalized(FormalMethodAssetVersionRef::new("ml:version:1"))
        .expect("eligible state should establish a formal version");
    assert!(state.is_formalized());

    let reevaluate_error = state
        .mark_assessment_pending(
            sample_reason_summary(),
            FormalizationBasisSummaryRefSet::new(),
        )
        .expect_err("established state must not re-enter pending in place");
    assert_eq!(
        reevaluate_error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );

    let overwrite_error = state
        .mark_formalized(FormalMethodAssetVersionRef::new("ml:version:2"))
        .expect_err("established state must reject silent overwrite");
    assert_eq!(
        overwrite_error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );
}

#[test]
fn formalization_state_can_block_and_recover_before_establishment() {
    let mut state = FormalizationState::pending_for_definition(
        FormalizationStateRef::new("ml:state:1"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        MethodAssetCatalogEntryRef::new("ml:catalog:1"),
        sample_reason_summary(),
    );

    state
        .block(FormalizationStateReasonSummary::new(
            boundary_marker("ml:reason:blocked"),
            FormalizationBasisSummaryRefSet::new(),
            Some(
                method_library_contracts::FormalizationEligibilityRejectionRef::new(
                    "ml:rejection:1",
                ),
            ),
        ))
        .expect("pending state can become ineligible");
    assert_eq!(
        state.state_kind,
        method_library_contracts::FormalizationStateKind::Ineligible
    );

    state
        .mark_assessment_pending(
            sample_reason_summary(),
            FormalizationBasisSummaryRefSet::new(),
        )
        .expect("ineligible state can re-enter pending");
    assert_eq!(
        state.state_kind,
        method_library_contracts::FormalizationStateKind::AssessmentPending
    );
}

#[test]
fn formal_version_guards_preserve_active_and_block_retired_or_superseded_reuse() {
    let mut version = FormalMethodAssetVersion::from_formalization_state(
        FormalMethodAssetVersionRef::new("ml:version:1"),
        sample_boundary_summary(),
    );

    version
        .record_semantic_change(sample_boundary_summary())
        .expect("active version can record semantic change");
    assert_eq!(version.version_state, FormalMethodAssetVersionState::Active);
    assert!(version.is_current_for(&FormalizationStateRef::new("ml:state:1")));

    version
        .supersede_with(&FormalMethodAssetVersionRef::new("ml:version:2"))
        .expect("active version can be superseded");
    assert_eq!(
        version.version_state,
        FormalMethodAssetVersionState::Superseded
    );

    let change_error = version
        .record_semantic_change(sample_boundary_summary())
        .expect_err("superseded version must reject semantic change reuse");
    assert_eq!(
        change_error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );

    version
        .mark_retired()
        .expect("superseded version can retire");
    assert_eq!(
        version.version_state,
        FormalMethodAssetVersionState::Retired
    );

    let retire_again = version
        .mark_retired()
        .expect_err("retired version stays terminal");
    assert_eq!(
        retire_again.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );

    let supersede_again = version
        .supersede_with(&FormalMethodAssetVersionRef::new("ml:version:3"))
        .expect_err("retired version cannot be superseded again");
    assert_eq!(
        supersede_again.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );
}

#[test]
fn formal_version_requires_stable_definition_and_state_identity() {
    let version = FormalMethodAssetVersion::from_formalization_state(
        FormalMethodAssetVersionRef::new("ml:version:1"),
        sample_boundary_summary(),
    );

    assert!(
        version
            .assert_definition_matches(&MethodAssetDefinitionRef::new("ml:def:1"))
            .is_ok()
    );
    assert!(
        version
            .assert_formalized_by(&FormalizationStateRef::new("ml:state:1"))
            .is_ok()
    );

    let definition_error = version
        .assert_definition_matches(&MethodAssetDefinitionRef::new("ml:def:2"))
        .expect_err("definition drift must be rejected");
    assert_eq!(
        definition_error.kind(),
        MethodLibraryDomainErrorKind::InvariantViolation
    );

    let state_error = version
        .assert_formalized_by(&FormalizationStateRef::new("ml:state:2"))
        .expect_err("state drift must be rejected");
    assert_eq!(
        state_error.kind(),
        MethodLibraryDomainErrorKind::InvariantViolation
    );
}
