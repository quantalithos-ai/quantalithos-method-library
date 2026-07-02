use method_library_contracts::{
    ForbiddenFormalizationTriggerKind, ForbiddenFormalizationTriggerKindSet,
    FormalMethodAssetVersionRef, FormalMethodAssetVersionRefSet, FormalMethodAssetVersionState,
    FormalizationBasisKind, FormalizationBasisKindSet, FormalizationBasisSafeSummary,
    FormalizationBasisSummaryRef, FormalizationBasisSummaryRefSet,
    FormalizationEligibilityRejectionRef, FormalizationStateKind, FormalizationStateRef,
    GovernanceBasisRef, MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef,
    MethodLibraryTypedBoundaryRefKind, OptionalGovernanceBasisRequirement,
};

fn typed_ref(
    kind: MethodLibraryTypedBoundaryRefKind,
    value: &str,
) -> MethodLibraryTypedBoundaryRef {
    MethodLibraryTypedBoundaryRef::from_verified_source(kind, value)
}

#[test]
fn formalization_ref_kinds_are_stable() {
    let basis_kind = MethodLibraryTypedBoundaryRefKind::FormalizationBasisSummary;
    let encoded = serde_json::to_string(&basis_kind).expect("basis kind should serialize");
    assert_eq!(encoded, "\"formalization_basis_summary\"");

    let state_kind = MethodLibraryTypedBoundaryRefKind::FormalizationState;
    let encoded = serde_json::to_string(&state_kind).expect("state kind should serialize");
    assert_eq!(encoded, "\"formalization_state\"");

    let version_kind = MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersion;
    let encoded = serde_json::to_string(&version_kind).expect("version kind should serialize");
    assert_eq!(encoded, "\"formal_method_asset_version\"");

    let retire_intent = MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersionRetireIntent;
    let encoded =
        serde_json::to_string(&retire_intent).expect("retire intent label should serialize");
    assert_eq!(encoded, "\"formal_method_asset_version_retire_intent\"");
}

#[test]
fn formalization_selector_intent_labels_are_exact() {
    let cases = [
        (
            MethodLibraryTypedBoundaryRefKind::MethodAssetFormalizationEligibilityEvaluateIntent,
            "\"method_asset_formalization_eligibility_evaluate_intent\"",
        ),
        (
            MethodLibraryTypedBoundaryRefKind::MethodAssetFormalizationInitiateIntent,
            "\"method_asset_formalization_initiate_intent\"",
        ),
        (
            MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersionEstablishIntent,
            "\"formal_method_asset_version_establish_intent\"",
        ),
        (
            MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersionSemanticChangeRecordIntent,
            "\"formal_method_asset_version_semantic_change_record_intent\"",
        ),
        (
            MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersionSupersedeIntent,
            "\"formal_method_asset_version_supersede_intent\"",
        ),
        (
            MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersionRetireIntent,
            "\"formal_method_asset_version_retire_intent\"",
        ),
    ];

    for (kind, expected) in cases {
        let encoded = serde_json::to_string(&kind).expect("selector kind should serialize");
        assert_eq!(encoded, expected);
    }
}

#[test]
fn named_formalization_wrappers_reject_wrong_kinds() {
    let wrong_basis = typed_ref(
        MethodLibraryTypedBoundaryRefKind::FormalizationState,
        "ml:basis:wrong",
    );
    let basis_error = FormalizationBasisSummaryRef::try_from(wrong_basis)
        .expect_err("basis wrapper must reject a mismatched kind");
    assert_eq!(
        basis_error.expected_kind(),
        MethodLibraryTypedBoundaryRefKind::FormalizationBasisSummary
    );
    assert_eq!(
        basis_error.actual_kind(),
        MethodLibraryTypedBoundaryRefKind::FormalizationState
    );

    let wrong_version = typed_ref(
        MethodLibraryTypedBoundaryRefKind::FormalizationEligibilityRejection,
        "ml:version:wrong",
    );
    let version_error = FormalMethodAssetVersionRef::try_from(wrong_version)
        .expect_err("version wrapper must reject a mismatched kind");
    assert_eq!(
        version_error.expected_kind(),
        MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersion
    );
    assert_eq!(
        version_error.actual_kind(),
        MethodLibraryTypedBoundaryRefKind::FormalizationEligibilityRejection
    );
}

