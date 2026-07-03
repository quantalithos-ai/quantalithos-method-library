use method_library_contracts::{
    ConsumptionBoundaryReasonRef, ConsumptionContextRef, DefinitionUseBoundaryGuardRef,
    DefinitionUseBoundaryGuardState, DefinitionUseGuardReasonRef, DownstreamConsumptionBoundaryRef,
    DownstreamConsumptionBoundaryState, DownstreamForbiddenWriteKind,
    DownstreamForbiddenWriteKindSet, FormalMethodAssetVersionRef, FormalMethodAssetVersionState,
    FormalVersionRequiredState, FormalVersionRequirement, MethodAssetAllowedUseKind,
    MethodAssetAllowedUseKindSet, MethodAssetConsumptionAvailabilityMarker,
    MethodAssetConsumptionAvailabilityMarkerSource, MethodAssetConsumptionAvailabilityTarget,
    MethodAssetConsumptionMaterialCursorRef, MethodAssetConsumptionMaterialRef,
    MethodAssetConsumptionMaterialScopeRef, MethodAssetConsumptionMaterialState,
    MethodAssetConsumptionSummary, MethodAssetDefinitionRef, MethodLibrarySafeMarker,
    MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};
use method_library_domain::{
    DefinitionUseBoundaryGuard, DownstreamConsumptionBoundary, MethodAssetConsumptionMaterial,
    MethodLibraryDomainErrorKind,
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

fn no_body_marker(value: &str) -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::no_body(typed_ref(
        MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
        value,
    ))
}

