//! Trace, impact, audit, and evidence-lineage carriers closed for `commit-06-a`.

use serde::{Deserialize, Serialize};

use crate::refs::{
    MethodAssetAuditEntryRef, MethodAssetEvidenceLineageRef, MethodAssetTraceMaterialRef,
    MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
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

/// Wrong-kind rejection for the six-kind trace-source family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MethodAssetTraceSourceRefKindMismatch {
    actual_kind: MethodLibraryTypedBoundaryRefKind,
}

impl MethodAssetTraceSourceRefKindMismatch {
    /// Creates a rejection for the observed source kind.
    pub const fn new(actual_kind: MethodLibraryTypedBoundaryRefKind) -> Self {
        Self { actual_kind }
    }

    /// Returns the observed source kind.
    pub const fn actual_kind(self) -> MethodLibraryTypedBoundaryRefKind {
        self.actual_kind
    }
}

impl core::fmt::Display for MethodAssetTraceSourceRefKindMismatch {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "unsupported method asset trace source kind: {:?}",
            self.actual_kind
        )
    }
}

impl std::error::Error for MethodAssetTraceSourceRefKindMismatch {}

/// Verified body-free source reference accepted by trace material.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetTraceSourceRef {
    /// The verified shared typed boundary ref.
    pub boundary_ref: MethodLibraryTypedBoundaryRef,
}

impl MethodAssetTraceSourceRef {
    /// Returns the verified typed boundary ref.
    pub fn as_typed_ref(&self) -> &MethodLibraryTypedBoundaryRef {
        &self.boundary_ref
    }

    /// Returns the body-free public reference.
    pub fn as_public_ref(&self) -> &str {
        self.boundary_ref.as_public_ref()
    }
}

impl TryFrom<MethodLibraryTypedBoundaryRef> for MethodAssetTraceSourceRef {
    type Error = MethodAssetTraceSourceRefKindMismatch;

    fn try_from(value: MethodLibraryTypedBoundaryRef) -> Result<Self, Self::Error> {
        if matches!(
            value.kind(),
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition
                | MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntry
                | MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersion
                | MethodLibraryTypedBoundaryRefKind::MethodAssetConsumptionMaterial
                | MethodLibraryTypedBoundaryRefKind::MethodAssetRelation
                | MethodLibraryTypedBoundaryRefKind::ExternalSourceSummary
        ) {
            Ok(Self {
                boundary_ref: value,
            })
        } else {
            Err(MethodAssetTraceSourceRefKindMismatch::new(value.kind()))
        }
    }
}

/// Deterministic trace-source ref set.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetTraceSourceRefSet {
    /// Trace-source refs ordered by first insertion after typed equality deduplication.
    pub refs: Vec<MethodAssetTraceSourceRef>,
}

impl MethodAssetTraceSourceRefSet {
    /// Creates an empty ref set.
    pub fn new() -> Self {
        Self { refs: Vec::new() }
    }

    /// Creates a deterministic set from trace-source refs.
    pub fn from_refs(refs: impl IntoIterator<Item = MethodAssetTraceSourceRef>) -> Self {
        let mut set = Self::new();
        for next in refs {
            set.insert(next);
        }
        set
    }

    /// Inserts a ref if typed equality has not already accepted it.
    pub fn insert(&mut self, next: MethodAssetTraceSourceRef) {
        push_unique(&mut self.refs, next);
    }

    /// Returns whether the set contains no refs.
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }
}

/// Deterministic trace-material ref set.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetTraceMaterialRefSet {
    /// Trace-material refs ordered by first insertion after typed equality deduplication.
    pub refs: Vec<MethodAssetTraceMaterialRef>,
}

impl MethodAssetTraceMaterialRefSet {
    /// Creates an empty ref set.
    pub fn new() -> Self {
        Self { refs: Vec::new() }
    }

    /// Creates a deterministic set from trace-material refs.
    pub fn from_refs(refs: impl IntoIterator<Item = MethodAssetTraceMaterialRef>) -> Self {
        let mut set = Self::new();
        for next in refs {
            set.insert(next);
        }
        set
    }

    /// Inserts a ref if typed equality has not already accepted it.
    pub fn insert(&mut self, next: MethodAssetTraceMaterialRef) {
        push_unique(&mut self.refs, next);
    }

    /// Returns whether the set contains no refs.
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }
}

/// Deterministic safe audit-entry ref set.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetAuditEntryRefSet {
    /// Audit-entry refs ordered by first insertion after typed equality deduplication.
    pub refs: Vec<MethodAssetAuditEntryRef>,
}

impl MethodAssetAuditEntryRefSet {
    /// Creates an empty ref set.
    pub fn new() -> Self {
        Self { refs: Vec::new() }
    }

