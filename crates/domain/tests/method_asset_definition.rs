use method_library_contracts::{
    CatalogScopeRef, ExternalSourceSummaryRef, ExternalSourceSummaryRefSet,
    MethodAssetApplicabilitySummary, MethodAssetCatalogClassification, MethodAssetCatalogEntryRef,
    MethodAssetCatalogEntryRefSet, MethodAssetCatalogEntryStatus, MethodAssetDefinitionKind,
    MethodAssetDefinitionRef, MethodAssetDefinitionSummary, MethodAssetIdentityKey,
    MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};
use method_library_domain::{
    MethodAssetCatalogEntry, MethodAssetDefinition, MethodLibraryDomainErrorKind,
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
    MethodAssetDefinition {
        definition_ref: MethodAssetDefinitionRef::new("ml:def:1"),
        definition_kind: MethodAssetDefinitionKind::SpemMethodContent,
        identity_key: sample_identity_key(),
        definition_summary: sample_definition_summary(marker),
        source_summary_refs: ExternalSourceSummaryRefSet::new(),
        catalog_entry_refs: MethodAssetCatalogEntryRefSet::new(),
    }
}

fn sample_catalog_entry(status: MethodAssetCatalogEntryStatus) -> MethodAssetCatalogEntry {
    let catalog_scope_ref = CatalogScopeRef::new("ml:scope:1");

    MethodAssetCatalogEntry {
        catalog_entry_ref: MethodAssetCatalogEntryRef::new("ml:catalog:1"),
        definition_ref: MethodAssetDefinitionRef::new("ml:def:1"),
        catalog_scope_ref: catalog_scope_ref.clone(),
        catalog_classification: MethodAssetCatalogClassification::new(
            MethodAssetDefinitionKind::SpemMethodContent,
            catalog_scope_ref.clone(),
            boundary_marker("ml:classification:1"),
        ),
        applicability_summary: MethodAssetApplicabilitySummary::new(
            catalog_scope_ref,
            boundary_marker("ml:applicability:1"),
            [typed_ref(
                MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
                "ml:ctx:1",
            )],
        ),
        catalog_status: status,
    }
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
fn catalog_entry_rejects_reclassification_to_a_different_scope() {
    let mut entry = sample_catalog_entry(MethodAssetCatalogEntryStatus::Visible);
    let different_scope = CatalogScopeRef::new("ml:scope:2");

    let error = entry
        .update_classification(MethodAssetCatalogClassification::new(
            MethodAssetDefinitionKind::SpemMethodContent,
            different_scope,
            boundary_marker("ml:classification:2"),
        ))
        .expect_err("classification scope must remain aligned with the entry scope");

    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::InvariantViolation
    );
}
