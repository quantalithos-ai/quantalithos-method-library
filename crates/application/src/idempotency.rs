//! Shell-only idempotency carriers for the current implementation boundary.

/// Shell operation context carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MethodAssetOperationContext;

/// Shell idempotency guard carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MethodAssetIdempotencyGuard;

/// Shell stored-operation-result carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MethodAssetStoredOperationResult;

/// Exact current-boundary idempotency decision labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodAssetIdempotencyDecisionKind {
    Fresh,
    DuplicateReplay,
    Conflict,
    Rejected,
    ReplayUnavailable,
}

/// Exact current-boundary stored-result labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodAssetStoredOperationResultKind {
    Accepted,
    Rejected,
    Ignored,
    Conflict,
}
