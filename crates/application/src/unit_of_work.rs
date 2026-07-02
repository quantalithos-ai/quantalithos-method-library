//! Application-owned transaction boundary for `commit-03-b`.

use core::any::Any;

/// A started command-scoped unit of work.
pub trait CommandUnitOfWork: Send + Any {
    /// Commits all staged writes.
    fn commit(&mut self) -> Result<(), ()>;

    /// Rolls back all staged writes.
    fn rollback(&mut self) -> Result<(), ()>;
}

/// Unit-of-work factory for accepted command flows.
pub trait UnitOfWork: Send + Sync {
    /// Starts a new command unit of work.
    fn begin_command_uow(&self) -> Box<dyn CommandUnitOfWork>;
}
