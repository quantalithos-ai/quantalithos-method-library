//! Shared command-side shell foundation for method library contracts.

use serde::{Deserialize, Serialize};

use crate::metadata::{ActorContext, CommandMetadata};
use crate::refs::MethodLibraryTypedBoundaryRef;
use crate::views::MethodLibrarySafeMarker;

/// Shared capability groups exposed by the method library public surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodLibraryCapabilityKind {
    /// Method asset definition and catalog capability.
    DefinitionCatalog,
    /// Formalization and version capability.
    FormalizationVersion,
    /// Controlled consumption capability.
    ControlledConsumption,
    /// Trace and consistency capability.
    TraceConsistency,
    /// Relation and distribution capability.
    RelationDistribution,
    /// External reference capability.
    ExternalReference,
    /// Maintenance and convergence capability.
    MaintenanceConvergence,
    /// Peripheral package and method-set capability.
    PeripheralPackageSet,
}

/// Shared command shell foundation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodLibraryCommandShell {
    /// The capability group addressed by the shell.
    pub capability_kind: MethodLibraryCapabilityKind,
    /// The effective actor context.
    pub actor_context: ActorContext,
    /// Shared command metadata.
    pub metadata: CommandMetadata,
    /// The main boundary anchor for the command shell.
    pub boundary_ref: MethodLibraryTypedBoundaryRef,
    /// Additional typed refs carried by the shell.
    pub typed_refs: Vec<MethodLibraryTypedBoundaryRef>,
    /// Safe markers carried by the shell.
    pub safe_markers: Vec<MethodLibrarySafeMarker>,
}
