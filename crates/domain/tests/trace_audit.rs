use method_library_contracts::metadata::{ActorContext, ActorKind, ActorRef, RequestOrigin};
use method_library_contracts::{
    ConsumptionContextRef, ConsumptionImpactKind, ConsumptionImpactSafeSummary,
    ConsumptionImpactSourceRef, ConsumptionImpactSummaryRef, ConsumptionImpactSummaryState,
    ExternalSourceSummaryRef, ExternalSourceSummaryRefSet, FormalizationBasisSummaryRef,
    FormalizationBasisSummaryRefSet, MethodAssetAuditCursorRef, MethodAssetAuditEntryRef,
    MethodAssetAuditTrailRef, MethodAssetAuditTrailState, MethodAssetConsumptionMaterialRef,
    MethodAssetEvidenceLineageRef, MethodAssetEvidenceLineageState,
    MethodAssetEvidenceLineageSummary, MethodAssetSafeReasonRef, MethodAssetTraceCursorRef,
    MethodAssetTraceFreshnessMarkerRef, MethodAssetTraceMaterialRef, MethodAssetTraceMaterialState,
    MethodAssetTraceSourceRef, MethodAssetTraceSourceRefSet, MethodAssetTraceSummary,
    MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
    TraceSubjectRef,
};
use method_library_domain::{
    ConsumptionImpactSummary, MethodAssetAuditTrail, MethodAssetEvidenceLineage,
    MethodAssetTraceMaterial, MethodLibraryDomainErrorKind,
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

fn reason(value: &str) -> MethodAssetSafeReasonRef {
    MethodAssetSafeReasonRef::new(marker(value))
}

fn trace_source(kind: MethodLibraryTypedBoundaryRefKind, value: &str) -> MethodAssetTraceSourceRef {
    MethodAssetTraceSourceRef::try_from(typed_ref(kind, value))
        .expect("sample trace source kind should be accepted")
}

fn trace_summary() -> MethodAssetTraceSummary {
    MethodAssetTraceSummary {
        summary_marker_ref: marker("ml:trace-summary:1"),
        coverage_marker_ref: marker("ml:coverage:1"),
    }
}

fn sample_trace() -> MethodAssetTraceMaterial {
    MethodAssetTraceMaterial::from_source_objects(
        MethodAssetTraceMaterialRef::new("ml:trace:1"),
        TraceSubjectRef::new("ml:subject:1"),
        MethodAssetTraceSourceRefSet::from_refs([trace_source(
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
            "ml:def:1",
        )]),
        trace_summary(),
        MethodAssetTraceCursorRef::new("ml:trace-cursor:1"),
        MethodAssetTraceFreshnessMarkerRef::new("ml:freshness:1"),
        ExternalSourceSummaryRefSet::from_refs([ExternalSourceSummaryRef::new("ml:external:1")]),
    )
}

fn impact_safe_summary() -> ConsumptionImpactSafeSummary {
    ConsumptionImpactSafeSummary {
        summary_marker_ref: marker("ml:impact-summary:1"),
        disposition_marker_ref: None,
        safe_reason_ref: None,
    }
}

fn sample_impact(kind: ConsumptionImpactKind) -> ConsumptionImpactSummary {
    ConsumptionImpactSummary::register(
        ConsumptionImpactSummaryRef::new("ml:impact:1"),
        ConsumptionImpactSourceRef::new("ml:impact-source:1"),
        Some(MethodAssetConsumptionMaterialRef::new("ml:material:1")),
        Some(ConsumptionContextRef::new("ml:context:1")),
        kind,
        impact_safe_summary(),
        Some(MethodAssetTraceMaterialRef::new("ml:trace:1")),
    )
}

fn actor_context() -> ActorContext {
    ActorContext::new(
        ActorRef::new("actor:trace-audit", ActorKind::System),
        RequestOrigin::Operations,
    )
}

fn sample_audit() -> MethodAssetAuditTrail {
    MethodAssetAuditTrail::for_subject(
        MethodAssetAuditTrailRef::new("ml:audit:1"),
        TraceSubjectRef::new("ml:subject:1"),
        actor_context(),
        reason("ml:audit-reason:1"),
    )
}

fn lineage_summary() -> MethodAssetEvidenceLineageSummary {
    MethodAssetEvidenceLineageSummary {
        summary_marker_ref: marker("ml:lineage-summary:1"),
        safe_reason_ref: None,
    }
}

fn sample_lineage() -> MethodAssetEvidenceLineage {
    MethodAssetEvidenceLineage::from_external_and_basis(
        MethodAssetEvidenceLineageRef::new("ml:lineage:1"),
        TraceSubjectRef::new("ml:subject:1"),
        ExternalSourceSummaryRefSet::from_refs([ExternalSourceSummaryRef::new("ml:external:1")]),
        FormalizationBasisSummaryRefSet::from_refs([FormalizationBasisSummaryRef::new(
            "ml:basis:1",
        )]),
        lineage_summary(),
    )
}

#[test]
fn trace_factory_uses_source_presence_without_synthesizing_reason() {
    let organized = sample_trace();
    assert_eq!(organized.state, MethodAssetTraceMaterialState::Organized);
    assert_eq!(organized.safe_reason_ref, None);

    let partial = MethodAssetTraceMaterial::from_source_objects(
        MethodAssetTraceMaterialRef::new("ml:trace:empty"),
        TraceSubjectRef::new("ml:subject:empty"),
        MethodAssetTraceSourceRefSet::new(),
        trace_summary(),
        MethodAssetTraceCursorRef::new("ml:trace-cursor:empty"),
        MethodAssetTraceFreshnessMarkerRef::new("ml:freshness:empty"),
        ExternalSourceSummaryRefSet::new(),
    );
    assert_eq!(partial.state, MethodAssetTraceMaterialState::Partial);
    assert_eq!(partial.safe_reason_ref, None);
}

#[test]
fn trace_state_guard_requires_reason_and_preserves_source_truth() {
    let mut trace = sample_trace();
    let identity = trace.trace_material_ref.clone();
    let cursor = trace.source_cursor_ref.clone();
    let freshness = trace.freshness_marker_ref.clone();
    let sources = trace.source_object_refs.clone();

    trace
        .mark_state(
            MethodAssetTraceMaterialState::Stale,
            Some(reason("ml:trace-reason:stale")),
        )
        .expect("explicit safe reason should mark trace stale");
    assert_eq!(trace.state, MethodAssetTraceMaterialState::Stale);
    assert_eq!(trace.safe_reason_ref, Some(reason("ml:trace-reason:stale")));

    trace
        .mark_state(MethodAssetTraceMaterialState::Organized, None)
        .expect("non-empty formal sources should allow organization");
    assert_eq!(trace.state, MethodAssetTraceMaterialState::Organized);
    assert_eq!(trace.safe_reason_ref, None);
    assert_eq!(trace.trace_material_ref, identity);
    assert_eq!(trace.source_cursor_ref, cursor);
    assert_eq!(trace.freshness_marker_ref, freshness);
    assert_eq!(trace.source_object_refs, sources);

    let before = trace.clone();
    let error = trace
        .mark_state(MethodAssetTraceMaterialState::Unavailable, None)
        .expect_err("non-organized state requires an explicit safe reason");
    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::MissingRequiredTypedInput
    );
    assert_eq!(trace, before);
}

