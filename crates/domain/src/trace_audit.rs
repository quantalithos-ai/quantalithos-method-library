//! Trace, impact, audit, and evidence-lineage domain objects closed for `commit-06-a`.

use method_library_contracts::metadata::ActorContext;
use method_library_contracts::{
    ConsumptionContextRef, ConsumptionImpactKind, ConsumptionImpactSafeSummary,
    ConsumptionImpactSourceRef, ConsumptionImpactSummaryRef, ConsumptionImpactSummaryState,
    ExternalSourceSummaryRefSet, FormalizationBasisSummaryRefSet, MethodAssetAuditCursorRef,
    MethodAssetAuditEntryRef, MethodAssetAuditEntryRefSet, MethodAssetAuditTrailRef,
    MethodAssetAuditTrailState, MethodAssetConsumptionMaterialRef, MethodAssetEvidenceLineageRef,
    MethodAssetEvidenceLineageState, MethodAssetEvidenceLineageSummary, MethodAssetSafeReasonRef,
    MethodAssetTraceCursorRef, MethodAssetTraceFreshnessMarkerRef, MethodAssetTraceMaterialRef,
    MethodAssetTraceMaterialRefSet, MethodAssetTraceMaterialState, MethodAssetTraceSourceRefSet,
    MethodAssetTraceSummary, MethodLibrarySafeMarker, TraceSubjectRef,
};

use crate::errors::MethodLibraryDomainError;

/// Body-free trace material organized from formal source-object references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetTraceMaterial {
    /// Stable trace-material anchor.
    pub trace_material_ref: MethodAssetTraceMaterialRef,
    /// Formal trace subject.
    pub trace_subject_ref: TraceSubjectRef,
    /// Verified source-object refs in first-seen order.
    pub source_object_refs: MethodAssetTraceSourceRefSet,
    /// Body-free trace summary.
    pub trace_summary: MethodAssetTraceSummary,
    /// Committed source cursor.
    pub source_cursor_ref: MethodAssetTraceCursorRef,
    /// Freshness source marker.
    pub freshness_marker_ref: MethodAssetTraceFreshnessMarkerRef,
    /// Accepted external summary refs.
    pub external_summary_refs: ExternalSourceSummaryRefSet,
    /// Current trace-material state.
    pub state: MethodAssetTraceMaterialState,
    /// Explicit reason for a non-organized state.
    pub safe_reason_ref: Option<MethodAssetSafeReasonRef>,
}

impl MethodAssetTraceMaterial {
    /// Organizes body-free trace material from formal source refs.
    pub fn from_source_objects(
        trace_material_ref: MethodAssetTraceMaterialRef,
        trace_subject_ref: TraceSubjectRef,
        source_object_refs: MethodAssetTraceSourceRefSet,
        trace_summary: MethodAssetTraceSummary,
        source_cursor_ref: MethodAssetTraceCursorRef,
        freshness_marker_ref: MethodAssetTraceFreshnessMarkerRef,
        external_summary_refs: ExternalSourceSummaryRefSet,
    ) -> Self {
        let state = if source_object_refs.is_empty() {
            MethodAssetTraceMaterialState::Partial
        } else {
            MethodAssetTraceMaterialState::Organized
        };

        Self {
            trace_material_ref,
            trace_subject_ref,
            source_object_refs,
            trace_summary,
            source_cursor_ref,
            freshness_marker_ref,
            external_summary_refs,
            state,
            safe_reason_ref: None,
        }
    }

    /// Applies an explicit state while preserving all source truth.
    pub fn mark_state(
        &mut self,
        next_state: MethodAssetTraceMaterialState,
        reason_ref: Option<MethodAssetSafeReasonRef>,
    ) -> Result<(), MethodLibraryDomainError> {
        match next_state {
            MethodAssetTraceMaterialState::Organized => {
                if self.source_object_refs.is_empty() {
                    return Err(MethodLibraryDomainError::invariant_violation());
                }

                self.state = MethodAssetTraceMaterialState::Organized;
                self.safe_reason_ref = None;
            }
            MethodAssetTraceMaterialState::Partial
            | MethodAssetTraceMaterialState::Stale
            | MethodAssetTraceMaterialState::Unavailable => {
                let reason_ref = reason_ref
                    .ok_or_else(MethodLibraryDomainError::missing_required_typed_input)?;
                self.state = next_state;
                self.safe_reason_ref = Some(reason_ref);
            }
        }

        Ok(())
    }
}

