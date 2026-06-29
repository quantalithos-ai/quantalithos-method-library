//! Pure domain error foundation for the current method library boundary.

use core::fmt;

/// Exact pure-domain error kinds allowed in `commit-02-b`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodLibraryDomainErrorKind {
    /// A required typed boundary or marker carrier was missing.
    MissingRequiredTypedInput,
    /// A current-boundary shell violated its closed invariant.
    InvariantViolation,
    /// A requested judgement transition is not allowed.
    InvalidTransition,
    /// A current-boundary policy shell rejected the candidate input.
    PolicyRejected,
    /// A raw body candidate would cross the body-free boundary.
    BodyFreeBoundaryViolation,
}

/// Pure-domain error wrapper for `commit-02-b`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MethodLibraryDomainError {
    kind: MethodLibraryDomainErrorKind,
}

impl MethodLibraryDomainError {
    /// Creates a new domain error from the exact current-boundary kind.
    pub const fn new(kind: MethodLibraryDomainErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the exact current-boundary error kind.
    pub const fn kind(self) -> MethodLibraryDomainErrorKind {
        self.kind
    }

    /// Builds a missing typed-input error.
    pub const fn missing_required_typed_input() -> Self {
        Self::new(MethodLibraryDomainErrorKind::MissingRequiredTypedInput)
    }

    /// Builds an invariant-violation error.
    pub const fn invariant_violation() -> Self {
        Self::new(MethodLibraryDomainErrorKind::InvariantViolation)
    }

    /// Builds an invalid-transition error.
    pub const fn invalid_transition() -> Self {
        Self::new(MethodLibraryDomainErrorKind::InvalidTransition)
    }

    /// Builds a policy-rejected error.
    pub const fn policy_rejected() -> Self {
        Self::new(MethodLibraryDomainErrorKind::PolicyRejected)
    }

    /// Builds a body-free boundary violation error.
    pub const fn body_free_boundary_violation() -> Self {
        Self::new(MethodLibraryDomainErrorKind::BodyFreeBoundaryViolation)
    }
}

impl From<MethodLibraryDomainErrorKind> for MethodLibraryDomainError {
    fn from(kind: MethodLibraryDomainErrorKind) -> Self {
        Self::new(kind)
    }
}

impl fmt::Display for MethodLibraryDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.kind {
            MethodLibraryDomainErrorKind::MissingRequiredTypedInput => {
                "missing required typed input"
            }
            MethodLibraryDomainErrorKind::InvariantViolation => {
                "current-boundary invariant violated"
            }
            MethodLibraryDomainErrorKind::InvalidTransition => {
                "invalid current-boundary transition"
            }
            MethodLibraryDomainErrorKind::PolicyRejected => {
                "current-boundary policy rejected the candidate"
            }
            MethodLibraryDomainErrorKind::BodyFreeBoundaryViolation => {
                "raw body would cross the body-free boundary"
            }
        };

        formatter.write_str(message)
    }
}

impl std::error::Error for MethodLibraryDomainError {}
