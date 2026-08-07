use method_library_contracts::{
    ConsumptionImpactKind, ConsumptionImpactSafeSummary, ConsumptionImpactSourceRef,
    ConsumptionImpactSummaryRef, ConsumptionImpactSummaryState, MethodAssetAuditCursorRef,
    MethodAssetAuditEntryRef, MethodAssetAuditEntryRefSet, MethodAssetAuditTrailRef,
    MethodAssetAuditTrailState, MethodAssetEvidenceLineageRef, MethodAssetEvidenceLineageRefSet,
    MethodAssetEvidenceLineageState, MethodAssetEvidenceLineageSummary, MethodAssetSafeReasonRef,
    MethodAssetTraceCursorRef, MethodAssetTraceFreshnessMarkerRef, MethodAssetTraceMaterialRef,
    MethodAssetTraceMaterialRefSet, MethodAssetTraceMaterialState, MethodAssetTraceSourceRef,
    MethodAssetTraceSourceRefSet, MethodAssetTraceSummary, MethodLibrarySafeMarker,
    MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind, TraceSubjectRef,
};

fn typed_ref(
    kind: MethodLibraryTypedBoundaryRefKind,
    value: &str,
) -> MethodLibraryTypedBoundaryRef {
    MethodLibraryTypedBoundaryRef::from_verified_source(kind, value)
}

fn marker(value: &str) -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::boundary(typed_ref(
        MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
        value,
    ))
}

macro_rules! assert_wrapper_kind_and_rejection {
    ($wrapper:ty, $kind:expr, $value:expr) => {{
        let wrapper = <$wrapper>::new($value);
        assert_eq!(wrapper.as_typed_ref().kind(), $kind);
        assert_eq!(<$wrapper>::expected_kind(), $kind);
        let serialized = serde_json::to_string(&wrapper).expect("wrapper should serialize");
        let roundtrip: $wrapper =
            serde_json::from_str(&serialized).expect("wrapper should deserialize");
        assert_eq!(roundtrip, wrapper);

        let error = <$wrapper>::try_from(typed_ref(
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
            "ml:wrong-kind",
        ))
        .expect_err("wrapper must reject a mismatched kind");
        assert_eq!(error.expected_kind(), $kind);
        assert_eq!(
            error.actual_kind(),
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition
        );
    }};
}

#[test]
fn trace_audit_named_refs_use_exact_kinds_and_reject_wrong_kinds() {
    assert_wrapper_kind_and_rejection!(
        TraceSubjectRef,
        MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
        "ml:subject:1"
    );
    assert_wrapper_kind_and_rejection!(
        ConsumptionImpactSourceRef,
        MethodLibraryTypedBoundaryRefKind::ConsumptionImpactSourceRef,
        "ml:impact-source:1"
    );
    assert_wrapper_kind_and_rejection!(
        MethodAssetTraceMaterialRef,
        MethodLibraryTypedBoundaryRefKind::MethodAssetTraceMaterial,
        "ml:trace:1"
    );
    assert_wrapper_kind_and_rejection!(
        MethodAssetTraceCursorRef,
        MethodLibraryTypedBoundaryRefKind::MethodAssetTraceCursor,
        "ml:trace-cursor:1"
    );
    assert_wrapper_kind_and_rejection!(
        MethodAssetTraceFreshnessMarkerRef,
        MethodLibraryTypedBoundaryRefKind::MethodAssetTraceFreshnessMarker,
        "ml:freshness:1"
    );
    assert_wrapper_kind_and_rejection!(
        ConsumptionImpactSummaryRef,
        MethodLibraryTypedBoundaryRefKind::ConsumptionImpactSummary,
        "ml:impact:1"
    );
    assert_wrapper_kind_and_rejection!(
        MethodAssetAuditTrailRef,
        MethodLibraryTypedBoundaryRefKind::MethodAssetAuditTrail,
        "ml:audit:1"
    );
    assert_wrapper_kind_and_rejection!(
        MethodAssetAuditEntryRef,
        MethodLibraryTypedBoundaryRefKind::MethodAssetAuditEntry,
        "ml:audit-entry:1"
    );
    assert_wrapper_kind_and_rejection!(
        MethodAssetAuditCursorRef,
        MethodLibraryTypedBoundaryRefKind::MethodAssetAuditCursor,
        "ml:audit-cursor:1"
    );
    assert_wrapper_kind_and_rejection!(
        MethodAssetEvidenceLineageRef,
        MethodLibraryTypedBoundaryRefKind::MethodAssetEvidenceLineage,
        "ml:lineage:1"
    );
}

