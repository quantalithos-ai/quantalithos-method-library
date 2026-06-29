//! Shared typed boundary references for method library public contracts.

use serde::{Deserialize, Serialize};

/// The typed family of a body-free public reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodLibraryTypedBoundaryRefKind {
    /// Stable identity for method asset definitions.
    MethodAssetDefinitionRef,
    /// Public scope for catalog reads.
    CatalogScopeRef,
    /// Body-free governance basis anchor.
    GovernanceBasisRef,
    /// Controlled consumption context anchor.
    ConsumptionContextRef,
    /// Trace or audit subject anchor.
    TraceSubjectRef,
    /// Consumption impact source anchor.
    ConsumptionImpactSourceRef,
    /// Related method asset anchor.
    RelatedMethodAssetRef,
    /// Distribution boundary anchor.
    MethodAssetDistributionRef,
    /// Distribution context anchor.
    DistributionContextRef,
    /// External source anchor.
    ExternalSourceRef,
    /// Artifact or archive anchor.
    ArtifactArchiveRef,
    /// Maintenance run anchor.
    MaintenanceRunRef,
    /// Refresh scope anchor.
    RefreshScopeRef,
    /// Peripheral package anchor.
    MethodPackageRef,
    /// Method set assembly anchor.
    MethodSetAssemblyRef,
    /// Marketplace context anchor.
    MarketplaceContextRef,
}

/// Shared typed boundary reference used by public shells.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodLibraryTypedBoundaryRef {
    /// The typed family of the opaque reference.
    pub kind: MethodLibraryTypedBoundaryRefKind,
    /// The public opaque reference value.
    pub public_ref: String,
}

impl MethodLibraryTypedBoundaryRef {
    /// Creates a new typed boundary reference.
    pub fn new(kind: MethodLibraryTypedBoundaryRefKind, public_ref: impl Into<String>) -> Self {
        Self {
            kind,
            public_ref: public_ref.into(),
        }
    }

    /// Creates a typed boundary reference from a verified source.
    pub fn from_verified_source(
        kind: MethodLibraryTypedBoundaryRefKind,
        public_ref: impl Into<String>,
    ) -> Self {
        Self::new(kind, public_ref)
    }

    /// Creates a typed boundary reference from a local domain identity.
    pub fn from_domain_identity(
        kind: MethodLibraryTypedBoundaryRefKind,
        public_ref: impl Into<String>,
    ) -> Self {
        Self::new(kind, public_ref)
    }

    /// Creates a typed boundary reference from an external summary anchor.
    pub fn from_external_summary(
        kind: MethodLibraryTypedBoundaryRefKind,
        public_ref: impl Into<String>,
    ) -> Self {
        Self::new(kind, public_ref)
    }

    /// Returns the typed family.
    pub fn kind(&self) -> MethodLibraryTypedBoundaryRefKind {
        self.kind
    }

    /// Returns the public opaque reference.
    pub fn as_public_ref(&self) -> &str {
        &self.public_ref
    }

    /// Returns whether the reference matches the expected kind.
    pub fn assert_kind(&self, expected_kind: MethodLibraryTypedBoundaryRefKind) -> bool {
        self.kind == expected_kind
    }
}
