//! Application-owned transaction boundary for `commit-03-b`.

use core::any::Any;

use method_library_contracts::MethodLibrarySafeMarker;

/// Application-owned post-commit observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetCommitObservation {
    /// The command commit completed durably.
    Committed,
    /// The command commit outcome is unknown and requires formal read-back.
    CommitUnknown {
        /// Safe marker describing the unknown commit observation.
        unknown_marker_ref: MethodLibrarySafeMarker,
    },
}

/// A started command-scoped unit of work.
pub trait CommandUnitOfWork: Send + Any {
    /// Commits all staged writes.
    fn commit(&mut self) -> Result<MethodAssetCommitObservation, ()>;

    /// Rolls back all staged writes.
    fn rollback(&mut self) -> Result<(), ()>;
}

/// Unit-of-work factory for accepted command flows.
pub trait UnitOfWork: Send + Sync {
    /// Starts a new command unit of work.
    fn begin_command_uow(&self) -> Box<dyn CommandUnitOfWork>;
}
