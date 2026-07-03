//! Controlled-consumption carriers closed for `commit-05-a`.

use serde::{Deserialize, Serialize};

use crate::formalization::FormalMethodAssetVersionState;
use crate::refs::{
    DownstreamConsumptionBoundaryRef, FormalMethodAssetVersionRef, MethodAssetDefinitionRef,
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

/// Named safe-marker wrapper for a definition-use guard reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DefinitionUseGuardReasonRef {
    /// The current-boundary safe marker.
    pub safe_marker: MethodLibrarySafeMarker,
}

impl DefinitionUseGuardReasonRef {
    /// Creates a named wrapper over the exact safe-marker carrier.
    pub fn new(safe_marker: MethodLibrarySafeMarker) -> Self {
        Self { safe_marker }
    }

    /// Returns the wrapped safe marker.
    pub fn as_safe_marker(&self) -> &MethodLibrarySafeMarker {
        &self.safe_marker
    }
}

impl From<DefinitionUseGuardReasonRef> for MethodLibrarySafeMarker {
    fn from(value: DefinitionUseGuardReasonRef) -> Self {
        value.safe_marker
    }
}

/// Named safe-marker wrapper for a consumption-boundary reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConsumptionBoundaryReasonRef {
    /// The current-boundary safe marker.
    pub safe_marker: MethodLibrarySafeMarker,
}

impl ConsumptionBoundaryReasonRef {
    /// Creates a named wrapper over the exact safe-marker carrier.
    pub fn new(safe_marker: MethodLibrarySafeMarker) -> Self {
        Self { safe_marker }
    }

    /// Returns the wrapped safe marker.
    pub fn as_safe_marker(&self) -> &MethodLibrarySafeMarker {
        &self.safe_marker
    }
}

impl From<ConsumptionBoundaryReasonRef> for MethodLibrarySafeMarker {
    fn from(value: ConsumptionBoundaryReasonRef) -> Self {
        value.safe_marker
    }
}

/// Current-boundary controlled-consumption material states.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodAssetConsumptionMaterialState {
    /// Material was prepared from a formal version and boundary.
    Prepared,
    /// Material is ready for controlled downstream use.
    Ready,
    /// Material remains readable but stale.
    Stale,
    /// Material is currently unavailable.
    Unavailable,
    /// Material is readable but constrained.
    Constrained,
}

/// Current-boundary definition-vs-use guard states.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionUseBoundaryGuardState {
    /// The guard is actively monitoring use-side writeback violations.
    Monitoring,
    /// A safe violation marker was recorded.
    ViolationRecorded,
    /// The candidate was safely rejected.
    RejectedCandidate,
    /// The candidate requires manual review through a safe marker.
    ManualReviewRequired,
}

/// Current-boundary downstream-consumption boundary states.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownstreamConsumptionBoundaryState {
    /// The boundary is registered for controlled consumption.
    Registered,
    /// The use family is unsupported in the current boundary.
    Unsupported,
    /// The boundary currently constrains consumption.
    Constrained,
    /// The boundary is currently unavailable.
    Unavailable,
    /// The boundary is retired and historical only.
    Retired,
}

/// Current-boundary availability-state transition targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodAssetConsumptionAvailabilityTarget {
    /// Copies to `MethodAssetConsumptionMaterialState::Ready`.
    Ready,
    /// Copies to `MethodAssetConsumptionMaterialState::Stale`.
    Stale,
    /// Copies to `MethodAssetConsumptionMaterialState::Unavailable`.
    Unavailable,
    /// Copies to `MethodAssetConsumptionMaterialState::Constrained`.
    Constrained,
}

/// Current-boundary availability marker source families.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodAssetConsumptionAvailabilityMarkerSource {
    /// The marker came from an availability resolver.
    AvailabilityResolver,
    /// The marker came from a degraded-state mapper.
    DegradedMapper,
    /// The marker came from a downstream boundary guard.
    DownstreamConsumptionBoundaryGuard,
}

/// Exact copy-only availability marker carrier for controlled consumption.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetConsumptionAvailabilityMarker {
    /// Marker for the accepted availability decision.
    pub marker_ref: MethodLibrarySafeMarker,
    /// Current-boundary material-state target.
    pub target_state: MethodAssetConsumptionAvailabilityTarget,
    /// Formal marker source family.
    pub source_kind: MethodAssetConsumptionAvailabilityMarkerSource,
    /// Safe source marker for the availability decision.
    pub source_marker_ref: MethodLibrarySafeMarker,
    /// Optional safe reason marker.
    pub reason_ref: Option<MethodLibrarySafeMarker>,
}

impl MethodAssetConsumptionAvailabilityMarker {
    /// Creates an exact copy-only availability marker.
    pub fn new(
        marker_ref: MethodLibrarySafeMarker,
        target_state: MethodAssetConsumptionAvailabilityTarget,
        source_kind: MethodAssetConsumptionAvailabilityMarkerSource,
        source_marker_ref: MethodLibrarySafeMarker,
        reason_ref: Option<MethodLibrarySafeMarker>,
    ) -> Self {
        Self {
            marker_ref,
            target_state,
            source_kind,
            source_marker_ref,
            reason_ref,
        }
    }

