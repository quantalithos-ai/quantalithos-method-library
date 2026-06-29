//! Shared event-side shell foundation for method library contracts.

use serde::{Deserialize, Serialize};

use crate::commands::MethodLibraryCapabilityKind;
use crate::metadata::{RequestMetadata, TraceId};
use crate::refs::MethodLibraryTypedBoundaryRef;
use crate::views::MethodLibrarySafeMarker;

/// Shared event shell foundation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodLibraryEventShell {
    /// The capability group addressed by the shell.
    pub capability_kind: MethodLibraryCapabilityKind,
    /// Shared request metadata copied into the shell.
    pub request_metadata: RequestMetadata,
    /// The correlated trace identifier.
    pub trace_id: TraceId,
    /// The typed refs carried by the shell.
    pub typed_refs: Vec<MethodLibraryTypedBoundaryRef>,
    /// Safe markers carried by the shell.
    pub safe_markers: Vec<MethodLibrarySafeMarker>,
}