#[test]
fn trace_cannot_organize_an_empty_source_set() {
    let mut trace = MethodAssetTraceMaterial::from_source_objects(
        MethodAssetTraceMaterialRef::new("ml:trace:empty"),
        TraceSubjectRef::new("ml:subject:empty"),
        MethodAssetTraceSourceRefSet::new(),
        trace_summary(),
        MethodAssetTraceCursorRef::new("ml:trace-cursor:empty"),
        MethodAssetTraceFreshnessMarkerRef::new("ml:freshness:empty"),
        ExternalSourceSummaryRefSet::new(),
    );
    let before = trace.clone();

    let error = trace
        .mark_state(MethodAssetTraceMaterialState::Organized, None)
        .expect_err("empty source truth cannot be repaired by a state guard");
    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::InvariantViolation
    );
    assert_eq!(trace, before);
}

#[test]
fn impact_disposition_and_supersession_preserve_impact_kind() {
    for kind in [
        ConsumptionImpactKind::UnknownImpact,
        ConsumptionImpactKind::PendingDownstreamSummary,
    ] {
        let mut impact = sample_impact(kind);
        assert_eq!(impact.state, ConsumptionImpactSummaryState::Current);

        impact
            .mark_disposition(marker("ml:disposition:1"), reason("ml:impact-reason:1"))
            .expect("current impact should accept a safe disposition");
        assert_eq!(
            impact.state,
            ConsumptionImpactSummaryState::DispositionMarked
        );
        assert_eq!(impact.impact_kind, kind);
        assert_eq!(
            impact.impact_safe_summary.disposition_marker_ref,
            Some(marker("ml:disposition:1"))
        );
        assert_eq!(
            impact.impact_safe_summary.safe_reason_ref,
            Some(reason("ml:impact-reason:1"))
        );

        let before_supersede = impact.clone();
        impact
            .supersede_with(ConsumptionImpactSummaryRef::new("ml:impact:2"))
            .expect("a distinct next summary should supersede disposition-marked impact");
        assert_eq!(impact.state, ConsumptionImpactSummaryState::Superseded);
        assert_eq!(impact.impact_kind, kind);
        assert_eq!(
            impact.impact_summary_ref,
            before_supersede.impact_summary_ref
        );
        assert_eq!(
            impact.impact_safe_summary,
            before_supersede.impact_safe_summary
        );
        assert_eq!(
            impact.trace_material_ref,
            before_supersede.trace_material_ref
        );
    }
}

