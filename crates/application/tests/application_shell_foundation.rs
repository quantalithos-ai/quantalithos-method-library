use core::mem::size_of;

use method_library_application::idempotency::{
    MethodAssetIdempotencyDecisionKind, MethodAssetIdempotencyGuard, MethodAssetOperationContext,
    MethodAssetStoredOperationResult, MethodAssetStoredOperationResultKind,
};
use method_library_application::ports::*;
use method_library_application::unit_of_work::{Clock, IdGenerator, UnitOfWork};

macro_rules! assert_object_safe_shell {
    ($($trait_name:ident),+ $(,)?) => {
        $(
            let _: Option<&dyn $trait_name> = None;
        )+
    };
}

#[test]
fn shell_carriers_are_zero_sized() {
    assert_eq!(size_of::<MethodAssetOperationContext>(), 0);
    assert_eq!(size_of::<MethodAssetIdempotencyGuard>(), 0);
    assert_eq!(size_of::<MethodAssetStoredOperationResult>(), 0);
}

#[test]
fn idempotency_enums_match_current_boundary_labels() {
    let decisions = [
        MethodAssetIdempotencyDecisionKind::Fresh,
        MethodAssetIdempotencyDecisionKind::DuplicateReplay,
        MethodAssetIdempotencyDecisionKind::Conflict,
        MethodAssetIdempotencyDecisionKind::Rejected,
        MethodAssetIdempotencyDecisionKind::ReplayUnavailable,
    ];
    assert_eq!(decisions.len(), 5);

    let results = [
        MethodAssetStoredOperationResultKind::Accepted,
        MethodAssetStoredOperationResultKind::Rejected,
        MethodAssetStoredOperationResultKind::Ignored,
        MethodAssetStoredOperationResultKind::Conflict,
    ];
    assert_eq!(results.len(), 4);
}

#[test]
fn port_shells_are_object_safe() {
    assert_object_safe_shell!(
        MethodAssetDefinitionRepository,
        MethodAssetCatalogEntryRepository,
        FormalizationStateRepository,
        FormalMethodAssetVersionRepository,
        MethodAssetCommittedTruthSnapshotReader,
        FormalizationBasisSummaryRepository,
        ExternalSourceSummaryRepository,
        MethodAssetConsumptionMaterialRepository,
        MethodAssetTraceMaterialRepository,
        ConsumptionImpactSummaryRepository,
        MethodAssetAuditTrailRepository,
        MethodAssetEvidenceLineageRepository,
        MethodAssetRelationRepository,
        MethodPackageRepository,
        MethodSetAssemblyRepository,
        FormalizationBasisResolverPort,
        MethodAssetPolicyDiagnosticBuilderPort,
        MethodAssetConsumptionAvailabilityResolverPort,
        MethodAssetQueryReadResolverPort,
        MethodAssetDegradedDecisionMapperPort,
        DistributionReadMaterialBuilderPort,
        PeripheralDiscoveryContextBuilderPort,
        MarketplaceContextRefResolverPort,
        MethodAssetInboundSourcePort,
        ExternalBodyFreeSourceAdapterPort,
        MethodAssetEventCandidatePublisherPort,
        MethodAssetCollaborationHandoffPort,
        MethodAssetCollaborationTargetRegistryPort,
        MethodAssetRefreshTargetPlannerPort,
        MethodAssetJobCheckpointStorePort,
        MethodAssetRuntimeAssemblyRegistryPort,
        MethodAssetAdapterAvailabilityPort,
        UnitOfWork,
        Clock,
        IdGenerator,
    );
}
