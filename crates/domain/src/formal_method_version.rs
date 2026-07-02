//! Formalization and formal method-version truth objects closed for `commit-04-a`.

use method_library_contracts::{
    ExternalSourceSummaryRef, FormalMethodAssetVersionRef, FormalMethodAssetVersionState,
    FormalVersionBoundarySummary, FormalizationBasisKind, FormalizationBasisSafeSummary,
    FormalizationBasisSummaryRef, FormalizationBasisSummaryRefSet, FormalizationStateKind,
    FormalizationStateReasonSummary, FormalizationStateRef, GovernanceBasisRef,
    MethodAssetCatalogEntryRef, MethodAssetDefinitionRef,
};

use crate::errors::MethodLibraryDomainError;

fn assert_compatible_basis_summary(
    basis_kind: FormalizationBasisKind,
    external_summary_ref: Option<&ExternalSourceSummaryRef>,
    governance_basis_ref: Option<&GovernanceBasisRef>,
    basis_safe_summary: &FormalizationBasisSafeSummary,
) -> Result<(), MethodLibraryDomainError> {
    let has_source = basis_safe_summary.source_summary_ref.is_some()
        || basis_safe_summary.governance_basis_ref.is_some()
        || basis_safe_summary.reassessment_marker_ref.is_some();
    if !has_source || !basis_safe_summary.summary_marker_ref.assert_no_body() {
        return Err(MethodLibraryDomainError::body_free_boundary_violation());
    }

    let source_matches = external_summary_ref == basis_safe_summary.source_summary_ref.as_ref();
    let governance_matches =
        governance_basis_ref == basis_safe_summary.governance_basis_ref.as_ref();
    let reassessment_present = basis_safe_summary.reassessment_marker_ref.is_some();

    match basis_kind {
        FormalizationBasisKind::ExternalSummary
            if source_matches && external_summary_ref.is_some() =>
        {
            Ok(())
        }
        FormalizationBasisKind::GovernanceBasis
            if governance_matches && governance_basis_ref.is_some() =>
        {
            Ok(())
        }
        FormalizationBasisKind::BasisReassessment if reassessment_present => Ok(()),
        _ => Err(MethodLibraryDomainError::body_free_boundary_violation()),
    }
}

/// Support summary for current-boundary formalization basis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormalizationBasisSummary {
    /// Stable basis-summary anchor.
    pub basis_summary_ref: FormalizationBasisSummaryRef,
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Optional linked catalog-entry context.
    pub catalog_entry_ref: Option<MethodAssetCatalogEntryRef>,
    /// Current-boundary basis source kind.
    pub basis_kind: FormalizationBasisKind,
    /// Optional accepted external summary ref.
    pub external_summary_ref: Option<ExternalSourceSummaryRef>,
    /// Optional governance basis ref.
    pub governance_basis_ref: Option<GovernanceBasisRef>,
    /// Body-free basis safe summary.
    pub basis_safe_summary: FormalizationBasisSafeSummary,
}

impl FormalizationBasisSummary {
    /// Creates an external-summary-backed basis summary.
    pub fn from_external_summary(
        basis_summary_ref: FormalizationBasisSummaryRef,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: Option<MethodAssetCatalogEntryRef>,
        external_summary_ref: ExternalSourceSummaryRef,
        basis_safe_summary: FormalizationBasisSafeSummary,
    ) -> Self {
        Self {
            basis_summary_ref,
            definition_ref,
            catalog_entry_ref,
            basis_kind: FormalizationBasisKind::ExternalSummary,
            external_summary_ref: Some(external_summary_ref),
            governance_basis_ref: None,
            basis_safe_summary,
        }
    }

    /// Creates a governance-basis-backed summary.
    pub fn from_governance_basis(
        basis_summary_ref: FormalizationBasisSummaryRef,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: Option<MethodAssetCatalogEntryRef>,
        governance_basis_ref: GovernanceBasisRef,
        basis_safe_summary: FormalizationBasisSafeSummary,
    ) -> Self {
        Self {
            basis_summary_ref,
            definition_ref,
            catalog_entry_ref,
            basis_kind: FormalizationBasisKind::GovernanceBasis,
            external_summary_ref: None,
            governance_basis_ref: Some(governance_basis_ref),
            basis_safe_summary,
        }
    }

    /// Creates a reassessment-backed summary.
    pub fn from_basis_reassessment(
        basis_summary_ref: FormalizationBasisSummaryRef,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: Option<MethodAssetCatalogEntryRef>,
        basis_safe_summary: FormalizationBasisSafeSummary,
    ) -> Self {
        Self {
            external_summary_ref: basis_safe_summary.source_summary_ref.clone(),
            governance_basis_ref: basis_safe_summary.governance_basis_ref.clone(),
            basis_summary_ref,
            definition_ref,
            catalog_entry_ref,
            basis_kind: FormalizationBasisKind::BasisReassessment,
            basis_safe_summary,
        }
    }

