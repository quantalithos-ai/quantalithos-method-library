//! Shared operations job shell foundation for method library contracts.

use serde::{Deserialize, Serialize};

use crate::metadata::RequestMetadata;
use crate::refs::MethodLibraryTypedBoundaryRef;
use crate::views::MethodLibrarySafeMarker;

/// Shared operations job families exposed by the public contracts surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodLibraryOperationsJobKind {
    /// Refresh definition and catalog read materials.
    RefreshCatalogDefinitionReadMaterials,
    /// Refresh formal version read materials.
    RefreshFormalVersionReadMaterials,
    /// Refresh controlled consumption read materials.
    RefreshConsumptionReadMaterials,
    /// Refresh relation and distribution read materials.
    RefreshRelationDistributionMaterials,
    /// Refresh external summary read materials.
    RefreshExternalSummaryReadMaterials,
    /// Refresh trace, audit and impact read materials.
    RefreshTraceAuditImpactMaterials,
    /// Run consistency recovery and convergence.
    RunConsistencyRecoveryConvergence,
    /// Refresh peripheral package and method-set read materials.
    RefreshPeripheralReadMaterials,
}

/// Shared job shell foundation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodLibraryJobShell {
    /// The operations job family.
    pub job_kind: MethodLibraryOperationsJobKind,
    /// Shared request metadata copied into the shell.
    pub request_metadata: RequestMetadata,
    /// The typed refs carried by the shell.
    pub typed_refs: Vec<MethodLibraryTypedBoundaryRef>,
    /// Safe markers carried by the shell.
    pub safe_markers: Vec<MethodLibrarySafeMarker>,
}
