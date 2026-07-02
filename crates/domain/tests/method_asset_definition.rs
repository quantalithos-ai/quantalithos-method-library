use method_library_contracts::{
    CatalogScopeRef, ExternalSourceSummaryRef, ExternalSourceSummaryRefSet,
    MethodAssetApplicabilitySummary, MethodAssetCatalogClassification, MethodAssetCatalogEntryRef,
    MethodAssetCatalogEntryStatus, MethodAssetDefinitionKind, MethodAssetDefinitionRef,
    MethodAssetDefinitionSummary, MethodAssetIdentityKey, MethodLibrarySafeMarker,
    MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};
use method_library_domain::{
    MethodAssetCatalogEntry, MethodAssetDefinition, MethodAssetDefinitionLifecycle,
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

fn sample_identity_key() -> MethodAssetIdentityKey {
    MethodAssetIdentityKey::new(
        MethodAssetDefinitionKind::SpemMethodContent,
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            "ml:ns:1",
        ),
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:anchor:1",
        ),
        CatalogScopeRef::new("ml:scope:1"),
    )
}

fn sample_definition_summary(marker: MethodLibrarySafeMarker) -> MethodAssetDefinitionSummary {
    MethodAssetDefinitionSummary::new(
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:summary:1",
        ),
        MethodAssetDefinitionKind::SpemMethodContent,
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:title:1",
        ),
        Some(typed_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:description:1",
        )),
        marker,
    )
}

fn sample_definition(marker: MethodLibrarySafeMarker) -> MethodAssetDefinition {
    MethodAssetDefinition::create(
        MethodAssetDefinitionRef::new("ml:def:1"),
        sample_identity_key(),
        sample_definition_summary(marker),
    )
}

fn sample_catalog_entry(status: MethodAssetCatalogEntryStatus) -> MethodAssetCatalogEntry {
    let catalog_scope_ref = CatalogScopeRef::new("ml:scope:1");
    let mut entry = MethodAssetCatalogEntry::create_for_definition(
        MethodAssetCatalogEntryRef::new("ml:catalog:1"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        catalog_scope_ref.clone(),
        MethodAssetCatalogClassification::new(
            MethodAssetDefinitionKind::SpemMethodContent,
            catalog_scope_ref.clone(),
            boundary_marker("ml:classification:1"),
        ),
        MethodAssetApplicabilitySummary::new(
            catalog_scope_ref,
            boundary_marker("ml:applicability:1"),
            [typed_ref(
                MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
                "ml:ctx:1",
            )],
        ),
    )
    .expect("catalog create should be valid");
    entry.catalog_status = status;
    entry
}

#[test]
fn definition_body_free_requires_no_body_marker() {
    let definition = sample_definition(no_body_marker("ml:marker:no-body"));
    assert!(definition.assert_body_free().is_ok());

    let rejected = sample_definition(boundary_marker("ml:marker:boundary"))
        .assert_body_free()
        .expect_err("body-free validation must reject non-no-body markers");
    assert_eq!(
        rejected.kind(),
        MethodLibraryDomainErrorKind::BodyFreeBoundaryViolation
    );
}

#[test]
fn definition_create_initializes_active_lifecycle() {
    let definition = sample_definition(no_body_marker("ml:marker:no-body"));
    assert_eq!(
        definition.definition_lifecycle,
        MethodAssetDefinitionLifecycle::Active
    );
}

#[test]
fn definition_adjust_preserves_active_and_replaces_summary_and_sources() {
    let mut definition = sample_definition(no_body_marker("ml:marker:no-body"));
    let replacement_summary = MethodAssetDefinitionSummary::new(
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:summary:replacement",
        ),
        MethodAssetDefinitionKind::SpemMethodContent,
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            "ml:title:replacement",
        ),
        None,
        no_body_marker("ml:marker:replacement"),
    );
    let replacement_sources =
        ExternalSourceSummaryRefSet::from_refs([ExternalSourceSummaryRef::new("ml:summary:src:1")]);

    definition
        .apply_adjustment(replacement_summary.clone(), replacement_sources.clone())
        .expect("active definitions can be adjusted");

    assert_eq!(definition.definition_summary, replacement_summary);
    assert_eq!(definition.source_summary_refs, replacement_sources);
    assert_eq!(
        definition.definition_lifecycle,
        MethodAssetDefinitionLifecycle::Active
    );
}