/// Body-free consumption-impact summary and lifecycle owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsumptionImpactSummary {
    /// Stable impact-summary anchor.
    pub impact_summary_ref: ConsumptionImpactSummaryRef,
    /// Formal impact source anchor.
    pub impact_source_ref: ConsumptionImpactSourceRef,
    /// Optional controlled consumption material source.
    pub consumption_material_ref: Option<MethodAssetConsumptionMaterialRef>,
    /// Optional controlled consumption context.
    pub consumption_context_ref: Option<ConsumptionContextRef>,
    /// Immutable impact category.
    pub impact_kind: ConsumptionImpactKind,
    /// Body-free impact summary details.
    pub impact_safe_summary: ConsumptionImpactSafeSummary,
    /// Optional linked trace material.
    pub trace_material_ref: Option<MethodAssetTraceMaterialRef>,
    /// Current impact-summary lifecycle state.
    pub state: ConsumptionImpactSummaryState,
}

impl ConsumptionImpactSummary {
    /// Registers a current impact summary without changing its supplied category.
    pub fn register(
        impact_summary_ref: ConsumptionImpactSummaryRef,
        impact_source_ref: ConsumptionImpactSourceRef,
        consumption_material_ref: Option<MethodAssetConsumptionMaterialRef>,
        consumption_context_ref: Option<ConsumptionContextRef>,
        impact_kind: ConsumptionImpactKind,
        impact_safe_summary: ConsumptionImpactSafeSummary,
        trace_material_ref: Option<MethodAssetTraceMaterialRef>,
    ) -> Self {
        Self {
            impact_summary_ref,
            impact_source_ref,
            consumption_material_ref,
            consumption_context_ref,
            impact_kind,
            impact_safe_summary,
            trace_material_ref,
            state: ConsumptionImpactSummaryState::Current,
        }
    }

    /// Records an explicit safe disposition on a current summary.
    pub fn mark_disposition(
        &mut self,
        disposition_marker_ref: MethodLibrarySafeMarker,
        safe_reason_ref: MethodAssetSafeReasonRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if self.state != ConsumptionImpactSummaryState::Current {
            return Err(MethodLibraryDomainError::invalid_transition());
        }
        if !disposition_marker_ref.is_public_safe()
            || !safe_reason_ref.as_safe_marker().is_public_safe()
        {
            return Err(MethodLibraryDomainError::policy_rejected());
        }

        self.impact_safe_summary.disposition_marker_ref = Some(disposition_marker_ref);
        self.impact_safe_summary.safe_reason_ref = Some(safe_reason_ref);
        self.state = ConsumptionImpactSummaryState::DispositionMarked;
        Ok(())
    }

    /// Marks this summary superseded by a distinct next summary identity.
    pub fn supersede_with(
        &mut self,
        next_impact_summary_ref: ConsumptionImpactSummaryRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if self.state == ConsumptionImpactSummaryState::Superseded
            || self.impact_summary_ref == next_impact_summary_ref
        {
            return Err(MethodLibraryDomainError::invalid_transition());
        }

        self.state = ConsumptionImpactSummaryState::Superseded;
        Ok(())
    }
}

/// Append-only body-free audit trail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetAuditTrail {
    /// Stable audit-trail anchor.
    pub audit_trail_ref: MethodAssetAuditTrailRef,
    /// Formal audit subject.
    pub audit_subject_ref: TraceSubjectRef,
    /// Linked trace-material refs.
    pub trace_material_refs: MethodAssetTraceMaterialRefSet,
    /// Actor associated with the trail owner.
    pub actor_context: ActorContext,
    /// Safe reason copied when the trail owner is established.
    pub safe_reason_ref: MethodAssetSafeReasonRef,
    /// Append-only safe audit-entry refs.
    pub audit_entry_refs: MethodAssetAuditEntryRefSet,
    /// Cursor copied from the latest accepted append.
    pub source_cursor_ref: Option<MethodAssetAuditCursorRef>,
    /// Current audit support state.
    pub state: MethodAssetAuditTrailState,
}

impl MethodAssetAuditTrail {
    /// Establishes an empty audit trail for a formal subject.
    pub fn for_subject(
        audit_trail_ref: MethodAssetAuditTrailRef,
        audit_subject_ref: TraceSubjectRef,
        actor_context: ActorContext,
        safe_reason_ref: MethodAssetSafeReasonRef,
    ) -> Self {
        Self {
            audit_trail_ref,
            audit_subject_ref,
            trace_material_refs: MethodAssetTraceMaterialRefSet::new(),
            actor_context,
            safe_reason_ref,
            audit_entry_refs: MethodAssetAuditEntryRefSet::new(),
            source_cursor_ref: None,
            state: MethodAssetAuditTrailState::TrailOwnerPresent,
        }
    }