    /// Returns the material-state target copied by the domain layer.
    pub const fn material_state(&self) -> MethodAssetConsumptionMaterialState {
        match self.target_state {
            MethodAssetConsumptionAvailabilityTarget::Ready => {
                MethodAssetConsumptionMaterialState::Ready
            }
            MethodAssetConsumptionAvailabilityTarget::Stale => {
                MethodAssetConsumptionMaterialState::Stale
            }
            MethodAssetConsumptionAvailabilityTarget::Unavailable => {
                MethodAssetConsumptionMaterialState::Unavailable
            }
            MethodAssetConsumptionAvailabilityTarget::Constrained => {
                MethodAssetConsumptionMaterialState::Constrained
            }
        }
    }
}

/// Body-free controlled-consumption summary carrier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetConsumptionSummary {
    /// Marker for the accepted body-free summary.
    pub summary_marker_ref: MethodLibrarySafeMarker,
    /// Formal version consumed by the downstream boundary.
    pub formal_version_ref: FormalMethodAssetVersionRef,
    /// Definition source for the consumed material.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Downstream boundary that governs the material.
    pub boundary_ref: DownstreamConsumptionBoundaryRef,
}

impl MethodAssetConsumptionSummary {
    /// Creates a body-free controlled-consumption summary.
    pub fn new(
        summary_marker_ref: MethodLibrarySafeMarker,
        formal_version_ref: FormalMethodAssetVersionRef,
        definition_ref: MethodAssetDefinitionRef,
        boundary_ref: DownstreamConsumptionBoundaryRef,
    ) -> Self {
        Self {
            summary_marker_ref,
            formal_version_ref,
            definition_ref,
            boundary_ref,
        }
    }
}

/// Current-boundary formal version requirement states for controlled consumption.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormalVersionRequiredState {
    /// Only active formal versions are accepted.
    ActiveOnly,
    /// Historical reads may use active or superseded formal versions.
    ActiveOrSupersededHistoricalRead,
}

impl FormalVersionRequiredState {
    /// Returns whether the requirement accepts the provided formal-version state.
    pub const fn accepts(self, version_state: FormalMethodAssetVersionState) -> bool {
        match self {
            Self::ActiveOnly => matches!(version_state, FormalMethodAssetVersionState::Active),
            Self::ActiveOrSupersededHistoricalRead => matches!(
                version_state,
                FormalMethodAssetVersionState::Active | FormalMethodAssetVersionState::Superseded
            ),
        }
    }
}

/// Body-free formal version requirement carrier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalVersionRequirement {
    /// Required formal-version state family.
    pub required_state: FormalVersionRequiredState,
    /// Safe requirement marker.
    pub requirement_marker_ref: MethodLibrarySafeMarker,
}

impl FormalVersionRequirement {
    /// Creates a body-free formal version requirement.
    pub fn new(
        required_state: FormalVersionRequiredState,
        requirement_marker_ref: MethodLibrarySafeMarker,
    ) -> Self {
        Self {
            required_state,
            requirement_marker_ref,
        }
    }
}

/// Current-boundary downstream use families.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodAssetAllowedUseKind {
    /// Read an existing controlled material.
    Read,
    /// Reference an existing formal version or material.
    Reference,
    /// Assemble a downstream use package from controlled material.
    Assemble,
    /// Distribute controlled material further downstream.
    Distribute,
}

/// Deterministic allowed-use-kind set.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetAllowedUseKindSet {
    /// Allowed downstream use families in first-seen order after canonical dedup.
    pub allowed_kinds: Vec<MethodAssetAllowedUseKind>,
}

impl MethodAssetAllowedUseKindSet {
    /// Creates an empty allowed-use-kind set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a deterministic set from allowed use kinds.
    pub fn from_kinds(kinds: impl IntoIterator<Item = MethodAssetAllowedUseKind>) -> Self {
        let mut set = Self::new();
        for next in kinds {
            set.insert(next);
        }
        set
    }

    /// Inserts an allowed use kind if it has not already been accepted.
    pub fn insert(&mut self, next: MethodAssetAllowedUseKind) {
        push_unique(&mut self.allowed_kinds, next);
    }

    /// Returns whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.allowed_kinds.is_empty()
    }
}

/// Current-boundary forbidden downstream truth writeback families.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownstreamForbiddenWriteKind {
    /// Downstream must not rewrite definition truth.
    DefinitionTruth,
    /// Downstream must not rewrite formal-version truth.
    FormalVersionTruth,
    /// Downstream must not rewrite controlled material truth.
    ConsumptionMaterialTruth,
    /// Downstream must not rewrite trace or audit truth.
    TraceAuditTruth,
    /// Downstream must not rewrite lineage truth.
    LineageTruth,
}

/// Deterministic forbidden-write-kind set.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DownstreamForbiddenWriteKindSet {
    /// Forbidden truth writeback families in first-seen order after canonical dedup.
    pub forbidden_kinds: Vec<DownstreamForbiddenWriteKind>,
}

impl DownstreamForbiddenWriteKindSet {
    /// Creates an empty forbidden-write-kind set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a deterministic set from forbidden write kinds.
    pub fn from_kinds(kinds: impl IntoIterator<Item = DownstreamForbiddenWriteKind>) -> Self {
        let mut set = Self::new();
        for next in kinds {
            set.insert(next);
        }
        set
    }

    /// Inserts a forbidden write kind if it has not already been accepted.
    pub fn insert(&mut self, next: DownstreamForbiddenWriteKind) {
        push_unique(&mut self.forbidden_kinds, next);
    }

    /// Returns whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.forbidden_kinds.is_empty()
    }
}
