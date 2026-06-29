//! Public contract skeleton for the method library workspace.

pub mod commands;
pub mod errors;
pub mod events;
pub mod fixtures;
pub mod jobs;
pub mod metadata;
pub mod queries;
pub mod refs;
pub mod views;

pub use commands::{MethodLibraryCapabilityKind, MethodLibraryCommandShell};
pub use events::MethodLibraryEventShell;
pub use jobs::{MethodLibraryJobShell, MethodLibraryOperationsJobKind};
pub use queries::MethodLibraryQueryShell;
pub use refs::{MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind};
pub use views::{
    MethodLibraryPublicShell, MethodLibrarySafeMarker, MethodLibrarySafeMarkerKind,
    MethodLibraryShellKind, MethodLibraryViewShell,
};
