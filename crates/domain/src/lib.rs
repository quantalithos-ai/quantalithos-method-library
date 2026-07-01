//! Domain skeleton for the method library workspace.

pub mod method_asset_definition;
pub mod formal_method_version {}
pub mod consumption_material {}
pub mod trace_audit {}
pub mod relation_distribution {}
pub mod external_summary {}
pub mod maintenance {}
pub mod package_set {}
pub mod errors;
pub mod policies;

pub use errors::{MethodLibraryDomainError, MethodLibraryDomainErrorKind};
pub use method_asset_definition::{MethodAssetCatalogEntry, MethodAssetDefinition};
pub use policies::{
    ConsistencyProtectionJudgement, ConsistencyProtectionPolicy, DefinitionUseBoundaryGuard,
    DefinitionUseBoundaryGuardState, DownstreamConsumptionBoundary,
    DownstreamConsumptionBoundaryState, ExternalBodyBoundaryRule, ExternalBodyBoundaryState,
    RelationIntegrityJudgement, RelationIntegrityRule,
};
