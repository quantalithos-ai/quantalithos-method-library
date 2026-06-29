//! Shared metadata and actor foundations for method library contracts.

pub use core_contracts::actor::{ActorContext, ActorKind, ActorRef, RequestOrigin};
pub use core_contracts::metadata::{
    ChangeReason, IdempotencyKey, PageRequest, PageToken, QueryConsistency, QueryMetadata,
    RequestId, RequestMetadata, Timestamp, TraceId,
};

use core_contracts::metadata::CommandMetadata as CoreCommandMetadata;

/// Shared command metadata foundation re-export.
pub type CommandMetadata = CoreCommandMetadata;
