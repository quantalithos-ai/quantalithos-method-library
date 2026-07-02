//! Formalization/version support carriers closed for `commit-04-a`.

use serde::{Deserialize, Serialize};

use crate::refs::{
    ExternalSourceSummaryRef, FormalMethodAssetVersionRef, FormalizationBasisSummaryRef,
    FormalizationEligibilityRejectionRef, FormalizationStateRef, GovernanceBasisRef,
    MethodAssetCatalogEntryRef, MethodAssetDefinitionRef,
};
use crate::views::MethodLibrarySafeMarker;

fn push_unique<T>(items: &mut Vec<T>, next: T)
where
    T: PartialEq,
{
    if !items.iter().any(|existing| existing == &next) {
        items.push(next);
    }
}

/// Current-boundary formalization state labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormalizationStateKind {
    /// Formalization is pending basis or eligibility closure.
    AssessmentPending,
    /// Formalization is eligible for version establishment.
    Eligible,
    /// Formalization is blocked by a safe eligibility rejection.
    Ineligible,
    /// Formalization already established a formal version.
    VersionEstablished,
}

/// Current-boundary formal method-version state labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormalMethodAssetVersionState {
    /// The formal version is active.
    Active,
    /// The formal version is superseded by an explicit next version.
    Superseded,
    /// The formal version is retired and historical only.
    Retired,
}

/// Current-boundary basis source labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormalizationBasisKind {
    /// The basis came from an accepted external summary.
    ExternalSummary,
    /// The basis came from a governance basis ref.
    GovernanceBasis,
    /// The basis came from an explicit reassessment marker.
    BasisReassessment,
}

/// Deterministic current-boundary basis-kind set.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalizationBasisKindSet {
    /// Accepted basis kinds ordered by first insertion after canonical dedup.
    pub kinds: Vec<FormalizationBasisKind>,
}

impl FormalizationBasisKindSet {
    /// Creates an empty basis-kind set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a deterministic set from basis kinds.
    pub fn from_kinds(kinds: impl IntoIterator<Item = FormalizationBasisKind>) -> Self {
        let mut set = Self::new();
        for next in kinds {
            set.insert(next);
        }
        set
    }

    /// Inserts a basis kind if it has not already been accepted.
    pub fn insert(&mut self, next: FormalizationBasisKind) {
        push_unique(&mut self.kinds, next);
    }
}

/// Deterministic basis-summary ref set.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalizationBasisSummaryRefSet {
    /// Basis-summary refs ordered by first insertion after canonical dedup.
    pub refs: Vec<FormalizationBasisSummaryRef>,
}

impl FormalizationBasisSummaryRefSet {
    /// Creates an empty ref set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a deterministic set from refs.
    pub fn from_refs(refs: impl IntoIterator<Item = FormalizationBasisSummaryRef>) -> Self {
        let mut set = Self::new();
        for next in refs {
            set.insert(next);
        }
        set
    }

    /// Inserts a ref if it has not already been accepted.
    pub fn insert(&mut self, next: FormalizationBasisSummaryRef) {
        push_unique(&mut self.refs, next);
    }
}

/// Deterministic formal-version ref set.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalMethodAssetVersionRefSet {
    /// Formal-version refs ordered by first insertion after canonical dedup.
    pub refs: Vec<FormalMethodAssetVersionRef>,
}

impl FormalMethodAssetVersionRefSet {
    /// Creates an empty ref set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a deterministic set from refs.
    pub fn from_refs(refs: impl IntoIterator<Item = FormalMethodAssetVersionRef>) -> Self {
        let mut set = Self::new();
        for next in refs {
            set.insert(next);
        }
        set
    }

    /// Inserts a ref if it has not already been accepted.
    pub fn insert(&mut self, next: FormalMethodAssetVersionRef) {
        push_unique(&mut self.refs, next);
    }
}

/// Body-free basis safe summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalizationBasisSafeSummary {
    /// Marker for the accepted safe summary.
    pub summary_marker_ref: MethodLibrarySafeMarker,
    /// Optional accepted external summary ref.
    pub source_summary_ref: Option<ExternalSourceSummaryRef>,
    /// Optional governance basis ref.
    pub governance_basis_ref: Option<GovernanceBasisRef>,
    /// Optional reassessment marker.
    pub reassessment_marker_ref: Option<MethodLibrarySafeMarker>,
}

impl FormalizationBasisSafeSummary {
    /// Creates a new body-free basis safe summary.
    pub fn new(
        summary_marker_ref: MethodLibrarySafeMarker,
        source_summary_ref: Option<ExternalSourceSummaryRef>,
        governance_basis_ref: Option<GovernanceBasisRef>,
        reassessment_marker_ref: Option<MethodLibrarySafeMarker>,
    ) -> Self {
        Self {
            summary_marker_ref,
            source_summary_ref,
            governance_basis_ref,
            reassessment_marker_ref,
        }
    }
}

/// Body-free current-boundary reason summary for formalization state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalizationStateReasonSummary {
    /// Marker describing the current safe reason.
    pub reason_marker_ref: MethodLibrarySafeMarker,
    /// Basis-summary refs carried into the current reason.
    pub basis_summary_refs: FormalizationBasisSummaryRefSet,
    /// Optional safe rejection ref for ineligible states.
    pub eligibility_rejection_ref: Option<FormalizationEligibilityRejectionRef>,
}

