//! Definition and catalog support carriers closed for `commit-03-a`.

use serde::{Deserialize, Serialize};

use crate::refs::{
    CatalogScopeRef, ExternalSourceSummaryRef, MethodAssetCatalogEntryRef,
    MethodLibraryTypedBoundaryRef,
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

/// Current-boundary definition kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodAssetDefinitionKind {
    /// SPEM method content.
    SpemMethodContent,
    /// Process template definition.
    ProcessTemplate,
    /// Lifecycle model definition.
    LifecycleModel,
    /// View profile definition.
    ViewProfile,
    /// AI policy definition.
    AiPolicy,
    /// AI objective definition.
    AiObjective,
    /// Relation semantic definition.
    RelationSemantic,
    /// Distribution semantic definition.
    DistributionSemantic,
    /// External summary link definition.
    ExternalSummaryLink,
    /// Package organization definition.
    PackageOrganization,
    /// Method-set assembly definition.
    MethodSetAssembly,
    /// Standard mapping material definition.
    StandardMappingMaterial,
}

/// Stable identity key for a method asset definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetIdentityKey {
    /// Current-boundary definition kind.
    pub definition_kind: MethodAssetDefinitionKind,
    /// Namespace anchor chosen by identity rules.
    pub identity_namespace_ref: MethodLibraryTypedBoundaryRef,
    /// Anchor chosen by identity rules.
    pub identity_anchor_ref: MethodLibraryTypedBoundaryRef,
    /// Applicability scope carried into the stable identity.
    pub applicability_scope_ref: CatalogScopeRef,
}

impl MethodAssetIdentityKey {
    /// Creates a body-free stable identity key.
    pub fn new(
        definition_kind: MethodAssetDefinitionKind,
        identity_namespace_ref: MethodLibraryTypedBoundaryRef,
        identity_anchor_ref: MethodLibraryTypedBoundaryRef,
        applicability_scope_ref: CatalogScopeRef,
    ) -> Self {
        Self {
            definition_kind,
            identity_namespace_ref,
            identity_anchor_ref,
            applicability_scope_ref,
        }
    }
}

/// Body-free definition summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetDefinitionSummary {
    /// Public summary anchor.
    pub summary_ref: MethodLibraryTypedBoundaryRef,
    /// Current-boundary definition kind.
    pub definition_kind: MethodAssetDefinitionKind,
    /// Safe title anchor.
    pub safe_title_ref: MethodLibraryTypedBoundaryRef,
    /// Optional safe description anchor.
    pub safe_description_ref: Option<MethodLibraryTypedBoundaryRef>,
    /// Public-safe summary marker.
    pub summary_marker_ref: MethodLibrarySafeMarker,
}

impl MethodAssetDefinitionSummary {
    /// Creates a body-free definition summary.
    pub fn new(
        summary_ref: MethodLibraryTypedBoundaryRef,
        definition_kind: MethodAssetDefinitionKind,
        safe_title_ref: MethodLibraryTypedBoundaryRef,
        safe_description_ref: Option<MethodLibraryTypedBoundaryRef>,
        summary_marker_ref: MethodLibrarySafeMarker,
    ) -> Self {
        Self {
            summary_ref,
            definition_kind,
            safe_title_ref,
            safe_description_ref,
            summary_marker_ref,
        }
    }
}

/// Deterministic accepted external-summary ref set.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExternalSourceSummaryRefSet {
    /// Accepted summary refs ordered by first insertion after canonical dedup.
    pub refs: Vec<ExternalSourceSummaryRef>,
}

impl ExternalSourceSummaryRefSet {
    /// Creates an empty ref set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a deterministic set from refs.
    pub fn from_refs(refs: impl IntoIterator<Item = ExternalSourceSummaryRef>) -> Self {
        let mut set = Self::new();
        for next in refs {
            set.insert(next);
        }
        set
    }

    /// Inserts a ref if it has not already been accepted.
    pub fn insert(&mut self, next: ExternalSourceSummaryRef) {
        push_unique(&mut self.refs, next);
    }
}

/// Deterministic catalog-entry ref set.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetCatalogEntryRefSet {
    /// Linked catalog-entry refs ordered by first insertion after canonical dedup.
    pub refs: Vec<MethodAssetCatalogEntryRef>,
}

impl MethodAssetCatalogEntryRefSet {
    /// Creates an empty ref set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a deterministic set from refs.
    pub fn from_refs(refs: impl IntoIterator<Item = MethodAssetCatalogEntryRef>) -> Self {
        let mut set = Self::new();
        for next in refs {
            set.insert(next);
        }
        set
    }

    /// Inserts a ref if it has not already been linked.
    pub fn insert(&mut self, next: MethodAssetCatalogEntryRef) {
        push_unique(&mut self.refs, next);
    }
}

/// Body-free catalog classification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetCatalogClassification {
    /// Definition kind carried into the catalog classification.
    pub definition_kind: MethodAssetDefinitionKind,
    /// Catalog scope for the classification.
    pub catalog_scope_ref: CatalogScopeRef,
    /// Classification marker copied from accepted inputs.
    pub classification_marker_ref: MethodLibrarySafeMarker,
}

impl MethodAssetCatalogClassification {
    /// Creates a body-free catalog classification.
    pub fn new(
        definition_kind: MethodAssetDefinitionKind,
        catalog_scope_ref: CatalogScopeRef,
        classification_marker_ref: MethodLibrarySafeMarker,
    ) -> Self {
        Self {
            definition_kind,
            catalog_scope_ref,
            classification_marker_ref,
        }
    }
}

/// Body-free applicability summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetApplicabilitySummary {
    /// Scope that anchors the applicability summary.
    pub applicability_scope_ref: CatalogScopeRef,
    /// Applicability marker copied from accepted inputs.
    pub applicability_marker_ref: MethodLibrarySafeMarker,
    /// Safe applicable context refs ordered by first insertion after canonical dedup.
    pub applicable_context_refs: Vec<MethodLibraryTypedBoundaryRef>,
}

impl MethodAssetApplicabilitySummary {
    /// Creates a body-free applicability summary.
    pub fn new(
        applicability_scope_ref: CatalogScopeRef,
        applicability_marker_ref: MethodLibrarySafeMarker,
        applicable_context_refs: impl IntoIterator<Item = MethodLibraryTypedBoundaryRef>,
    ) -> Self {
        let mut refs = Vec::new();
        for next in applicable_context_refs {
            push_unique(&mut refs, next);
        }

        Self {
            applicability_scope_ref,
            applicability_marker_ref,
            applicable_context_refs: refs,
        }
    }
}

/// Current-boundary catalog public/truth summary status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodAssetCatalogEntryStatus {
    /// The entry exists but is not yet visible.
    Pending,
    /// The entry is visible for current discovery.
    Visible,
    /// The entry remains hidden from current discovery.
    Hidden,
    /// The entry is deprecated but historically readable.
    Deprecated,
    /// The entry is retired and historical only.
    Retired,
}
