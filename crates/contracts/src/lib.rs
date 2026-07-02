//! Public contract skeleton for the method library workspace.

pub mod commands;
pub mod definition_catalog;
pub mod errors;
pub mod events;
pub mod fixtures;
pub mod jobs;
pub mod metadata;
pub mod queries;
pub mod refs;
pub mod views;

pub use commands::{MethodLibraryCapabilityKind, MethodLibraryCommandShell};
pub use definition_catalog::{
    ExternalSourceSummaryRefSet, MethodAssetApplicabilitySummary, MethodAssetCatalogClassification,
    MethodAssetCatalogEntryRefSet, MethodAssetCatalogEntryStatus, MethodAssetDefinitionKind,
    MethodAssetDefinitionSummary, MethodAssetIdentityKey,
};
pub use events::MethodLibraryEventShell;
pub use jobs::{MethodLibraryJobShell, MethodLibraryOperationsJobKind};
pub use queries::MethodLibraryQueryShell;
pub use refs::{
    CatalogScopeRef, ExternalSourceSummaryRef, MethodAssetAcceptedOperationSummaryRef,
    MethodAssetApiEntryContextRef, MethodAssetApplicationDispatchRef, MethodAssetCatalogEntryRef,
    MethodAssetDedupScopeRef, MethodAssetDefinitionRef, MethodAssetEffectSummaryRef,
    MethodAssetIdempotencyKeyRef, MethodAssetOperationContextRef, MethodAssetOperationDigestRef,
    MethodAssetReplayMarkerRef, MethodAssetSafeIgnoreReasonRef, MethodAssetSafeRejectReasonRef,
    MethodAssetStoredOperationResultRef, MethodLibraryTypedBoundaryRef,
    MethodLibraryTypedBoundaryRefKind, MethodLibraryTypedBoundaryRefKindMismatch,
};
pub use views::{
    MethodLibraryPublicShell, MethodLibrarySafeMarker, MethodLibrarySafeMarkerKind,
    MethodLibraryShellKind, MethodLibraryViewShell,
};
