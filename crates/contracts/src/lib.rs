//! Public contract skeleton for the method library workspace.

pub mod commands;
pub mod consumption;
pub mod definition_catalog;
pub mod distribution;
pub mod errors;
pub mod events;
pub mod fixtures;
pub mod formalization;
pub mod jobs;
pub mod metadata;
pub mod queries;
pub mod refs;
pub mod trace_audit;
pub mod views;

pub use commands::{MethodLibraryCapabilityKind, MethodLibraryCommandShell};
pub use consumption::{
    ConsumptionBoundaryReasonRef, DefinitionUseBoundaryGuardState, DefinitionUseGuardReasonRef,
    DownstreamConsumptionBoundaryState, DownstreamForbiddenWriteKind,
    DownstreamForbiddenWriteKindSet, FormalVersionRequiredState, FormalVersionRequirement,
    MethodAssetAllowedUseKind, MethodAssetAllowedUseKindSet,
    MethodAssetConsumptionAvailabilityMarker, MethodAssetConsumptionAvailabilityMarkerSource,
    MethodAssetConsumptionAvailabilityTarget, MethodAssetConsumptionMaterialState,
    MethodAssetConsumptionSummary,
};
pub use definition_catalog::{
    ExternalSourceSummaryRefSet, MethodAssetApplicabilitySummary, MethodAssetCatalogClassification,
    MethodAssetCatalogEntryRefSet, MethodAssetCatalogEntryStatus, MethodAssetDefinitionKind,
    MethodAssetDefinitionSummary, MethodAssetIdentityKey,
};
pub use distribution::{
    MethodAssetAdapterSlotRefSet, MethodAssetDistributionAdjustmentReasonRef,
    MethodAssetEventCandidateReasonRef, MethodAssetHandoffBoundaryMarkerRef,
    MethodAssetHandoffTargetRefSet, MethodAssetPublicationBoundaryMarkerRef,
};
pub use events::MethodLibraryEventShell;
pub use formalization::{
    ForbiddenFormalizationTriggerKind, ForbiddenFormalizationTriggerKindSet,
    FormalMethodAssetVersionRefSet, FormalMethodAssetVersionState, FormalVersionBoundarySummary,
    FormalizationBasisKind, FormalizationBasisKindSet, FormalizationBasisRequirement,
    FormalizationBasisSafeSummary, FormalizationBasisSummaryRefSet, FormalizationStateKind,
    FormalizationStateReasonSummary, MethodAssetDefinitionRequirement,
    OptionalGovernanceBasisRequirement,
};
pub use jobs::{MethodLibraryJobShell, MethodLibraryOperationsJobKind};
pub use queries::MethodLibraryQueryShell;
pub use refs::{
    CatalogScopeRef, ConsumptionContextRef, ConsumptionImpactSourceRef,
    ConsumptionImpactSummaryRef, DefinitionUseBoundaryGuardRef, DistributionContextRef,
    DownstreamConsumptionBoundaryRef, ExternalSourceSummaryRef, FormalMethodAssetVersionRef,
    FormalizationBasisSummaryRef, FormalizationEligibilityRejectionRef,
    FormalizationEligibilityRuleRef, FormalizationStateRef, GovernanceBasisRef,
    MethodAssetAcceptedOperationSummaryRef, MethodAssetAdapterAvailabilityStateRef,
    MethodAssetAdapterSlotRef, MethodAssetApiEntryContextRef, MethodAssetApplicationDispatchRef,
    MethodAssetAuditCursorRef, MethodAssetAuditEntryRef, MethodAssetAuditTrailRef,
    MethodAssetCatalogEntryRef, MethodAssetConsumptionMaterialCursorRef,
    MethodAssetConsumptionMaterialRef, MethodAssetConsumptionMaterialScopeRef,
    MethodAssetDedupScopeRef, MethodAssetDefinitionRef, MethodAssetDegradedDecisionRef,
    MethodAssetDistributionRef, MethodAssetEffectSummaryRef, MethodAssetEventCandidateAssemblyRef,
    MethodAssetEvidenceLineageRef, MethodAssetHandoffBindingStateRef, MethodAssetHandoffMarkerRef,
    MethodAssetHandoffTargetRef, MethodAssetIdempotencyKeyRef, MethodAssetInfraSafeDiagnosticRef,
    MethodAssetOperationContextRef, MethodAssetOperationDigestRef,
    MethodAssetPublicationOutcomeRef, MethodAssetPublisherBindingStateRef, MethodAssetRelationRef,
    MethodAssetReplayMarkerRef, MethodAssetSafeIgnoreReasonRef, MethodAssetSafeRejectReasonRef,
    MethodAssetStoredOperationResultRef, MethodAssetTargetRegistryScopeRef,
    MethodAssetTraceCursorRef, MethodAssetTraceFreshnessMarkerRef, MethodAssetTraceMaterialRef,
    MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
    MethodLibraryTypedBoundaryRefKindMismatch, TraceSubjectRef,
};
pub use trace_audit::{
    ConsumptionImpactKind, ConsumptionImpactSafeSummary, ConsumptionImpactSummaryState,
    MethodAssetAuditEntryRefSet, MethodAssetAuditTrailState, MethodAssetEvidenceLineageRefSet,
    MethodAssetEvidenceLineageState, MethodAssetEvidenceLineageSummary, MethodAssetSafeReasonRef,
    MethodAssetTraceMaterialRefSet, MethodAssetTraceMaterialState, MethodAssetTraceSourceRef,
    MethodAssetTraceSourceRefKindMismatch, MethodAssetTraceSourceRefSet, MethodAssetTraceSummary,
};
pub use views::{
    MethodLibraryPublicShell, MethodLibrarySafeMarker, MethodLibrarySafeMarkerKind,
    MethodLibraryShellKind, MethodLibraryViewShell,
};