impl FormalizationStateReasonSummary {
    /// Creates a new body-free current-boundary reason summary.
    pub fn new(
        reason_marker_ref: MethodLibrarySafeMarker,
        basis_summary_refs: FormalizationBasisSummaryRefSet,
        eligibility_rejection_ref: Option<FormalizationEligibilityRejectionRef>,
    ) -> Self {
        Self {
            reason_marker_ref,
            basis_summary_refs,
            eligibility_rejection_ref,
        }
    }
}

/// Body-free current-boundary summary for a formal version boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalVersionBoundarySummary {
    /// Boundary marker copied from accepted inputs.
    pub boundary_marker_ref: MethodLibrarySafeMarker,
    /// Definition ref carried into the formal version boundary.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Catalog-entry ref carried into the formal version boundary.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Formalization-state ref that established the version.
    pub formalization_state_ref: FormalizationStateRef,
    /// Accepted basis-summary refs for the version boundary.
    pub basis_summary_refs: FormalizationBasisSummaryRefSet,
}

impl FormalVersionBoundarySummary {
    /// Creates a new body-free formal version boundary summary.
    pub fn new(
        boundary_marker_ref: MethodLibrarySafeMarker,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
        formalization_state_ref: FormalizationStateRef,
        basis_summary_refs: FormalizationBasisSummaryRefSet,
    ) -> Self {
        Self {
            boundary_marker_ref,
            definition_ref,
            catalog_entry_ref,
            formalization_state_ref,
            basis_summary_refs,
        }
    }
}

/// Body-free current-boundary definition/catalog requirement carrier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetDefinitionRequirement {
    /// Whether a definition is required for eligibility.
    pub definition_required: bool,
    /// Whether a catalog entry is required for eligibility.
    pub catalog_entry_required: bool,
    /// Whether the definition must still be active.
    pub active_definition_required: bool,
    /// Whether the catalog entry must still be visible.
    pub visible_catalog_entry_required: bool,
    /// Requirement marker copied from accepted inputs.
    pub requirement_marker_ref: MethodLibrarySafeMarker,
}

impl MethodAssetDefinitionRequirement {
    /// Creates a new body-free requirement carrier.
    pub fn new(
        definition_required: bool,
        catalog_entry_required: bool,
        active_definition_required: bool,
        visible_catalog_entry_required: bool,
        requirement_marker_ref: MethodLibrarySafeMarker,
    ) -> Self {
        Self {
            definition_required,
            catalog_entry_required,
            active_definition_required,
            visible_catalog_entry_required,
            requirement_marker_ref,
        }
    }
}

/// Body-free current-boundary basis requirement carrier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalizationBasisRequirement {
    /// Accepted basis kinds for the requirement.
    pub accepted_basis_kinds: FormalizationBasisKindSet,
    /// Whether the rule requires a body-free summary marker.
    pub requires_body_free_summary: bool,
    /// Requirement marker copied from accepted inputs.
    pub requirement_marker_ref: MethodLibrarySafeMarker,
}

impl FormalizationBasisRequirement {
    /// Creates a new body-free basis requirement.
    pub fn new(
        accepted_basis_kinds: FormalizationBasisKindSet,
        requires_body_free_summary: bool,
        requirement_marker_ref: MethodLibrarySafeMarker,
    ) -> Self {
        Self {
            accepted_basis_kinds,
            requires_body_free_summary,
            requirement_marker_ref,
        }
    }
}

/// Body-free current-boundary governance basis requirement carrier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OptionalGovernanceBasisRequirement {
    /// Whether a governance basis is allowed at all.
    pub governance_basis_allowed: bool,
    /// Whether a governance basis is required.
    pub governance_basis_required: bool,
    /// Requirement marker copied from accepted inputs.
    pub requirement_marker_ref: MethodLibrarySafeMarker,
}

impl OptionalGovernanceBasisRequirement {
    /// Creates a new current-boundary governance basis requirement.
    pub fn new(
        governance_basis_allowed: bool,
        governance_basis_required: bool,
        requirement_marker_ref: MethodLibrarySafeMarker,
    ) -> Self {
        Self {
            governance_basis_allowed: governance_basis_allowed || governance_basis_required,
            governance_basis_required,
            requirement_marker_ref,
        }
    }
}

/// Current-boundary forbidden trigger labels for implicit formalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForbiddenFormalizationTriggerKind {
    /// Read-driven trigger.
    Read,
    /// Reference-driven trigger.
    Reference,
    /// Sync-driven trigger.
    Sync,
    /// Runtime-use-driven trigger.
    RuntimeUse,
    /// Downstream-consumption-driven trigger.
    DownstreamConsumption,
    /// Query-driven trigger.
    Query,
}

/// Deterministic current-boundary forbidden-trigger set.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForbiddenFormalizationTriggerKindSet {
    /// Forbidden trigger kinds ordered by first insertion after canonical dedup.
    pub forbidden_kinds: Vec<ForbiddenFormalizationTriggerKind>,
}

impl ForbiddenFormalizationTriggerKindSet {
    /// Creates an empty forbidden-trigger set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a deterministic set from trigger kinds.
    pub fn from_kinds(
        forbidden_kinds: impl IntoIterator<Item = ForbiddenFormalizationTriggerKind>,
    ) -> Self {
        let mut set = Self::new();
        for next in forbidden_kinds {
            set.insert(next);
        }
        set
    }

    /// Inserts a trigger kind if it has not already been forbidden.
    pub fn insert(&mut self, next: ForbiddenFormalizationTriggerKind) {
        push_unique(&mut self.forbidden_kinds, next);
    }
}