    /// Creates a deterministic set from audit-entry refs.
    pub fn from_refs(refs: impl IntoIterator<Item = MethodAssetAuditEntryRef>) -> Self {
        let mut set = Self::new();
        for next in refs {
            set.insert(next);
        }
        set
    }

    /// Inserts a ref if typed equality has not already accepted it.
    pub fn insert(&mut self, next: MethodAssetAuditEntryRef) {
        push_unique(&mut self.refs, next);
    }

    /// Returns whether the set contains no refs.
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }
}

/// Deterministic evidence-lineage ref set.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetEvidenceLineageRefSet {
    /// Evidence-lineage refs ordered by first insertion after typed equality deduplication.
    pub refs: Vec<MethodAssetEvidenceLineageRef>,
}

impl MethodAssetEvidenceLineageRefSet {
    /// Creates an empty ref set.
    pub fn new() -> Self {
        Self { refs: Vec::new() }
    }

    /// Creates a deterministic set from evidence-lineage refs.
    pub fn from_refs(refs: impl IntoIterator<Item = MethodAssetEvidenceLineageRef>) -> Self {
        let mut set = Self::new();
        for next in refs {
            set.insert(next);
        }
        set
    }

    /// Inserts a ref if typed equality has not already accepted it.
    pub fn insert(&mut self, next: MethodAssetEvidenceLineageRef) {
        push_unique(&mut self.refs, next);
    }

    /// Returns whether the set contains no refs.
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }
}

/// Named safe-marker wrapper for a PH-06 safe reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetSafeReasonRef {
    /// Safe marker copied from a formal body-free source.
    pub safe_marker: MethodLibrarySafeMarker,
}

impl MethodAssetSafeReasonRef {
    /// Creates a named wrapper over an existing safe marker.
    pub fn new(safe_marker: MethodLibrarySafeMarker) -> Self {
        Self { safe_marker }
    }

    /// Returns the wrapped safe marker.
    pub fn as_safe_marker(&self) -> &MethodLibrarySafeMarker {
        &self.safe_marker
    }
}

/// Body-free trace summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetTraceSummary {
    /// Marker for the accepted safe summary.
    pub summary_marker_ref: MethodLibrarySafeMarker,
    /// Marker for the accepted trace coverage.
    pub coverage_marker_ref: MethodLibrarySafeMarker,
}

/// Body-free consumption impact summary details.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConsumptionImpactSafeSummary {
    /// Marker for the accepted safe summary.
    pub summary_marker_ref: MethodLibrarySafeMarker,
    /// Optional disposition marker copied by the domain guard.
    pub disposition_marker_ref: Option<MethodLibrarySafeMarker>,
    /// Optional safe reason copied by the domain guard.
    pub safe_reason_ref: Option<MethodAssetSafeReasonRef>,
}

/// Body-free evidence-lineage summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetEvidenceLineageSummary {
    /// Marker for the accepted safe summary.
    pub summary_marker_ref: MethodLibrarySafeMarker,
    /// Optional safe reason for partial or rejected lineage.
    pub safe_reason_ref: Option<MethodAssetSafeReasonRef>,
}

/// Trace-material lifecycle states.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodAssetTraceMaterialState {
    /// Trace material has complete formal source anchors.
    Organized,
    /// Trace material has only partial safe source coverage.
    Partial,
    /// Trace material is stale relative to its source truth.
    Stale,
    /// Trace material cannot currently be organized safely.
    Unavailable,
}

/// Consumption impact categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsumptionImpactKind {
    /// A body-free impact is known.
    KnownImpact,
    /// The impact remains explicitly unknown.
    UnknownImpact,
    /// A formal downstream safe summary remains pending.
    PendingDownstreamSummary,
    /// No effect is known from the accepted sources.
    NoKnownEffect,
}

/// Consumption impact summary lifecycle states.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsumptionImpactSummaryState {
    /// The summary is current and has no disposition marker.
    Current,
    /// An explicit safe disposition was recorded.
    DispositionMarked,
    /// A distinct next summary superseded this summary.
    Superseded,
}

/// Audit-trail support states.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodAssetAuditTrailState {
    /// The trail owner exists without an appended entry.
    TrailOwnerPresent,
    /// One or more safe entry refs were appended.
    SafeEntryRefsAppended,
    /// Only a partial audit surface is available.
    PartialAuditAvailable,
    /// The audit surface is unavailable.
    AuditUnavailable,
}

/// Evidence-lineage support states.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodAssetEvidenceLineageState {
    /// External and basis lineage anchors are linked.
    LineageLinked,
    /// The lineage remains linked with an explicit partial reason.
    LineagePartial,
    /// The lineage cannot currently be assembled safely.
    LineageUnavailable,
    /// A body candidate was rejected without being retained.
    BodyCandidateRejected,
}