#[test]
fn impact_rejects_repeat_disposition_same_identity_and_terminal_mutation() {
    let mut disposition = sample_impact(ConsumptionImpactKind::KnownImpact);
    disposition
        .mark_disposition(marker("ml:disposition:1"), reason("ml:impact-reason:1"))
        .expect("first disposition should succeed");
    let before_repeat = disposition.clone();
    let error = disposition
        .mark_disposition(marker("ml:disposition:2"), reason("ml:impact-reason:2"))
        .expect_err("disposition is legal only from current");
    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );
    assert_eq!(disposition, before_repeat);

    let mut same_identity = sample_impact(ConsumptionImpactKind::NoKnownEffect);
    let before_same = same_identity.clone();
    let error = same_identity
        .supersede_with(ConsumptionImpactSummaryRef::new("ml:impact:1"))
        .expect_err("an impact cannot supersede itself");
    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );
    assert_eq!(same_identity, before_same);

    same_identity
        .supersede_with(ConsumptionImpactSummaryRef::new("ml:impact:2"))
        .expect("distinct current impact should supersede");
    let terminal = same_identity.clone();
    let error = same_identity
        .supersede_with(ConsumptionImpactSummaryRef::new("ml:impact:3"))
        .expect_err("superseded impact is terminal");
    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );
    assert_eq!(same_identity, terminal);
}

#[test]
fn audit_append_is_first_seen_and_copies_explicit_cursor() {
    let mut audit = sample_audit();
    assert_eq!(audit.state, MethodAssetAuditTrailState::TrailOwnerPresent);
    assert!(audit.audit_entry_refs.is_empty());
    assert_eq!(audit.source_cursor_ref, None);

    audit
        .append_entry(
            MethodAssetAuditEntryRef::new("ml:entry:1"),
            MethodAssetAuditCursorRef::new("ml:audit-cursor:1"),
        )
        .expect("owner-present trail should accept an entry");
    audit
        .append_entry(
            MethodAssetAuditEntryRef::new("ml:entry:2"),
            MethodAssetAuditCursorRef::new("ml:audit-cursor:2"),
        )
        .expect("appended trail should preserve prior refs");
    audit
        .append_entry(
            MethodAssetAuditEntryRef::new("ml:entry:1"),
            MethodAssetAuditCursorRef::new("ml:audit-cursor:3"),
        )
        .expect("typed duplicate should retain first-seen refs and copy cursor");

    assert_eq!(
        audit.state,
        MethodAssetAuditTrailState::SafeEntryRefsAppended
    );
    assert_eq!(
        audit.audit_entry_refs.refs,
        vec![
            MethodAssetAuditEntryRef::new("ml:entry:1"),
            MethodAssetAuditEntryRef::new("ml:entry:2")
        ]
    );
    assert_eq!(
        audit.source_cursor_ref,
        Some(MethodAssetAuditCursorRef::new("ml:audit-cursor:3"))
    );
}

#[test]
fn audit_partial_and_unavailable_states_reject_append_without_mutation() {
    for state in [
        MethodAssetAuditTrailState::PartialAuditAvailable,
        MethodAssetAuditTrailState::AuditUnavailable,
    ] {
        let mut audit = sample_audit();
        audit.state = state;
        let before = audit.clone();

        let error = audit
            .append_entry(
                MethodAssetAuditEntryRef::new("ml:entry:blocked"),
                MethodAssetAuditCursorRef::new("ml:audit-cursor:blocked"),
            )
            .expect_err("partial or unavailable audit cannot append");
        assert_eq!(
            error.kind(),
            MethodLibraryDomainErrorKind::InvalidTransition
        );
        assert_eq!(audit, before);
    }
}