#[test]
fn trace_source_accepts_exactly_the_six_formal_source_kinds() {
    let allowed = [
        MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
        MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntry,
        MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersion,
        MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterial,
        MethodLibraryTypedBoundaryRefKind::MethodAssetRelation,
        MethodLibraryTypedBoundaryRefKind::ExternalSourceSummary,
    ];

    for (index, kind) in allowed.into_iter().enumerate() {
        let public_ref = format!("ml:source:{index}");
        let source = MethodAssetTraceSourceRef::try_from(typed_ref(kind, &public_ref))
            .expect("formal source kind should be accepted");
        assert_eq!(source.as_typed_ref().kind(), kind);
        assert_eq!(source.as_public_ref(), public_ref);
        let serialized = serde_json::to_string(&source).expect("trace source should serialize");
        let roundtrip: MethodAssetTraceSourceRef =
            serde_json::from_str(&serialized).expect("trace source should deserialize");
        assert_eq!(roundtrip, source);
    }

    let error = MethodAssetTraceSourceRef::try_from(typed_ref(
        MethodLibraryTypedBoundaryRefKind::MethodAssetAuditTrail,
        "ml:unsupported-source:1",
    ))
    .expect_err("an audit trail is not one of the six trace source kinds");
    assert_eq!(
        error.actual_kind(),
        MethodLibraryTypedBoundaryRefKind::MethodAssetAuditTrail
    );
}

#[test]
fn trace_audit_ref_sets_dedup_and_preserve_first_seen_order() {
    let source_a = MethodAssetTraceSourceRef::try_from(typed_ref(
        MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
        "ml:def:1",
    ))
    .expect("definition source should be accepted");
    let source_b = MethodAssetTraceSourceRef::try_from(typed_ref(
        MethodLibraryTypedBoundaryRefKind::ExternalSourceSummary,
        "ml:external:1",
    ))
    .expect("external source should be accepted");
    let sources = MethodAssetTraceSourceRefSet::from_refs([
        source_a.clone(),
        source_b.clone(),
        source_a.clone(),
    ]);
    assert_eq!(sources.refs, vec![source_a, source_b]);

    let trace_a = MethodAssetTraceMaterialRef::new("ml:trace:1");
    let trace_b = MethodAssetTraceMaterialRef::new("ml:trace:2");
    let traces = MethodAssetTraceMaterialRefSet::from_refs([
        trace_a.clone(),
        trace_b.clone(),
        trace_a.clone(),
    ]);
    assert_eq!(traces.refs, vec![trace_a, trace_b]);

    let entry_a = MethodAssetAuditEntryRef::new("ml:entry:1");
    let entry_b = MethodAssetAuditEntryRef::new("ml:entry:2");
    let entries =
        MethodAssetAuditEntryRefSet::from_refs([entry_a.clone(), entry_b.clone(), entry_a.clone()]);
    assert_eq!(entries.refs, vec![entry_a, entry_b]);

    let lineage_a = MethodAssetEvidenceLineageRef::new("ml:lineage:1");
    let lineage_b = MethodAssetEvidenceLineageRef::new("ml:lineage:2");
    let lineages = MethodAssetEvidenceLineageRefSet::from_refs([
        lineage_a.clone(),
        lineage_b.clone(),
        lineage_a.clone(),
    ]);
    assert_eq!(lineages.refs, vec![lineage_a, lineage_b]);
}

#[test]
fn safe_reason_and_body_free_summaries_roundtrip_exact_fields() {
    let safe_reason = MethodAssetSafeReasonRef::new(marker("ml:reason:1"));
    assert_eq!(safe_reason.as_safe_marker(), &marker("ml:reason:1"));

    let reason_json = serde_json::to_value(&safe_reason).expect("safe reason should serialize");
    assert_eq!(
        reason_json
            .as_object()
            .expect("safe reason should be an object")
            .len(),
        1
    );
    assert!(reason_json.get("safe_marker").is_some());

    let trace_summary = MethodAssetTraceSummary {
        summary_marker_ref: marker("ml:trace-summary:1"),
        coverage_marker_ref: marker("ml:coverage:1"),
    };
    let impact_summary = ConsumptionImpactSafeSummary {
        summary_marker_ref: marker("ml:impact-summary:1"),
        disposition_marker_ref: Some(marker("ml:disposition:1")),
        safe_reason_ref: Some(safe_reason.clone()),
    };
    let lineage_summary = MethodAssetEvidenceLineageSummary {
        summary_marker_ref: marker("ml:lineage-summary:1"),
        safe_reason_ref: Some(safe_reason),
    };

    let trace_roundtrip: MethodAssetTraceSummary = serde_json::from_str(
        &serde_json::to_string(&trace_summary).expect("trace summary should serialize"),
    )
    .expect("trace summary should deserialize");
    let impact_roundtrip: ConsumptionImpactSafeSummary = serde_json::from_str(
        &serde_json::to_string(&impact_summary).expect("impact summary should serialize"),
    )
    .expect("impact summary should deserialize");
    let lineage_roundtrip: MethodAssetEvidenceLineageSummary = serde_json::from_str(
        &serde_json::to_string(&lineage_summary).expect("lineage summary should serialize"),
    )
    .expect("lineage summary should deserialize");

    assert_eq!(trace_roundtrip, trace_summary);
    assert_eq!(impact_roundtrip, impact_summary);
    assert_eq!(lineage_roundtrip, lineage_summary);
}