    /// Confirms the summary still applies to the provided definition.
    pub fn assert_applicable_to_definition(
        &self,
        definition_ref: &MethodAssetDefinitionRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if &self.definition_ref == definition_ref {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invariant_violation())
        }
    }

    /// Verifies the body-free current-boundary basis-summary invariants.
    pub fn assert_body_free(&self) -> Result<(), MethodLibraryDomainError> {
        assert_compatible_basis_summary(
            self.basis_kind,
            self.external_summary_ref.as_ref(),
            self.governance_basis_ref.as_ref(),
            &self.basis_safe_summary,
        )
    }

    /// Rejects in-place supersession using the same basis-summary ref.
    pub fn supersede_with(
        &self,
        next_basis_summary_ref: &FormalizationBasisSummaryRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if &self.basis_summary_ref == next_basis_summary_ref {
            return Err(MethodLibraryDomainError::invalid_transition());
        }

        Ok(())
    }
}

/// State owner for current-boundary formalization lifecycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormalizationState {
    /// Stable formalization-state anchor.
    pub formalization_state_ref: FormalizationStateRef,
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Linked catalog-entry context.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Current-boundary formalization state.
    pub state_kind: FormalizationStateKind,
    /// Safe reason summary for the current state.
    pub state_reason_summary: FormalizationStateReasonSummary,
    /// Accepted basis-summary refs for the current state.
    pub basis_summary_refs: FormalizationBasisSummaryRefSet,
    /// Current formal version ref once formalization is established.
    pub current_formal_version_ref: Option<FormalMethodAssetVersionRef>,
}

impl FormalizationState {
    /// Creates a pending formalization state for a definition/catalog pair.
    pub fn pending_for_definition(
        formalization_state_ref: FormalizationStateRef,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
        state_reason_summary: FormalizationStateReasonSummary,
    ) -> Self {
        Self {
            formalization_state_ref,
            definition_ref,
            catalog_entry_ref,
            state_kind: FormalizationStateKind::AssessmentPending,
            basis_summary_refs: state_reason_summary.basis_summary_refs.clone(),
            state_reason_summary,
            current_formal_version_ref: None,
        }
    }

    /// Returns whether the state is already linked to a formal version.
    pub fn is_formalized(&self) -> bool {
        self.state_kind == FormalizationStateKind::VersionEstablished
            && self.current_formal_version_ref.is_some()
    }

    /// Records a still-pending assessment result.
    pub fn mark_assessment_pending(
        &mut self,
        reason_summary: FormalizationStateReasonSummary,
        basis_summary_refs: FormalizationBasisSummaryRefSet,
    ) -> Result<(), MethodLibraryDomainError> {
        match self.state_kind {
            FormalizationStateKind::AssessmentPending | FormalizationStateKind::Ineligible => {
                let mut reason_summary = reason_summary;
                reason_summary.basis_summary_refs = basis_summary_refs.clone();
                self.state_kind = FormalizationStateKind::AssessmentPending;
                self.state_reason_summary = reason_summary;
                self.basis_summary_refs = basis_summary_refs;
                self.current_formal_version_ref = None;
                Ok(())
            }
            _ => Err(MethodLibraryDomainError::invalid_transition()),
        }
    }

    /// Marks the formalization state eligible while preserving body-free basis refs.
    pub fn mark_eligible(
        &mut self,
        basis_summary_refs: FormalizationBasisSummaryRefSet,
    ) -> Result<(), MethodLibraryDomainError> {
        match self.state_kind {
            FormalizationStateKind::AssessmentPending | FormalizationStateKind::Ineligible => {
                self.state_kind = FormalizationStateKind::Eligible;
                self.state_reason_summary.basis_summary_refs = basis_summary_refs.clone();
                self.state_reason_summary.eligibility_rejection_ref = None;
                self.basis_summary_refs = basis_summary_refs;
                self.current_formal_version_ref = None;
                Ok(())
            }
            _ => Err(MethodLibraryDomainError::invalid_transition()),
        }
    }

    /// Records an ineligible result with an explicit safe reason summary.
    pub fn block(
        &mut self,
        reason_summary: FormalizationStateReasonSummary,
    ) -> Result<(), MethodLibraryDomainError> {
        match self.state_kind {
            FormalizationStateKind::AssessmentPending | FormalizationStateKind::Ineligible => {
                self.state_kind = FormalizationStateKind::Ineligible;
                self.basis_summary_refs = reason_summary.basis_summary_refs.clone();
                self.state_reason_summary = reason_summary;
                self.current_formal_version_ref = None;
                Ok(())
            }
            _ => Err(MethodLibraryDomainError::invalid_transition()),
        }
    }

    /// Links the state to an established formal version.
    pub fn mark_formalized(
        &mut self,
        formal_version_ref: FormalMethodAssetVersionRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if self.state_kind != FormalizationStateKind::Eligible
            || self.current_formal_version_ref.is_some()
        {
            return Err(MethodLibraryDomainError::invalid_transition());
        }

        self.state_kind = FormalizationStateKind::VersionEstablished;
        self.current_formal_version_ref = Some(formal_version_ref);
        Ok(())
    }
}

