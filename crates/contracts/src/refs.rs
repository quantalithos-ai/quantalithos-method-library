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
    /// Body-free formalization basis-summary anchor.
    FormalizationBasisSummary,
    /// Body-free formalization state anchor.
    FormalizationState,
    /// Body-free formal method-version anchor.
    FormalMethodAssetVersion,
    /// Body-free formalization eligibility-rule anchor.
    FormalizationEligibilityRule,
    /// Body-free formalization eligibility-rejection anchor.
    FormalizationEligibilityRejection,
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
    /// Application-owned operation context anchor.
    MethodAssetOperationContext,
    /// Application-owned idempotency key anchor.
    MethodAssetIdempotencyKey,
    /// Application-owned operation digest anchor.
    MethodAssetOperationDigest,
    /// Application-owned dedup scope anchor.
    MethodAssetDedupScope,
    /// Stored operation result anchor.
    MethodAssetStoredOperationResult,
    /// Accepted operation summary anchor.
    MethodAssetAcceptedOperationSummary,
    /// Safe reject reason anchor.
    MethodAssetSafeRejectReason,
    /// Safe ignore reason anchor.
    MethodAssetSafeIgnoreReason,
    /// Effect summary anchor.
    MethodAssetEffectSummary,
    /// Replay marker anchor.
    MethodAssetReplayMarker,
    /// Application dispatch anchor.
    MethodAssetApplicationDispatch,
    /// API entry context anchor.
    MethodAssetApiEntryContext,
    /// Definition establish intent selector.
    MethodAssetDefinitionEstablishIntent,
    /// Definition adjust intent selector.
    MethodAssetDefinitionAdjustIntent,
    /// Definition retire intent selector.
    MethodAssetDefinitionRetireIntent,
    /// Catalog register intent selector.
    MethodAssetCatalogEntryRegisterIntent,
    /// Catalog reclassify intent selector.
    MethodAssetCatalogEntryReclassifyIntent,
    /// Catalog retire intent selector.
    MethodAssetCatalogEntryRetireIntent,
    /// Formalization eligibility evaluate intent selector.
    MethodAssetFormalizationEligibilityEvaluateIntent,
    /// Formalization initiate intent selector.
    MethodAssetFormalizationInitiateIntent,
    /// Formal method-version establish intent selector.
    FormalMethodAssetVersionEstablishIntent,
    /// Formal method-version semantic-change record intent selector.
    FormalMethodAssetVersionSemanticChangeRecordIntent,
    /// Formal method-version supersede intent selector.
    FormalMethodAssetVersionSupersedeIntent,
    /// Formal method-version retire intent selector.
    FormalMethodAssetVersionRetireIntent,
}

