//! Shell-only transaction and helper carriers for the current implementation boundary.

/// Shell transaction boundary for current-boundary application work.
pub trait UnitOfWork: Send + Sync {}

/// Shell time source for current-boundary application work.
pub trait Clock: Send + Sync {}

/// Shell typed-id source for current-boundary application work.
pub trait IdGenerator: Send + Sync {}
