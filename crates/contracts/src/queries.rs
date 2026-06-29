//! Shared query-side shell foundation for method library contracts.

use serde::{Deserialize, Serialize};

use crate::commands::MethodLibraryCapabilityKind;
use crate::metadata::{ActorContext, QueryMetadata};
use crate::refs::MethodLibraryTypedBoundaryRef;
use crate::views::MethodLibrarySafeMarker;

/// Shared query shell foundation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodLibraryQueryShell {
    /// The capability group addressed by the shell.
    pub capability_kind: MethodLibraryCapabilityKind,
    /// The effective actor context.
    pub actor_context: ActorContext,
    /// Shared query metadata.
    pub metadata: QueryMetadata,
    /// The main boundary anchor for the query shell.
    pub boundary_ref: MethodLibraryTypedBoundaryRef,
    /// Additional typed refs carried by the shell.
    pub typed_refs: Vec<MethodLibraryTypedBoundaryRef>,
    /// Safe markers carried by the shell.
    pub safe_markers: Vec<MethodLibrarySafeMarker>,
}