#[test]
fn lineage_links_from_linked_and_partial_without_changing_summary_or_state() {
    let mut lineage = sample_lineage();
    let linked_summary = lineage.lineage_summary.clone();
    lineage
        .link_trace_material(MethodAssetTraceMaterialRef::new("ml:trace:1"))
        .expect("linked lineage should accept a trace ref");
    assert_eq!(
        lineage.state,
        MethodAssetEvidenceLineageState::LineageLinked
    );
    assert_eq!(lineage.lineage_summary, linked_summary);

    let before_duplicate = lineage.clone();
    lineage
        .link_trace_material(MethodAssetTraceMaterialRef::new("ml:trace:1"))
        .expect("typed duplicate should be a successful no-op");
    assert_eq!(lineage, before_duplicate);

    lineage
        .mark_partial(reason("ml:lineage-reason:partial"))
        .expect("linked lineage should become partial with a safe reason");
    let partial_summary = lineage.lineage_summary.clone();
    lineage
        .link_trace_material(MethodAssetTraceMaterialRef::new("ml:trace:2"))
        .expect("partial lineage should accept a new trace ref");
    assert_eq!(
        lineage.state,
        MethodAssetEvidenceLineageState::LineagePartial
    );
    assert_eq!(lineage.lineage_summary, partial_summary);
    assert_eq!(
        lineage.trace_material_refs.refs,
        vec![
            MethodAssetTraceMaterialRef::new("ml:trace:1"),
            MethodAssetTraceMaterialRef::new("ml:trace:2")
        ]
    );

    let before_partial_duplicate = lineage.clone();
    lineage
        .link_trace_material(MethodAssetTraceMaterialRef::new("ml:trace:2"))
        .expect("partial typed duplicate should be a successful no-op");
    assert_eq!(lineage, before_partial_duplicate);
}

#[test]
fn unavailable_and_terminal_lineage_reject_links_without_mutation() {
    for state in [
        MethodAssetEvidenceLineageState::LineageUnavailable,
        MethodAssetEvidenceLineageState::BodyCandidateRejected,
    ] {
        let mut lineage = sample_lineage();
        lineage.state = state;
        let before = lineage.clone();

        let error = lineage
            .link_trace_material(MethodAssetTraceMaterialRef::new("ml:trace:blocked"))
            .expect_err("unavailable or terminal lineage must reject linking");
        assert_eq!(
            error.kind(),
            MethodLibraryDomainErrorKind::InvalidTransition
        );
        assert_eq!(lineage, before);
    }
}

#[test]
fn lineage_partial_and_body_rejection_follow_exact_terminal_rules() {
    let mut cannot_partial = sample_lineage();
    cannot_partial.state = MethodAssetEvidenceLineageState::LineageUnavailable;
    let before_partial = cannot_partial.clone();
    let error = cannot_partial
        .mark_partial(reason("ml:lineage-reason:invalid-partial"))
        .expect_err("unavailable lineage cannot become partial");
    assert_eq!(
        error.kind(),
        MethodLibraryDomainErrorKind::InvalidTransition
    );
    assert_eq!(cannot_partial, before_partial);

    for state in [
        MethodAssetEvidenceLineageState::LineageLinked,
        MethodAssetEvidenceLineageState::LineagePartial,
        MethodAssetEvidenceLineageState::LineageUnavailable,
    ] {
        let mut lineage = sample_lineage();
        lineage.state = state;
        if state == MethodAssetEvidenceLineageState::LineagePartial {
            lineage.lineage_summary.safe_reason_ref =
                Some(reason("ml:lineage-reason:existing-partial"));
        }
        let before_rejection = lineage.clone();

        lineage
            .reject_body_candidate(reason("ml:lineage-reason:body-rejected"))
            .expect("linked, partial, or unavailable lineage may reject a body candidate");
        assert_eq!(
            lineage.state,
            MethodAssetEvidenceLineageState::BodyCandidateRejected
        );
        assert_eq!(
            lineage.lineage_summary.safe_reason_ref,
            Some(reason("ml:lineage-reason:body-rejected"))
        );
        assert_eq!(
            lineage.evidence_lineage_ref,
            before_rejection.evidence_lineage_ref
        );
        assert_eq!(
            lineage.lineage_subject_ref,
            before_rejection.lineage_subject_ref
        );
        assert_eq!(
            lineage.external_summary_refs,
            before_rejection.external_summary_refs
        );
        assert_eq!(
            lineage.basis_summary_refs,
            before_rejection.basis_summary_refs
        );
        assert_eq!(
            lineage.trace_material_refs,
            before_rejection.trace_material_refs
        );
        assert_eq!(lineage.audit_trail_ref, before_rejection.audit_trail_ref);
        assert_eq!(
            lineage.lineage_summary.summary_marker_ref,
            before_rejection.lineage_summary.summary_marker_ref
        );

        let terminal = lineage.clone();
        let error = lineage
            .reject_body_candidate(reason("ml:lineage-reason:repeat"))
            .expect_err("body-candidate rejection is terminal");
        assert_eq!(
            error.kind(),
            MethodLibraryDomainErrorKind::InvalidTransition
        );
        assert_eq!(lineage, terminal);
    }
}