macro_rules! named_typed_boundary_ref {
    ($name:ident, $kind:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
        pub struct $name {
            /// The shared typed boundary ref.
            pub boundary_ref: MethodLibraryTypedBoundaryRef,
        }

        impl $name {
            /// Creates a named wrapper with the exact current-boundary kind.
            pub fn new(public_ref: impl Into<String>) -> Self {
                Self {
                    boundary_ref: MethodLibraryTypedBoundaryRef::new(
                        MethodLibraryTypedBoundaryRefKind::$kind,
                        public_ref,
                    ),
                }
            }

            /// Returns the exact current-boundary kind.
            pub const fn expected_kind() -> MethodLibraryTypedBoundaryRefKind {
                MethodLibraryTypedBoundaryRefKind::$kind
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

        impl From<$name> for MethodLibraryTypedBoundaryRef {
            fn from(value: $name) -> Self {
                value.boundary_ref
            }
        }

        impl TryFrom<MethodLibraryTypedBoundaryRef> for $name {
            type Error = MethodLibraryTypedBoundaryRefKindMismatch;

            fn try_from(value: MethodLibraryTypedBoundaryRef) -> Result<Self, Self::Error> {
                if value.assert_kind(MethodLibraryTypedBoundaryRefKind::$kind) {
                    Ok(Self {
                        boundary_ref: value,
                    })
                } else {
                    Err(MethodLibraryTypedBoundaryRefKindMismatch::new(
                        MethodLibraryTypedBoundaryRefKind::$kind,
                        value.kind(),
                    ))
                }
            }
        }
    };
}

named_typed_boundary_ref!(
    GovernanceBasisRef,
    GovernanceBasisRef,
    "Named typed boundary ref for a governance basis."
);
named_typed_boundary_ref!(
    MethodAssetDefinitionRef,
    MethodAssetDefinition,
    "Named typed boundary ref for a method asset definition."
);
named_typed_boundary_ref!(
    MethodAssetCatalogEntryRef,
    MethodAssetCatalogEntry,
    "Named typed boundary ref for a method asset catalog entry."
);
named_typed_boundary_ref!(
    FormalizationBasisSummaryRef,
    FormalizationBasisSummary,
    "Named typed boundary ref for a formalization basis summary."
);
named_typed_boundary_ref!(
    FormalizationStateRef,
    FormalizationState,
    "Named typed boundary ref for a formalization state."
);
named_typed_boundary_ref!(
    FormalMethodAssetVersionRef,
    FormalMethodAssetVersion,
    "Named typed boundary ref for a formal method asset version."
);
named_typed_boundary_ref!(
    FormalizationEligibilityRuleRef,
    FormalizationEligibilityRule,
    "Named typed boundary ref for a formalization eligibility rule."
);
named_typed_boundary_ref!(
    FormalizationEligibilityRejectionRef,
    FormalizationEligibilityRejection,
    "Named typed boundary ref for a formalization eligibility rejection."
);
named_typed_boundary_ref!(
    CatalogScopeRef,
    CatalogScope,
    "Named typed boundary ref for a catalog scope."
);
named_typed_boundary_ref!(
    ExternalSourceSummaryRef,
    ExternalSourceSummary,
    "Named typed boundary ref for an external source summary."
);
named_typed_boundary_ref!(
    MethodAssetOperationContextRef,
    MethodAssetOperationContext,
    "Named typed boundary ref for an operation context."
);
named_typed_boundary_ref!(
    MethodAssetIdempotencyKeyRef,
    MethodAssetIdempotencyKey,
    "Named typed boundary ref for an application-owned idempotency key."
);
named_typed_boundary_ref!(
    MethodAssetOperationDigestRef,
    MethodAssetOperationDigest,
    "Named typed boundary ref for an operation digest."
);
named_typed_boundary_ref!(
    MethodAssetDedupScopeRef,
    MethodAssetDedupScope,
    "Named typed boundary ref for a dedup scope."
);
named_typed_boundary_ref!(
    MethodAssetStoredOperationResultRef,
    MethodAssetStoredOperationResult,
    "Named typed boundary ref for a stored operation result."
);
named_typed_boundary_ref!(
    MethodAssetAcceptedOperationSummaryRef,
    MethodAssetAcceptedOperationSummary,
    "Named typed boundary ref for an accepted operation summary."
);
named_typed_boundary_ref!(
    MethodAssetSafeRejectReasonRef,
    MethodAssetSafeRejectReason,
    "Named typed boundary ref for a safe reject reason."
);
named_typed_boundary_ref!(
    MethodAssetSafeIgnoreReasonRef,
    MethodAssetSafeIgnoreReason,
    "Named typed boundary ref for a safe ignore reason."
);
named_typed_boundary_ref!(
    MethodAssetEffectSummaryRef,
    MethodAssetEffectSummary,
    "Named typed boundary ref for an effect summary."
);
named_typed_boundary_ref!(
    MethodAssetReplayMarkerRef,
    MethodAssetReplayMarker,
    "Named typed boundary ref for a replay marker."
);
named_typed_boundary_ref!(
    MethodAssetApplicationDispatchRef,
    MethodAssetApplicationDispatch,
    "Named typed boundary ref for an application dispatch target."
);
named_typed_boundary_ref!(
    MethodAssetApiEntryContextRef,
    MethodAssetApiEntryContext,
    "Named typed boundary ref for an API entry context."
);

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