#[test]
fn retired_definition_rejects_adjust_and_retire() {
    let mut definition = sample_definition(no_body_marker("ml:marker:no-body"));
    definition
        .mark_retired(boundary_marker("ml:reason:retire"))
        .expect("active definition can retire");
    assert_eq!(
        definition.definition_lifecycle,
        MethodAssetDefinitionLifecycle::Retired
    );

    let adjust_error = definition
        .apply_adjustment(
            sample_definition_summary(no_body_marker("ml:marker:replacement")),
            ExternalSourceSummaryRefSet::new(),
        )
        .expect_err("retired definitions must reject adjust");
    assert_eq!(
        adjust_error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );

    let retire_error = definition
        .mark_retired(boundary_marker("ml:reason:retire:again"))
        .expect_err("retired definitions must stay terminal");
    assert_eq!(
        retire_error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );
}

#[test]
fn definition_links_catalog_entries_and_source_summaries_without_duplicates() {
    let mut definition = sample_definition(no_body_marker("ml:marker:no-body"));
    let catalog_ref = MethodAssetCatalogEntryRef::new("ml:catalog:1");
    let summary_ref = ExternalSourceSummaryRef::new("ml:summary:1");

    definition.link_catalog_entry(catalog_ref.clone());
    definition.link_catalog_entry(catalog_ref.clone());
    definition.accept_source_summary(summary_ref.clone());
    definition.accept_source_summary(summary_ref.clone());

    assert_eq!(definition.catalog_entry_refs.refs, vec![catalog_ref]);
    assert_eq!(definition.source_summary_refs.refs, vec![summary_ref]);
}

#[test]
fn catalog_create_requires_scope_consistency_and_sets_visible() {
    let catalog_scope_ref = CatalogScopeRef::new("ml:scope:1");
    let entry = MethodAssetCatalogEntry::create_for_definition(
        MethodAssetCatalogEntryRef::new("ml:catalog:1"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        catalog_scope_ref.clone(),
        MethodAssetCatalogClassification::new(
            MethodAssetDefinitionKind::SpemMethodContent,
            catalog_scope_ref.clone(),
            boundary_marker("ml:classification:1"),
        ),
        MethodAssetApplicabilitySummary::new(
            catalog_scope_ref.clone(),
            boundary_marker("ml:applicability:1"),
            [],
        ),
    )
    .expect("scope-aligned create should succeed");
    assert_eq!(entry.catalog_scope_ref, catalog_scope_ref);
    assert_eq!(entry.catalog_status, MethodAssetCatalogEntryStatus::Visible);

    let error = MethodAssetCatalogEntry::create_for_definition(
        MethodAssetCatalogEntryRef::new("ml:catalog:2"),
        MethodAssetDefinitionRef::new("ml:def:1"),
        CatalogScopeRef::new("ml:scope:1"),
        MethodAssetCatalogClassification::new(
            MethodAssetDefinitionKind::SpemMethodContent,
            CatalogScopeRef::new("ml:scope:2"),
            boundary_marker("ml:classification:2"),
        ),
        MethodAssetApplicabilitySummary::new(
            CatalogScopeRef::new("ml:scope:1"),
            boundary_marker("ml:applicability:2"),
            [],
        ),
    )
    .expect_err("mismatched scope must be rejected");
    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::InvariantViolation
    );
}

