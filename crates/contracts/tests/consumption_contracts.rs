use method_library_contracts::{
    ConsumptionBoundaryReasonRef, ConsumptionContextRef, DefinitionUseBoundaryGuardRef,
    DefinitionUseBoundaryGuardState, DefinitionUseGuardReasonRef, DownstreamConsumptionBoundaryRef,
    DownstreamConsumptionBoundaryState, DownstreamForbiddenWriteKind,
    DownstreamForbiddenWriteKindSet, FormalMethodAssetVersionRef, FormalVersionRequiredState,
    FormalVersionRequirement, MethodAssetAllowedUseKind, MethodAssetAllowedUseKindSet,
    MethodAssetConsumptionAvailabilityMarker, MethodAssetConsumptionAvailabilityMarkerSource,
    MethodAssetConsumptionAvailabilityTarget, MethodAssetConsumptionMaterialCursorRef,
    MethodAssetConsumptionMaterialRef, MethodAssetConsumptionMaterialScopeRef,
    MethodAssetConsumptionMaterialState, MethodAssetConsumptionSummary, MethodAssetDefinitionRef,
    MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};

fn typed_ref(
    kind: MethodLibraryTypedBoundaryRefKind,
    value: &str,
) -> MethodLibraryTypedBoundaryRef {
    MethodLibraryTypedBoundaryRef::from_verified_source(kind, value)
}

fn marker(kind: MethodLibraryTypedBoundaryRefKind, value: &str) -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::boundary(typed_ref(kind, value))
}

#[test]
fn consumption_ref_kinds_are_stable() {
    let cases = [
        (
            MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterial,
            "\"method_asset_consumption_material\"",
        ),
        (
            MethodLibraryTypedBoundaryRefKind::ConsumptionContext,
            "\"consumption_context\"",
        ),
        (
            MethodLibraryTypedBoundaryRefKind::DownstreamConsumptionBoundary,
            "\"downstream_consumption_boundary\"",
        ),
        (
            MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterialCursor,
            "\"method_asset_consumption_material_cursor\"",
        ),
        (
            MethodLibraryTypedBoundaryRefKind::DefinitionUseBoundaryGuard,
            "\"definition_use_boundary_guard\"",
        ),
        (
            MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterialScope,
            "\"method_asset_consumption_material_scope\"",
        ),
    ];

    for (kind, expected) in cases {
        let encoded = serde_json::to_string(&kind).expect("ref kind should serialize");
        assert_eq!(encoded, expected);
    }
}

#[test]
fn consumption_wrappers_reject_wrong_kinds() {
    let wrong_material = typed_ref(
        MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
        "ml:material:wrong",
    );
    let material_error = MethodAssetConsumptionMaterialRef::try_from(wrong_material)
        .expect_err("material wrapper must reject a mismatched kind");
    assert_eq!(
        material_error.expected_kind(),
        MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterial
    );

    let wrong_context = typed_ref(
        MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
        "ml:ctx:wrong",
    );
    let context_error = ConsumptionContextRef::try_from(wrong_context)
        .expect_err("context wrapper must reject a mismatched kind");
    assert_eq!(
        context_error.expected_kind(),
        MethodLibraryTypedBoundaryRefKind::ConsumptionContext
    );

    let wrong_boundary = typed_ref(
        MethodLibraryTypedBoundaryRefKind::DistributionContextRef,
        "ml:boundary:wrong",
    );
    let boundary_error = DownstreamConsumptionBoundaryRef::try_from(wrong_boundary)
        .expect_err("boundary wrapper must reject a mismatched kind");
    assert_eq!(
        boundary_error.expected_kind(),
        MethodLibraryTypedBoundaryRefKind::DownstreamConsumptionBoundary
    );
}

#[test]
fn consumption_state_and_marker_labels_are_exact() {
    let constrained = serde_json::to_string(&MethodAssetConsumptionMaterialState::Constrained)
        .expect("material state should serialize");
    assert_eq!(constrained, "\"constrained\"");

    let manual = serde_json::to_string(&DefinitionUseBoundaryGuardState::ManualReviewRequired)
        .expect("guard state should serialize");
    assert_eq!(manual, "\"manual_review_required\"");

    let retired = serde_json::to_string(&DownstreamConsumptionBoundaryState::Retired)
        .expect("boundary state should serialize");
    assert_eq!(retired, "\"retired\"");

    let target = serde_json::to_string(&MethodAssetConsumptionAvailabilityTarget::Unavailable)
        .expect("availability target should serialize");
    assert_eq!(target, "\"unavailable\"");

    let source = serde_json::to_string(
        &MethodAssetConsumptionAvailabilityMarkerSource::DownstreamConsumptionBoundaryGuard,
    )
    .expect("availability source should serialize");
    assert_eq!(source, "\"downstream_consumption_boundary_guard\"");
}

