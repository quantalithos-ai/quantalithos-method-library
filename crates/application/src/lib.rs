//! Definition/catalog accepted-service surface for `commit-03-b`.

pub mod definition_catalog;
pub mod formalization_version;
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
pub use formalization_version::{
    DefaultMethodAssetFormalizationVersionCommandFacade,
    EstablishFormalMethodAssetVersionCommandSource, EstablishFormalMethodAssetVersionInput,
    EvaluateFormalizationEligibilityCommandSource,
    EvaluateMethodAssetFormalizationEligibilityInput,
    InitiateMethodAssetFormalizationCommandSource, InitiateMethodAssetFormalizationInput,
    MethodAssetFormalizationVersionCommandDispatchInput,
    MethodAssetFormalizationVersionCommandDispatchOutput,
    MethodAssetFormalizationVersionCommandFacade, MethodAssetFormalizationVersionCommandSelector,
    MethodAssetFormalizationVersionCommandSource, MethodAssetFormalizationVersionReplayEnvelope,
    MethodAssetFormalizationVersionReplayEnvelopeFactoryInput,
    MethodAssetFormalizationVersionServiceInput, MethodAssetFormalizationVersionSupportRefFactory,
    RecordFormalVersionSemanticChangeCommandSource, RecordFormalVersionSemanticChangeInput,
    RetireFormalMethodAssetVersionCommandSource, RetireFormalMethodAssetVersionInput,
    SupersedeFormalMethodAssetVersionCommandSource, SupersedeFormalMethodAssetVersionInput,
};
pub use ports::{
    FormalVersionChangeDiagnostic, FormalizationBasisResolution, FormalizationBasisResolutionInput,
    FormalizationEligibilityDiagnostic,
};
pub use unit_of_work::{CommandUnitOfWork, MethodAssetCommitObservation, UnitOfWork};
