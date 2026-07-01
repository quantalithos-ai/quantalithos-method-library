use method_library_contracts::{
    CatalogScopeRef, ExternalSourceSummaryRef, ExternalSourceSummaryRefSet,
    MethodAssetApplicabilitySummary, MethodAssetCatalogEntryRef, MethodAssetCatalogEntryRefSet,
    MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};

fn typed_ref(
    kind: MethodLibraryTypedBoundaryRefKind,
    value: &str,
) -> MethodLibraryTypedBoundaryRef {
    MethodLibraryTypedBoundaryRef::from_verified_source(kind, value)
}

#[test]
fn definition_catalog_ref_kinds_are_stable() {
    let catalog_kind = MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntry;
    let encoded = serde_json::to_string(&catalog_kind).expect("catalog kind should serialize");
    assert_eq!(encoded, "\"method_asset_catalog_entry\"");

    let summary_kind = MethodLibraryTypedBoundaryRefKind::ExternalSourceSummary;
    let encoded = serde_json::to_string(&summary_kind).expect("summary kind should serialize");
    assert_eq!(encoded, "\"external_source_summary\"");
}

#[test]
fn named_definition_catalog_wrappers_reject_wrong_kinds() {
    let wrong_catalog = typed_ref(
        MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
        "x",
    );
    let catalog_error = MethodAssetCatalogEntryRef::try_from(wrong_catalog)
        .expect_err("catalog wrapper must reject a mismatched kind");
    assert_eq!(
        catalog_error.expected_kind(),
        MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntry
    );
    assert_eq!(
        catalog_error.actual_kind(),
        MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition
    );

    let wrong_summary = typed_ref(
        MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntry,
        "y",
    );
    let summary_error = ExternalSourceSummaryRef::try_from(wrong_summary)
        .expect_err("summary wrapper must reject a mismatched kind");
    assert_eq!(
        summary_error.expected_kind(),
        MethodLibraryTypedBoundaryRefKind::ExternalSourceSummary
    );
    assert_eq!(
        summary_error.actual_kind(),
        MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntry
    );
}

#[test]
fn ref_sets_dedup_and_preserve_first_seen_order() {
    let first_summary = ExternalSourceSummaryRef::new("ml:summary:1");
    let second_summary = ExternalSourceSummaryRef::new("ml:summary:2");
    let summary_set = ExternalSourceSummaryRefSet::from_refs([
        first_summary.clone(),
        second_summary.clone(),
        first_summary.clone(),
    ]);
    assert_eq!(summary_set.refs, vec![first_summary, second_summary]);

    let first_catalog = MethodAssetCatalogEntryRef::new("ml:catalog:1");
    let second_catalog = MethodAssetCatalogEntryRef::new("ml:catalog:2");
    let catalog_set = MethodAssetCatalogEntryRefSet::from_refs([
        first_catalog.clone(),
        second_catalog.clone(),
        first_catalog.clone(),
    ]);
    assert_eq!(catalog_set.refs, vec![first_catalog, second_catalog]);
}

#[test]
fn applicability_summary_dedups_context_refs_in_first_seen_order() {
    let scope_ref = CatalogScopeRef::new("ml:scope:1");
    let marker = MethodLibrarySafeMarker::boundary(typed_ref(
        MethodLibraryTypedBoundaryRefKind::CatalogScope,
        "ml:marker:scope",
    ));
    let first = typed_ref(
        MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
        "ml:ctx:1",
    );
    let second = typed_ref(
        MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
        "ml:ctx:2",
    );

    let summary = MethodAssetApplicabilitySummary::new(
        scope_ref,
        marker,
        [first.clone(), second.clone(), first.clone()],
    );

    assert_eq!(summary.applicable_context_refs, vec![first, second]);
}