#[test]
fn deterministic_sets_dedup_and_preserve_first_seen_order() {
    let allowed = MethodAssetAllowedUseKindSet::from_kinds([
        MethodAssetAllowedUseKind::Read,
        MethodAssetAllowedUseKind::Distribute,
        MethodAssetAllowedUseKind::Read,
    ]);
    assert_eq!(
        allowed.allowed_kinds,
        vec![
            MethodAssetAllowedUseKind::Read,
            MethodAssetAllowedUseKind::Distribute
        ]
    );

    let forbidden = DownstreamForbiddenWriteKindSet::from_kinds([
        DownstreamForbiddenWriteKind::DefinitionTruth,
        DownstreamForbiddenWriteKind::FormalVersionTruth,
        DownstreamForbiddenWriteKind::DefinitionTruth,
    ]);
    assert_eq!(
        forbidden.forbidden_kinds,
        vec![
            DownstreamForbiddenWriteKind::DefinitionTruth,
            DownstreamForbiddenWriteKind::FormalVersionTruth
        ]
    );
}

#[test]
fn requirement_and_summary_carriers_keep_exact_fields() {
    let summary = MethodAssetConsumptionSummary::new(
        marker(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:summary:marker",
        ),
        FormalMethodAssetVersionRef::new("ml:version:1"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        DownstreamConsumptionBoundaryRef::new("ml:boundary:1"),
    );
    assert_eq!(summary.formal_version_ref.as_public_ref(), "ml:version:1");
    assert_eq!(summary.definition_ref.as_public_ref(), "ml:def:1");
    assert_eq!(summary.boundary_ref.as_public_ref(), "ml:boundary:1");

    let requirement = FormalVersionRequirement::new(
        FormalVersionRequiredState::ActiveOnly,
        marker(
            MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersion,
            "ml:requirement:1",
        ),
    );
    assert_eq!(
        requirement.required_state,
        FormalVersionRequiredState::ActiveOnly
    );
}

#[test]
fn safe_reason_wrappers_keep_safe_markers() {
    let guard_reason = DefinitionUseGuardReasonRef::new(marker(
        MethodLibraryTypedBoundaryRefKind::DefinitionUseBoundaryGuard,
        "ml:guard:reason",
    ));
    assert!(guard_reason.as_safe_marker().is_public_safe());

    let boundary_reason = ConsumptionBoundaryReasonRef::new(marker(
        MethodLibraryTypedBoundaryRefKind::DownstreamConsumptionBoundary,
        "ml:boundary:reason",
    ));
    assert!(boundary_reason.as_safe_marker().is_public_safe());
}

#[test]
fn availability_marker_copies_only_exact_target_state() {
    let availability_marker = MethodAssetConsumptionAvailabilityMarker::new(
        marker(
            MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterial,
            "ml:marker:availability",
        ),
        MethodAssetConsumptionAvailabilityTarget::Stale,
        MethodAssetConsumptionAvailabilityMarkerSource::AvailabilityResolver,
        marker(
            MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterialCursor,
            "ml:marker:source",
        ),
        Some(marker(
            MethodLibraryTypedBoundaryRefKind::DefinitionUseBoundaryGuard,
            "ml:marker:reason",
        )),
    );

    assert_eq!(
        availability_marker.material_state(),
        MethodAssetConsumptionMaterialState::Stale
    );
}

#[test]
fn helper_refs_and_sets_roundtrip_current_boundary_values() {
    let cursor = MethodAssetConsumptionMaterialCursorRef::new("ml:cursor:1");
    assert_eq!(cursor.as_public_ref(), "ml:cursor:1");

    let guard = DefinitionUseBoundaryGuardRef::new("ml:guard:1");
    assert_eq!(guard.as_public_ref(), "ml:guard:1");

    let scope = MethodAssetConsumptionMaterialScopeRef::new("ml:scope:1");
    assert_eq!(scope.as_public_ref(), "ml:scope:1");
}