    /// Appends a safe entry ref and copies its committed source cursor.
    pub fn append_entry(
        &mut self,
        entry_ref: MethodAssetAuditEntryRef,
        source_cursor_ref: MethodAssetAuditCursorRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if !matches!(
            self.state,
            MethodAssetAuditTrailState::TrailOwnerPresent
                | MethodAssetAuditTrailState::SafeEntryRefsAppended
        ) {
            return Err(MethodLibraryDomainError::invalid_transition());
        }

        self.audit_entry_refs.insert(entry_ref);
        self.source_cursor_ref = Some(source_cursor_ref);
        self.state = MethodAssetAuditTrailState::SafeEntryRefsAppended;
        Ok(())
    }
}

/// Body-free evidence-lineage graph for a formal subject.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetEvidenceLineage {
    /// Stable evidence-lineage anchor.
    pub evidence_lineage_ref: MethodAssetEvidenceLineageRef,
    /// Formal lineage subject.
    pub lineage_subject_ref: TraceSubjectRef,
    /// Accepted external summary refs.
    pub external_summary_refs: ExternalSourceSummaryRefSet,
    /// Accepted formalization basis-summary refs.
    pub basis_summary_refs: FormalizationBasisSummaryRefSet,
    /// Linked trace-material refs.
    pub trace_material_refs: MethodAssetTraceMaterialRefSet,
    /// Optional linked audit trail.
    pub audit_trail_ref: Option<MethodAssetAuditTrailRef>,
    /// Body-free lineage summary.
    pub lineage_summary: MethodAssetEvidenceLineageSummary,
    /// Current evidence-lineage support state.
    pub state: MethodAssetEvidenceLineageState,
}

impl MethodAssetEvidenceLineage {
    /// Establishes lineage from accepted external and basis-summary refs.
    pub fn from_external_and_basis(
        evidence_lineage_ref: MethodAssetEvidenceLineageRef,
        lineage_subject_ref: TraceSubjectRef,
        external_summary_refs: ExternalSourceSummaryRefSet,
        basis_summary_refs: FormalizationBasisSummaryRefSet,
        lineage_summary: MethodAssetEvidenceLineageSummary,
    ) -> Self {
        Self {
            evidence_lineage_ref,
            lineage_subject_ref,
            external_summary_refs,
            basis_summary_refs,
            trace_material_refs: MethodAssetTraceMaterialRefSet::new(),
            audit_trail_ref: None,
            lineage_summary,
            state: MethodAssetEvidenceLineageState::LineageLinked,
        }
    }

    /// Links a trace-material ref without changing lineage state or summary.
    pub fn link_trace_material(
        &mut self,
        trace_material_ref: MethodAssetTraceMaterialRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if !matches!(
            self.state,
            MethodAssetEvidenceLineageState::LineageLinked
                | MethodAssetEvidenceLineageState::LineagePartial
        ) {
            return Err(MethodLibraryDomainError::invalid_transition());
        }

        self.trace_material_refs.insert(trace_material_ref);
        Ok(())
    }

    /// Marks linked lineage partial with an explicit safe reason.
    pub fn mark_partial(
        &mut self,
        reason_ref: MethodAssetSafeReasonRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if !matches!(
            self.state,
            MethodAssetEvidenceLineageState::LineageLinked
                | MethodAssetEvidenceLineageState::LineagePartial
        ) {
            return Err(MethodLibraryDomainError::invalid_transition());
        }

        self.lineage_summary.safe_reason_ref = Some(reason_ref);
        self.state = MethodAssetEvidenceLineageState::LineagePartial;
        Ok(())
    }

    /// Rejects a body candidate by recording only its formal safe reason.
    ///
    /// ```compile_fail
    /// use method_library_domain::MethodAssetEvidenceLineage;
    ///
    /// fn raw_body(lineage: &mut MethodAssetEvidenceLineage) {
    ///     let _ = lineage.reject_body_candidate(());
    /// }
    /// ```
    pub fn reject_body_candidate(
        &mut self,
        reason_ref: MethodAssetSafeReasonRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if self.state == MethodAssetEvidenceLineageState::BodyCandidateRejected {
            return Err(MethodLibraryDomainError::invalid_transition());
        }

        self.lineage_summary.safe_reason_ref = Some(reason_ref);
        self.state = MethodAssetEvidenceLineageState::BodyCandidateRejected;
        Ok(())
    }
}
