//! Shared typed boundary references for method library public contracts.

use serde::{Deserialize, Serialize};

/// The typed family of a body-free public reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodLibraryTypedBoundaryRefKind {
    /// Stable identity for method asset definitions.
    MethodAssetDefinition,
    /// Stable identity for method asset catalog entries.
    MethodAssetCatalogEntry,
    /// Public scope for catalog reads.
    CatalogScope,
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
    /// Accepted external summary anchor.
    ExternalSourceSummary,
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

/// Named typed boundary ref for a method asset definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetDefinitionRef {
    /// The shared typed boundary ref.
    pub boundary_ref: MethodLibraryTypedBoundaryRef,
}

impl MethodAssetDefinitionRef {
    /// Creates a definition ref with the exact current-boundary kind.
    pub fn new(public_ref: impl Into<String>) -> Self {
        Self {
            boundary_ref: MethodLibraryTypedBoundaryRef::new(
                MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
                public_ref,
            ),
        }
    }

    /// Returns the exact current-boundary kind.
    pub const fn expected_kind() -> MethodLibraryTypedBoundaryRefKind {
        MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition
    }

    /// Returns the inner typed boundary ref.
    pub fn as_typed_ref(&self) -> &MethodLibraryTypedBoundaryRef {
        &self.boundary_ref
    }

    /// Returns the body-free public reference.
    pub fn as_public_ref(&self) -> &str {
        self.boundary_ref.as_public_ref()
    }
}

impl From<MethodAssetDefinitionRef> for MethodLibraryTypedBoundaryRef {
    fn from(value: MethodAssetDefinitionRef) -> Self {
        value.boundary_ref
    }
}

impl TryFrom<MethodLibraryTypedBoundaryRef> for MethodAssetDefinitionRef {
    type Error = MethodLibraryTypedBoundaryRefKindMismatch;

    fn try_from(value: MethodLibraryTypedBoundaryRef) -> Result<Self, Self::Error> {
        if value.assert_kind(MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition) {
            Ok(Self {
                boundary_ref: value,
            })
        } else {
            Err(MethodLibraryTypedBoundaryRefKindMismatch::new(
                MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
                value.kind(),
            ))
        }
    }
}

/// Named typed boundary ref for a method asset catalog entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetCatalogEntryRef {
    /// The shared typed boundary ref.
    pub boundary_ref: MethodLibraryTypedBoundaryRef,
}

impl MethodAssetCatalogEntryRef {
    /// Creates a catalog-entry ref with the exact current-boundary kind.
    pub fn new(public_ref: impl Into<String>) -> Self {
        Self {
            boundary_ref: MethodLibraryTypedBoundaryRef::new(
                MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntry,
                public_ref,
            ),
        }
    }

    /// Returns the exact current-boundary kind.
    pub const fn expected_kind() -> MethodLibraryTypedBoundaryRefKind {
        MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntry
    }

    /// Returns the inner typed boundary ref.
    pub fn as_typed_ref(&self) -> &MethodLibraryTypedBoundaryRef {
        &self.boundary_ref
    }

    /// Returns the body-free public reference.
    pub fn as_public_ref(&self) -> &str {
        self.boundary_ref.as_public_ref()
    }
}

impl From<MethodAssetCatalogEntryRef> for MethodLibraryTypedBoundaryRef {
    fn from(value: MethodAssetCatalogEntryRef) -> Self {
        value.boundary_ref
    }
}

impl TryFrom<MethodLibraryTypedBoundaryRef> for MethodAssetCatalogEntryRef {
    type Error = MethodLibraryTypedBoundaryRefKindMismatch;

    fn try_from(value: MethodLibraryTypedBoundaryRef) -> Result<Self, Self::Error> {
        if value.assert_kind(MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntry) {
            Ok(Self {
                boundary_ref: value,
            })
        } else {
            Err(MethodLibraryTypedBoundaryRefKindMismatch::new(
                MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntry,
                value.kind(),
            ))
        }
    }
}

/// Named typed boundary ref for a catalog scope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CatalogScopeRef {
    /// The shared typed boundary ref.
    pub boundary_ref: MethodLibraryTypedBoundaryRef,
}

impl CatalogScopeRef {
    /// Creates a scope ref with the exact current-boundary kind.
    pub fn new(public_ref: impl Into<String>) -> Self {
        Self {
            boundary_ref: MethodLibraryTypedBoundaryRef::new(
                MethodLibraryTypedBoundaryRefKind::CatalogScope,
                public_ref,
            ),
        }
    }