#[test]
fn formalization_ref_sets_dedup_and_preserve_first_seen_order() {
    let first_basis = FormalizationBasisSummaryRef::new("ml:basis:1");
    let second_basis = FormalizationBasisSummaryRef::new("ml:basis:2");
    let basis_set = FormalizationBasisSummaryRefSet::from_refs([
        first_basis.clone(),
        second_basis.clone(),
        first_basis.clone(),
    ]);
    assert_eq!(basis_set.refs, vec![first_basis, second_basis]);

    let first_version = FormalMethodAssetVersionRef::new("ml:version:1");
    let second_version = FormalMethodAssetVersionRef::new("ml:version:2");
    let version_set = FormalMethodAssetVersionRefSet::from_refs([
        first_version.clone(),
        second_version.clone(),
        first_version.clone(),
    ]);
    assert_eq!(version_set.refs, vec![first_version, second_version]);
}

#[test]
fn basis_and_trigger_sets_are_canonically_deduped() {
    let basis_kind_set = FormalizationBasisKindSet::from_kinds([
        FormalizationBasisKind::ExternalSummary,
        FormalizationBasisKind::GovernanceBasis,
        FormalizationBasisKind::ExternalSummary,
    ]);
    assert_eq!(
        basis_kind_set.kinds,
        vec![
            FormalizationBasisKind::ExternalSummary,
            FormalizationBasisKind::GovernanceBasis
        ]
    );

    let trigger_set = ForbiddenFormalizationTriggerKindSet::from_kinds([
        ForbiddenFormalizationTriggerKind::Read,
        ForbiddenFormalizationTriggerKind::Query,
        ForbiddenFormalizationTriggerKind::Read,
    ]);
    assert_eq!(
        trigger_set.forbidden_kinds,
        vec![
            ForbiddenFormalizationTriggerKind::Read,
            ForbiddenFormalizationTriggerKind::Query
        ]
    );
}

#[test]
fn governance_requirement_required_implies_allowed() {
    let marker = MethodLibrarySafeMarker::boundary(typed_ref(
        MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
        "ml:marker:governance",
    ));
    let requirement = OptionalGovernanceBasisRequirement::new(false, true, marker);
    assert!(requirement.governance_basis_allowed);
    assert!(requirement.governance_basis_required);
}

#[test]
fn formalization_basis_safe_summary_roundtrips_existing_support_refs() {
    let summary = FormalizationBasisSafeSummary::new(
        MethodLibrarySafeMarker::no_body(typed_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:marker:no-body",
        )),
        Some(method_library_contracts::ExternalSourceSummaryRef::new(
            "ml:external-summary:1",
        )),
        Some(GovernanceBasisRef::new("ml:governance:1")),
        Some(MethodLibrarySafeMarker::boundary(typed_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:marker:reassess",
        ))),
    );

    assert_eq!(
        summary.governance_basis_ref,
        Some(GovernanceBasisRef::new("ml:governance:1"))
    );
}

#[test]
fn formalization_state_kind_labels_are_exact() {
    let encoded = serde_json::to_string(&FormalizationStateKind::VersionEstablished)
        .expect("state label should serialize");
    assert_eq!(encoded, "\"version_established\"");

    let encoded = serde_json::to_string(&FormalMethodAssetVersionState::Superseded)
        .expect("version label should serialize");
    assert_eq!(encoded, "\"superseded\"");

    let rejection_ref = FormalizationEligibilityRejectionRef::new("ml:rejection:1");
    assert_eq!(rejection_ref.as_public_ref(), "ml:rejection:1");

    let state_ref = FormalizationStateRef::new("ml:state:1");
    assert_eq!(state_ref.as_public_ref(), "ml:state:1");
}
