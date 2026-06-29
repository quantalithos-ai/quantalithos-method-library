//! Shell-only application port families for the current implementation boundary.

macro_rules! shell_port {
    ($name:ident) => {
        pub trait $name: Send + Sync {}
    };
}

// Core truth repository shells.
shell_port!(MethodAssetDefinitionRepository);
shell_port!(MethodAssetCatalogEntryRepository);
shell_port!(FormalizationStateRepository);
shell_port!(FormalMethodAssetVersionRepository);
shell_port!(MethodAssetCommittedTruthSnapshotReader);

// Support, material and aggregate repository shells.
shell_port!(FormalizationBasisSummaryRepository);
shell_port!(ExternalSourceSummaryRepository);
shell_port!(MethodAssetConsumptionMaterialRepository);
shell_port!(MethodAssetTraceMaterialRepository);
shell_port!(ConsumptionImpactSummaryRepository);
shell_port!(MethodAssetAuditTrailRepository);
shell_port!(MethodAssetEvidenceLineageRepository);
shell_port!(MethodAssetRelationRepository);
shell_port!(MethodPackageRepository);
shell_port!(MethodSetAssemblyRepository);

// Resolver, mapper and builder shells.
shell_port!(FormalizationBasisResolverPort);
shell_port!(MethodAssetPolicyDiagnosticBuilderPort);
shell_port!(MethodAssetConsumptionAvailabilityResolverPort);
shell_port!(MethodAssetQueryReadResolverPort);
shell_port!(MethodAssetDegradedDecisionMapperPort);
shell_port!(DistributionReadMaterialBuilderPort);
shell_port!(PeripheralDiscoveryContextBuilderPort);
shell_port!(MarketplaceContextRefResolverPort);

// Inbound, outbound, handoff and runtime shells.
shell_port!(MethodAssetInboundSourcePort);
shell_port!(ExternalBodyFreeSourceAdapterPort);
shell_port!(MethodAssetEventCandidatePublisherPort);
shell_port!(MethodAssetCollaborationHandoffPort);
shell_port!(MethodAssetCollaborationTargetRegistryPort);
shell_port!(MethodAssetRefreshTargetPlannerPort);
shell_port!(MethodAssetJobCheckpointStorePort);
shell_port!(MethodAssetRuntimeAssemblyRegistryPort);
shell_port!(MethodAssetAdapterAvailabilityPort);