/// Truth owner for a current-boundary formal method version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormalMethodAssetVersion {
    /// Stable formal-version anchor.
    pub formal_version_ref: FormalMethodAssetVersionRef,
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Linked catalog-entry context.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Linked formalization-state anchor.
    pub formalization_state_ref: FormalizationStateRef,
    /// Current-boundary version state truth field.
    pub version_state: FormalMethodAssetVersionState,
    /// Body-free boundary summary for the formal version.
    pub version_boundary_summary: FormalVersionBoundarySummary,
    /// Accepted basis-summary refs for the version.
    pub basis_summary_refs: FormalizationBasisSummaryRefSet,
    /// Optional previous formal version superseded by this one.
    pub supersedes_version_ref: Option<FormalMethodAssetVersionRef>,
}

impl FormalMethodAssetVersion {
    /// Establishes an active formal version from a formalization state boundary summary.
    pub fn from_formalization_state(
        formal_version_ref: FormalMethodAssetVersionRef,
        version_boundary_summary: FormalVersionBoundarySummary,
    ) -> Self {
        Self {
            formal_version_ref,
            definition_ref: version_boundary_summary.definition_ref.clone(),
            catalog_entry_ref: version_boundary_summary.catalog_entry_ref.clone(),
            formalization_state_ref: version_boundary_summary.formalization_state_ref.clone(),
            version_state: FormalMethodAssetVersionState::Active,
            basis_summary_refs: version_boundary_summary.basis_summary_refs.clone(),
            version_boundary_summary,
            supersedes_version_ref: None,
        }
    }

    /// Establishes an active follow-up version with an explicit superseded previous version.
    pub fn from_explicit_version_change(
        formal_version_ref: FormalMethodAssetVersionRef,
        version_boundary_summary: FormalVersionBoundarySummary,
        previous_version_ref: FormalMethodAssetVersionRef,
    ) -> Self {
        Self {
            formal_version_ref,
            definition_ref: version_boundary_summary.definition_ref.clone(),
            catalog_entry_ref: version_boundary_summary.catalog_entry_ref.clone(),
            formalization_state_ref: version_boundary_summary.formalization_state_ref.clone(),
            version_state: FormalMethodAssetVersionState::Active,
            basis_summary_refs: version_boundary_summary.basis_summary_refs.clone(),
            version_boundary_summary,
            supersedes_version_ref: Some(previous_version_ref),
        }
    }

    /// Confirms the version remains bound to the provided definition.
    pub fn assert_definition_matches(
        &self,
        definition_ref: &MethodAssetDefinitionRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if &self.definition_ref == definition_ref {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invariant_violation())
        }
    }

    /// Confirms the version still points to the provided formalization-state owner.
    pub fn assert_formalized_by(
        &self,
        formalization_state_ref: &FormalizationStateRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if &self.formalization_state_ref == formalization_state_ref {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invariant_violation())
        }
    }

    /// Records a semantic change without leaving the active state.
    pub fn record_semantic_change(
        &mut self,
        next_boundary_summary: FormalVersionBoundarySummary,
    ) -> Result<(), MethodLibraryDomainError> {
        if self.version_state != FormalMethodAssetVersionState::Active {
            return Err(MethodLibraryDomainError::invalid_transition());
        }
        if next_boundary_summary.definition_ref != self.definition_ref
            || next_boundary_summary.catalog_entry_ref != self.catalog_entry_ref
            || next_boundary_summary.formalization_state_ref != self.formalization_state_ref
        {
            return Err(MethodLibraryDomainError::invariant_violation());
        }

        self.version_boundary_summary = next_boundary_summary.clone();
        self.basis_summary_refs = next_boundary_summary.basis_summary_refs;
        self.version_state = FormalMethodAssetVersionState::Active;
        Ok(())
    }

    /// Marks the current version superseded by an explicit next version ref.
    pub fn supersede_with(
        &mut self,
        next_version_ref: &FormalMethodAssetVersionRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if self.version_state != FormalMethodAssetVersionState::Active
            || &self.formal_version_ref == next_version_ref
        {
            return Err(MethodLibraryDomainError::invalid_transition());
        }

        self.version_state = FormalMethodAssetVersionState::Superseded;
        Ok(())
    }

    /// Marks the version retired.
    pub fn mark_retired(&mut self) -> Result<(), MethodLibraryDomainError> {
        match self.version_state {
            FormalMethodAssetVersionState::Active | FormalMethodAssetVersionState::Superseded => {
                self.version_state = FormalMethodAssetVersionState::Retired;
                Ok(())
            }
            FormalMethodAssetVersionState::Retired => {
                Err(MethodLibraryDomainError::invalid_transition())
            }
        }
    }

    /// Returns whether this is the active current version for the state owner.
    pub fn is_current_for(&self, state_ref: &FormalizationStateRef) -> bool {
        &self.formalization_state_ref == state_ref
            && self.version_state == FormalMethodAssetVersionState::Active
    }
}