#[test]
fn trace_audit_state_and_impact_labels_are_exact() {
    let cases = [
        (
            serde_json::to_string(&MethodLibraryTypedBoundaryRefKind::MethodAssetTraceMaterial)
                .expect("trace kind should serialize"),
            "\"method_asset_trace_material\"",
        ),
        (
            serde_json::to_string(&MethodLibraryTypedBoundaryRefKind::MethodAssetTraceCursor)
                .expect("trace cursor kind should serialize"),
            "\"method_asset_trace_cursor\"",
        ),
        (
            serde_json::to_string(
                &MethodLibraryTypedBoundaryRefKind::MethodAssetTraceFreshnessMarker,
            )
            .expect("freshness kind should serialize"),
            "\"method_asset_trace_freshness_marker\"",
        ),
        (
            serde_json::to_string(&MethodLibraryTypedBoundaryRefKind::ConsumptionImpactSummary)
                .expect("impact kind should serialize"),
            "\"consumption_impact_summary\"",
        ),
        (
            serde_json::to_string(&MethodLibraryTypedBoundaryRefKind::MethodAssetAuditTrail)
                .expect("audit trail kind should serialize"),
            "\"method_asset_audit_trail\"",
        ),
        (
            serde_json::to_string(&MethodLibraryTypedBoundaryRefKind::MethodAssetAuditEntry)
                .expect("audit entry kind should serialize"),
            "\"method_asset_audit_entry\"",
        ),
        (
            serde_json::to_string(&MethodLibraryTypedBoundaryRefKind::MethodAssetAuditCursor)
                .expect("audit cursor kind should serialize"),
            "\"method_asset_audit_cursor\"",
        ),
        (
            serde_json::to_string(&MethodLibraryTypedBoundaryRefKind::MethodAssetEvidenceLineage)
                .expect("lineage kind should serialize"),
            "\"method_asset_evidence_lineage\"",
        ),
        (
            serde_json::to_string(&MethodAssetTraceMaterialState::Organized)
                .expect("trace state should serialize"),
            "\"organized\"",
        ),
        (
            serde_json::to_string(&MethodAssetTraceMaterialState::Partial)
                .expect("trace state should serialize"),
            "\"partial\"",
        ),
        (
            serde_json::to_string(&MethodAssetTraceMaterialState::Stale)
                .expect("trace state should serialize"),
            "\"stale\"",
        ),
        (
            serde_json::to_string(&MethodAssetTraceMaterialState::Unavailable)
                .expect("trace state should serialize"),
            "\"unavailable\"",
        ),
        (
            serde_json::to_string(&ConsumptionImpactKind::KnownImpact)
                .expect("impact kind should serialize"),
            "\"known_impact\"",
        ),
        (
            serde_json::to_string(&ConsumptionImpactKind::UnknownImpact)
                .expect("impact kind should serialize"),
            "\"unknown_impact\"",
        ),
        (
            serde_json::to_string(&ConsumptionImpactKind::PendingDownstreamSummary)
                .expect("impact kind should serialize"),
            "\"pending_downstream_summary\"",
        ),
        (
            serde_json::to_string(&ConsumptionImpactKind::NoKnownEffect)
                .expect("impact kind should serialize"),
            "\"no_known_effect\"",
        ),
        (
            serde_json::to_string(&ConsumptionImpactSummaryState::Current)
                .expect("impact state should serialize"),
            "\"current\"",
        ),
        (
            serde_json::to_string(&ConsumptionImpactSummaryState::DispositionMarked)
                .expect("impact state should serialize"),
            "\"disposition_marked\"",
        ),
        (
            serde_json::to_string(&ConsumptionImpactSummaryState::Superseded)
                .expect("impact state should serialize"),
            "\"superseded\"",
        ),
        (
            serde_json::to_string(&MethodAssetAuditTrailState::TrailOwnerPresent)
                .expect("audit state should serialize"),
            "\"trail_owner_present\"",
        ),
        (
            serde_json::to_string(&MethodAssetAuditTrailState::SafeEntryRefsAppended)
                .expect("audit state should serialize"),
            "\"safe_entry_refs_appended\"",
        ),
        (
            serde_json::to_string(&MethodAssetAuditTrailState::PartialAuditAvailable)
                .expect("audit state should serialize"),
            "\"partial_audit_available\"",
        ),
        (
            serde_json::to_string(&MethodAssetAuditTrailState::AuditUnavailable)
                .expect("audit state should serialize"),
            "\"audit_unavailable\"",
        ),
        (
            serde_json::to_string(&MethodAssetEvidenceLineageState::LineageLinked)
                .expect("lineage state should serialize"),
            "\"lineage_linked\"",
        ),
        (
            serde_json::to_string(&MethodAssetEvidenceLineageState::LineagePartial)
                .expect("lineage state should serialize"),
            "\"lineage_partial\"",
        ),
        (
            serde_json::to_string(&MethodAssetEvidenceLineageState::LineageUnavailable)
                .expect("lineage state should serialize"),
            "\"lineage_unavailable\"",
        ),
        (
            serde_json::to_string(&MethodAssetEvidenceLineageState::BodyCandidateRejected)
                .expect("lineage state should serialize"),
            "\"body_candidate_rejected\"",
        ),
    ];

    for (actual, expected) in cases {
        assert_eq!(actual, expected);
    }
}
