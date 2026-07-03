//! Domain skeleton for the method library workspace.

pub mod consumption_material;
pub mod formal_method_version;
pub mod method_asset_definition;
pub mod trace_audit {}
pub mod relation_distribution {}
pub mod external_summary {}
pub mod maintenance {}
pub mod package_set {}
pub mod errors;
pub mod policies;

pub use consumption_material::MethodAssetConsumptionMaterial;
pub use errors::{MethodLibraryDomainError, MethodLibraryDomainErrorKind};
pub use formal_method_version::{
    FormalMethodAssetVersion, FormalizationBasisSummary, FormalizationState,
};
pub use method_asset_definition::{
    MethodAssetCatalogEntry, MethodAssetDefinition, MethodAssetDefinitionLifecycle,
};
pub use method_library_contracts::{
    DefinitionUseBoundaryGuardState, DownstreamConsumptionBoundaryState,
    MethodAssetConsumptionMaterialState,
};
pub use policies::{
    ConsistencyProtectionJudgement, ConsistencyProtectionPolicy, DefinitionUseBoundaryGuard,
    DownstreamConsumptionBoundary, ExternalBodyBoundaryRule, ExternalBodyBoundaryState,
    RelationIntegrityJudgement, RelationIntegrityRule,
};