#[test]
fn catalog_reclassify_updates_scope_classification_and_applicability_together() {
    let mut entry = sample_catalog_entry(MethodAssetCatalogEntryStatus::Visible);
    let new_scope = CatalogScopeRef::new("ml:scope:2");
    let new_classification = MethodAssetCatalogClassification::new(
        MethodAssetDefinitionKind::SpemMethodContent,
        new_scope.clone(),
        boundary_marker("ml:classification:2"),
    );
    let new_applicability = MethodAssetApplicabilitySummary::new(
        new_scope.clone(),
        boundary_marker("ml:applicability:2"),
        [typed_ref(
            MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
            "ml:ctx:2",
        )],
    );

    entry
        .reclassify(new_classification.clone(), new_applicability.clone())
        .expect("visible entry can be reclassified");

    assert_eq!(entry.catalog_scope_ref, new_scope);
    assert_eq!(entry.catalog_classification, new_classification);
    assert_eq!(entry.applicability_summary, new_applicability);
    assert_eq!(entry.catalog_status, MethodAssetCatalogEntryStatus::Visible);
}

#[test]
fn catalog_reclassify_rejects_non_visible_or_scope_mismatch() {
    let error = sample_catalog_entry(MethodAssetCatalogEntryStatus::Retired)
        .reclassify(
            MethodAssetCatalogClassification::new(
                MethodAssetDefinitionKind::SpemMethodContent,
                CatalogScopeRef::new("ml:scope:1"),
                boundary_marker("ml:classification:2"),
            ),
            MethodAssetApplicabilitySummary::new(
                CatalogScopeRef::new("ml:scope:1"),
                boundary_marker("ml:applicability:2"),
                [],
            ),
        )
        .expect_err("retired entries must not reclassify");
    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );

    let mut visible = sample_catalog_entry(MethodAssetCatalogEntryStatus::Visible);
    let mismatch_error = visible
        .reclassify(
            MethodAssetCatalogClassification::new(
                MethodAssetDefinitionKind::SpemMethodContent,
                CatalogScopeRef::new("ml:scope:2"),
                boundary_marker("ml:classification:3"),
            ),
            MethodAssetApplicabilitySummary::new(
                CatalogScopeRef::new("ml:scope:3"),
                boundary_marker("ml:applicability:3"),
                [],
            ),
        )
        .expect_err("scope mismatch must be rejected");
    assert_eq!(
        mismatch_error.kind(),
        MethodLibraryDomainErrorKind::InvariantViolation
    );
}

#[test]
fn catalog_entry_deprecation_sets_status_without_rewriting_identity() {
    let mut entry = sample_catalog_entry(MethodAssetCatalogEntryStatus::Visible);
    let original_definition_ref = entry.definition_ref.clone();
    let original_catalog_ref = entry.catalog_entry_ref.clone();
    let original_classification = entry.catalog_classification.clone();

    entry
        .mark_deprecated(boundary_marker("ml:reason:deprecated"))
        .expect("safe marker should allow deprecation");

    assert_eq!(
        entry.catalog_status,
        MethodAssetCatalogEntryStatus::Deprecated
    );
    assert_eq!(entry.definition_ref, original_definition_ref);
    assert_eq!(entry.catalog_entry_ref, original_catalog_ref);
    assert_eq!(entry.catalog_classification, original_classification);
}

#[test]
fn retired_catalog_entry_rejects_deprecation() {
    let error = sample_catalog_entry(MethodAssetCatalogEntryStatus::Retired)
        .mark_deprecated(boundary_marker("ml:reason:deprecated"))
        .expect_err("retired entries must stay terminal");

    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );
}

#[test]
fn catalog_retire_requires_visible_and_preserves_identity() {
    let mut entry = sample_catalog_entry(MethodAssetCatalogEntryStatus::Visible);
    let original_definition_ref = entry.definition_ref.clone();
    let original_catalog_ref = entry.catalog_entry_ref.clone();

    entry
        .mark_retired(boundary_marker("ml:reason:retired"))
        .expect("visible entry can retire");
    assert_eq!(entry.catalog_status, MethodAssetCatalogEntryStatus::Retired);
    assert_eq!(entry.definition_ref, original_definition_ref);
    assert_eq!(entry.catalog_entry_ref, original_catalog_ref);

    let error = entry
        .mark_retired(boundary_marker("ml:reason:retired:again"))
        .expect_err("retired entries stay terminal");
    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );
}
