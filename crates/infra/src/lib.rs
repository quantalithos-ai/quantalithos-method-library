//! Infrastructure skeleton for the method library workspace.

pub mod definition_catalog;
pub mod distribution_handoff;
pub mod formalization_version;
pub mod config {}
pub mod runtime_builder {}
pub mod repositories {}
pub mod material_stores {}
pub mod reference_stores {}
pub mod external_adapters {}
pub mod publishers {}
pub mod handoff_adapters {}
pub mod clock_id {}
pub mod errors {}

pub use definition_catalog::{
    InMemoryMethodAssetCatalogEntryRepository, InMemoryMethodAssetDefinitionCatalogRuntime,
    InMemoryMethodAssetDefinitionCatalogSupportRefFactory, InMemoryMethodAssetDefinitionRepository,
    InMemoryMethodAssetStoredOperationResultRepository, InMemoryUnitOfWorkFactory,
};
pub use distribution_handoff::{
    InMemoryDistributionHandoffStoredOperationResultRepository,
    InMemoryDistributionHandoffUnitOfWorkFactory, InMemoryDistributionReadMaterialBuilderPort,
    InMemoryMethodAssetAdapterAvailabilityPort, InMemoryMethodAssetCollaborationHandoffPort,
    InMemoryMethodAssetCollaborationTargetRegistryPort,
    InMemoryMethodAssetDistributionHandoffRuntime,
    InMemoryMethodAssetDistributionHandoffSupportRefFactory,
    InMemoryMethodAssetDistributionRepository, InMemoryMethodAssetEventCandidateAssemblyRepository,
    InMemoryMethodAssetEventCandidatePublisherPort, InMemoryMethodAssetHandoffMarkerRepository,
    InMemoryMethodAssetPublicationOutcomeRepository, InMemoryMethodAssetRelationRepository,
};
pub use formalization_version::{
    InMemoryFormalMethodAssetVersionRepository, InMemoryFormalizationBasisSummaryRepository,
    InMemoryFormalizationStateRepository, InMemoryMethodAssetFormalizationVersionRuntime,
    InMemoryMethodAssetFormalizationVersionSupportRefFactory,
};