fn sample_summary() -> MethodAssetConsumptionSummary {
    MethodAssetConsumptionSummary::new(
        no_body_marker("ml:summary:marker"),
        FormalMethodAssetVersionRef::new("ml:version:1"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        DownstreamConsumptionBoundaryRef::new("ml:boundary:1"),
    )
}

fn sample_material() -> MethodAssetConsumptionMaterial {
    MethodAssetConsumptionMaterial::from_formal_version(
        MethodAssetConsumptionMaterialRef::new("ml:material:1"),
        FormalMethodAssetVersionRef::new("ml:version:1"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        DownstreamConsumptionBoundaryRef::new("ml:boundary:1"),
        ConsumptionContextRef::new("ml:context:1"),
        sample_summary(),
        MethodAssetConsumptionMaterialCursorRef::new("ml:cursor:1"),
    )
}

fn sample_boundary() -> DownstreamConsumptionBoundary {
    DownstreamConsumptionBoundary::for_consumption_context(
        DownstreamConsumptionBoundaryRef::new("ml:boundary:1"),
        ConsumptionContextRef::new("ml:context:1"),
        FormalVersionRequirement::new(
            FormalVersionRequiredState::ActiveOnly,
            marker(
                MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersion,
                "ml:requirement:1",
            ),
        ),
        MethodAssetAllowedUseKindSet::from_kinds([
            MethodAssetAllowedUseKind::Read,
            MethodAssetAllowedUseKind::Reference,
        ]),
        DownstreamForbiddenWriteKindSet::from_kinds([
            DownstreamForbiddenWriteKind::DefinitionTruth,
            DownstreamForbiddenWriteKind::FormalVersionTruth,
        ]),
        MethodAssetConsumptionMaterialScopeRef::new("ml:scope:1"),
        ConsumptionBoundaryReasonRef::new(marker(
            MethodLibraryTypedBoundaryRefKind::DownstreamConsumptionBoundary,
            "ml:boundary:reason:1",
        )),
    )
    .expect("boundary should be valid")
}

fn sample_guard() -> DefinitionUseBoundaryGuard {
    DefinitionUseBoundaryGuard::protect_formal_consumption(
        DefinitionUseBoundaryGuardRef::new("ml:guard:1"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        FormalMethodAssetVersionRef::new("ml:version:1"),
        DownstreamConsumptionBoundaryRef::new("ml:boundary:1"),
        ConsumptionContextRef::new("ml:context:1"),
        DefinitionUseGuardReasonRef::new(marker(
            MethodLibraryTypedBoundaryRefKind::DefinitionUseBoundaryGuard,
            "ml:guard:reason:1",
        )),
    )
}

#[test]
fn material_factory_initializes_prepared_without_downstream_truth() {
    let material = sample_material();
    assert_eq!(
        material.material_state,
        MethodAssetConsumptionMaterialState::Prepared
    );
    assert_eq!(material.availability_marker, None);
    assert_eq!(material.definition_ref.as_public_ref(), "ml:def:1");
    assert_eq!(material.formal_version_ref.as_public_ref(), "ml:version:1");
}

#[test]
fn material_applies_copy_only_availability_marker() {
    let mut material = sample_material();
    let marker = MethodAssetConsumptionAvailabilityMarker::new(
        marker(
            MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterial,
            "ml:availability:1",
        ),
        MethodAssetConsumptionAvailabilityTarget::Constrained,
        MethodAssetConsumptionAvailabilityMarkerSource::DownstreamConsumptionBoundaryGuard,
        marker(
            MethodLibraryTypedBoundaryRefKind::DownstreamConsumptionBoundary,
            "ml:availability:source:1",
        ),
        Some(marker(
            MethodLibraryTypedBoundaryRefKind::DefinitionUseBoundaryGuard,
            "ml:availability:reason:1",
        )),
    );

    material.apply_availability_marker(marker.clone());

    assert_eq!(
        material.material_state,
        MethodAssetConsumptionMaterialState::Constrained
    );
    assert_eq!(material.availability_marker, Some(marker));
}

#[test]
fn material_mark_stale_preserves_copy_only_marker_shape() {
    let mut material = sample_material();
    material.apply_availability_marker(MethodAssetConsumptionAvailabilityMarker::new(
        marker(
            MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterial,
            "ml:availability:seed",
        ),
        MethodAssetConsumptionAvailabilityTarget::Ready,
        MethodAssetConsumptionAvailabilityMarkerSource::AvailabilityResolver,
        marker(
            MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterialCursor,
            "ml:availability:seed:source",
        ),
        Some(marker(
            MethodLibraryTypedBoundaryRefKind::DefinitionUseBoundaryGuard,
            "ml:availability:seed:reason",
        )),
    ));

    material
        .mark_stale(marker(
            MethodLibraryTypedBoundaryRefKind::DefinitionUseBoundaryGuard,
            "ml:stale:reason:1",
        ))
        .expect("stale marker should be accepted");

    let availability_marker = material
        .availability_marker
        .expect("stale transition should store a marker");
    assert_eq!(
        material.material_state,
        MethodAssetConsumptionMaterialState::Stale
    );
    assert_eq!(
        availability_marker.target_state,
        MethodAssetConsumptionAvailabilityTarget::Stale
    );
    assert_eq!(
        availability_marker.source_kind,
        MethodAssetConsumptionAvailabilityMarkerSource::AvailabilityResolver
    );
    assert_eq!(
        availability_marker.source_marker_ref,
        marker(
            MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterialCursor,
            "ml:availability:seed:source",
        )
    );
}

#[test]
fn material_mark_stale_accepts_boundary_reason_when_no_marker_was_loaded() {
    let mut material = sample_material();
    material
        .mark_stale(marker(
            MethodLibraryTypedBoundaryRefKind::DownstreamConsumptionBoundary,
            "ml:boundary:reason:stale",
        ))
        .expect("boundary marker should close the stale helper source");

    let availability_marker = material
        .availability_marker
        .expect("stale transition should store a marker");
    assert_eq!(
        availability_marker.source_kind,
        MethodAssetConsumptionAvailabilityMarkerSource::DownstreamConsumptionBoundaryGuard
    );
    assert_eq!(
        availability_marker.source_marker_ref,
        marker(
            MethodLibraryTypedBoundaryRefKind::DownstreamConsumptionBoundary,
            "ml:boundary:reason:stale",
        )
    );
}

#[test]
fn material_mark_stale_rejects_missing_formal_source_closure() {
    let mut material = sample_material();
    let error = material
        .mark_stale(marker(
            MethodLibraryTypedBoundaryRefKind::DefinitionUseBoundaryGuard,
            "ml:stale:reason:unclosed",
        ))
        .expect_err("missing stale marker source must not be synthesized");
    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::MissingRequiredTypedInput
    );
}

#[test]
fn material_invariants_reject_wrong_formal_version_context_or_boundary() {
    let material = sample_material();

    assert!(
        material
            .assert_from_formal_version(&FormalMethodAssetVersionRef::new("ml:version:1"))
            .is_ok()
    );
    assert!(
        material
            .assert_context(&ConsumptionContextRef::new("ml:context:1"))
            .is_ok()
    );
    assert!(
        material
            .assert_boundary(&DownstreamConsumptionBoundaryRef::new("ml:boundary:1"))
            .is_ok()
    );

    let version_error = material
        .assert_from_formal_version(&FormalMethodAssetVersionRef::new("ml:version:2"))
        .expect_err("version drift must be rejected");
    assert_eq!(
        version_error.kind(),
        MethodLibraryDomainErrorKind::InvariantViolation
    );
}

#[test]
fn guard_initializes_monitoring_and_records_safe_violation() {
    let mut guard = sample_guard();
    assert_eq!(
        guard.guard_state,
        DefinitionUseBoundaryGuardState::Monitoring
    );

    guard
        .mark_violation(marker(
            MethodLibraryTypedBoundaryRefKind::DefinitionUseBoundaryGuard,
            "ml:violation:1",
        ))
        .expect("safe violation marker should be accepted");
    assert_eq!(
        guard.guard_state,
        DefinitionUseBoundaryGuardState::ViolationRecorded
    );
}

#[test]
fn guard_rejects_or_manual_reviews_writeback_by_attempt_kind() {
    let mut rejected = sample_guard();
    rejected
        .reject_downstream_definition_write(
            typed_ref(
                MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
                "ml:write:def:1",
            ),
            DefinitionUseGuardReasonRef::new(marker(
                MethodLibraryTypedBoundaryRefKind::DefinitionUseBoundaryGuard,
                "ml:reject:reason:1",
            )),
        )
        .expect("definition writeback should become rejected");
    assert_eq!(
        rejected.guard_state,
        DefinitionUseBoundaryGuardState::RejectedCandidate
    );

    let mut manual = sample_guard();
    manual
        .reject_downstream_definition_write(
            typed_ref(
                MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterial,
                "ml:write:material:1",
            ),
            DefinitionUseGuardReasonRef::new(marker(
                MethodLibraryTypedBoundaryRefKind::DefinitionUseBoundaryGuard,
                "ml:manual:reason:1",
            )),
        )
        .expect("non-definition writeback should require manual review");
    assert_eq!(
        manual.guard_state,
        DefinitionUseBoundaryGuardState::ManualReviewRequired
    );
}

#[test]
fn boundary_registration_requires_non_empty_allowed_and_forbidden_sets() {
    let error = DownstreamConsumptionBoundary::for_consumption_context(
        DownstreamConsumptionBoundaryRef::new("ml:boundary:1"),
        ConsumptionContextRef::new("ml:context:1"),
        FormalVersionRequirement::new(
            FormalVersionRequiredState::ActiveOnly,
            marker(
                MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersion,
                "ml:requirement:1",
            ),
        ),
        MethodAssetAllowedUseKindSet::new(),
        DownstreamForbiddenWriteKindSet::from_kinds([
            DownstreamForbiddenWriteKind::DefinitionTruth,
        ]),
        MethodAssetConsumptionMaterialScopeRef::new("ml:scope:1"),
        ConsumptionBoundaryReasonRef::new(marker(
            MethodLibraryTypedBoundaryRefKind::DownstreamConsumptionBoundary,
            "ml:boundary:reason:1",
        )),
    )
    .expect_err("registered boundary must require explicit allowed kinds");

    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::InvariantViolation
    );
}

#[test]
fn boundary_constraints_and_writeback_rejection_stay_body_free() {
    let mut boundary = sample_boundary();
    assert_eq!(
        boundary.boundary_state,
        DownstreamConsumptionBoundaryState::Registered
    );

    assert!(
        boundary
            .assert_use_kind_allowed(MethodAssetAllowedUseKind::Read)
            .is_ok()
    );
    let disallowed = boundary
        .assert_use_kind_allowed(MethodAssetAllowedUseKind::Distribute)
        .expect_err("disallowed use kind must be rejected");
    assert_eq!(
        disallowed.kind(),
        MethodLibraryDomainErrorKind::PolicyRejected
    );

    let writeback_error = boundary
        .reject_forbidden_write(&DownstreamForbiddenWriteKindSet::from_kinds([
            DownstreamForbiddenWriteKind::DefinitionTruth,
        ]))
        .expect_err("definition truth writeback must be rejected");
    assert_eq!(
        writeback_error.kind(),
        MethodLibraryDomainErrorKind::PolicyRejected
    );

    boundary
        .scope_limited(ConsumptionBoundaryReasonRef::new(marker(
            MethodLibraryTypedBoundaryRefKind::DownstreamConsumptionBoundary,
            "ml:boundary:reason:constrained",
        )))
        .expect("scope-limited transition should succeed");
    assert_eq!(
        boundary.boundary_state,
        DownstreamConsumptionBoundaryState::Constrained
    );

    boundary
        .unavailable(ConsumptionBoundaryReasonRef::new(marker(
            MethodLibraryTypedBoundaryRefKind::DownstreamConsumptionBoundary,
            "ml:boundary:reason:unavailable",
        )))
        .expect("unavailable transition should succeed");
    assert_eq!(
        boundary.boundary_state,
        DownstreamConsumptionBoundaryState::Unavailable
    );
}

#[test]
fn active_only_requirement_blocks_superseded_and_retired_versions() {
    let requirement = FormalVersionRequirement::new(
        FormalVersionRequiredState::ActiveOnly,
        marker(
            MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersion,
            "ml:requirement:1",
        ),
    );
    assert!(
        requirement
            .required_state
            .accepts(FormalMethodAssetVersionState::Active)
    );
    assert!(
        !requirement
            .required_state
            .accepts(FormalMethodAssetVersionState::Superseded)
    );
    assert!(
        !requirement
            .required_state
            .accepts(FormalMethodAssetVersionState::Retired)
    );
}