    /// Returns the exact current-boundary kind.
    pub const fn expected_kind() -> MethodLibraryTypedBoundaryRefKind {
        MethodLibraryTypedBoundaryRefKind::CatalogScope
    }

    /// Returns the inner typed boundary ref.
    pub fn as_typed_ref(&self) -> &MethodLibraryTypedBoundaryRef {
        &self.boundary_ref
    }

    /// Returns the body-free public reference.
    pub fn as_public_ref(&self) -> &str {
        self.boundary_ref.as_public_ref()
    }
}

impl From<CatalogScopeRef> for MethodLibraryTypedBoundaryRef {
    fn from(value: CatalogScopeRef) -> Self {
        value.boundary_ref
    }
}

impl TryFrom<MethodLibraryTypedBoundaryRef> for CatalogScopeRef {
    type Error = MethodLibraryTypedBoundaryRefKindMismatch;

    fn try_from(value: MethodLibraryTypedBoundaryRef) -> Result<Self, Self::Error> {
        if value.assert_kind(MethodLibraryTypedBoundaryRefKind::CatalogScope) {
            Ok(Self {
                boundary_ref: value,
            })
        } else {
            Err(MethodLibraryTypedBoundaryRefKindMismatch::new(
                MethodLibraryTypedBoundaryRefKind::CatalogScope,
                value.kind(),
            ))
        }
    }
}

/// Named typed boundary ref for an external source summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExternalSourceSummaryRef {
    /// The shared typed boundary ref.
    pub boundary_ref: MethodLibraryTypedBoundaryRef,
}

impl ExternalSourceSummaryRef {
    /// Creates an external summary ref with the exact current-boundary kind.
    pub fn new(public_ref: impl Into<String>) -> Self {
        Self {
            boundary_ref: MethodLibraryTypedBoundaryRef::new(
                MethodLibraryTypedBoundaryRefKind::ExternalSourceSummary,
                public_ref,
            ),
        }
    }

    /// Returns the exact current-boundary kind.
    pub const fn expected_kind() -> MethodLibraryTypedBoundaryRefKind {
        MethodLibraryTypedBoundaryRefKind::ExternalSourceSummary
    }

    /// Returns the inner typed boundary ref.
    pub fn as_typed_ref(&self) -> &MethodLibraryTypedBoundaryRef {
        &self.boundary_ref
    }

    /// Returns the body-free public reference.
    pub fn as_public_ref(&self) -> &str {
        self.boundary_ref.as_public_ref()
    }
}

impl From<ExternalSourceSummaryRef> for MethodLibraryTypedBoundaryRef {
    fn from(value: ExternalSourceSummaryRef) -> Self {
        value.boundary_ref
    }
}

impl TryFrom<MethodLibraryTypedBoundaryRef> for ExternalSourceSummaryRef {
    type Error = MethodLibraryTypedBoundaryRefKindMismatch;

    fn try_from(value: MethodLibraryTypedBoundaryRef) -> Result<Self, Self::Error> {
        if value.assert_kind(MethodLibraryTypedBoundaryRefKind::ExternalSourceSummary) {
            Ok(Self {
                boundary_ref: value,
            })
        } else {
            Err(MethodLibraryTypedBoundaryRefKindMismatch::new(
                MethodLibraryTypedBoundaryRefKind::ExternalSourceSummary,
                value.kind(),
            ))
        }
    }
}

/// Wrong-kind rejection for named typed-boundary wrappers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MethodLibraryTypedBoundaryRefKindMismatch {
    expected_kind: MethodLibraryTypedBoundaryRefKind,
    actual_kind: MethodLibraryTypedBoundaryRefKind,
}

impl MethodLibraryTypedBoundaryRefKindMismatch {
    /// Creates a new wrong-kind rejection.
    pub const fn new(
        expected_kind: MethodLibraryTypedBoundaryRefKind,
        actual_kind: MethodLibraryTypedBoundaryRefKind,
    ) -> Self {
        Self {
            expected_kind,
            actual_kind,
        }
    }

    /// Returns the expected current-boundary kind.
    pub const fn expected_kind(self) -> MethodLibraryTypedBoundaryRefKind {
        self.expected_kind
    }

    /// Returns the actual current-boundary kind.
    pub const fn actual_kind(self) -> MethodLibraryTypedBoundaryRefKind {
        self.actual_kind
    }
}

impl core::fmt::Display for MethodLibraryTypedBoundaryRefKindMismatch {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "expected {:?} typed boundary ref, got {:?}",
            self.expected_kind, self.actual_kind
        )
    }
}

impl std::error::Error for MethodLibraryTypedBoundaryRefKindMismatch {}

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
