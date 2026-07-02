//! Definition/catalog accepted-service surface for `commit-03-b`.

pub mod definition_catalog;
pub mod idempotency;
pub mod ports;
pub mod unit_of_work;

pub use definition_catalog::{
    AdjustDefinitionCommandSource, AdjustMethodAssetDefinitionInput,
    DefaultMethodAssetDefinitionCatalogCommandFacade, EstablishDefinitionCommandSource,
    EstablishMethodAssetDefinitionInput, MethodAssetDefinitionCatalogCommandDispatchInput,
    MethodAssetDefinitionCatalogCommandDispatchOutput, MethodAssetDefinitionCatalogCommandFacade,
    MethodAssetDefinitionCatalogCommandSelector, MethodAssetDefinitionCatalogCommandServiceInput,
    MethodAssetDefinitionCatalogCommandSource, MethodAssetDefinitionCatalogReplayEnvelope,
    MethodAssetDefinitionCatalogReplayEnvelopeFactoryInput,
    MethodAssetDefinitionCatalogSupportRefFactory, MethodAssetEffectSummaryRefSet,
    MethodAssetExpectedVersion, MethodAssetReplayEnvelopeBuildError, MethodAssetRepositoryError,
    MethodAssetRepositoryVersion, MethodAssetStoredOperationResult,
    MethodAssetStoredOperationResultKind, ReclassifyCatalogEntryCommandSource,
    ReclassifyMethodAssetCatalogEntryInput, RegisterCatalogEntryCommandSource,
    RegisterMethodAssetCatalogEntryInput, RetireCatalogEntryCommandSource,
    RetireDefinitionCommandSource, RetireMethodAssetCatalogEntryInput,
    RetireMethodAssetDefinitionInput, Versioned, VersionedRef,
};
pub use unit_of_work::{CommandUnitOfWork, UnitOfWork};
