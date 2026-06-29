//! Shared current-boundary policy shells and judgement carriers.

use method_library_contracts::{MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef};

use crate::errors::MethodLibraryDomainError;

fn required_ref(
    value: Option<MethodLibraryTypedBoundaryRef>,
) -> Result<MethodLibraryTypedBoundaryRef, MethodLibraryDomainError> {
    value.ok_or_else(MethodLibraryDomainError::missing_required_typed_input)
}

fn required_marker(
    value: Option<MethodLibrarySafeMarker>,
) -> Result<MethodLibrarySafeMarker, MethodLibraryDomainError> {
    value.ok_or_else(MethodLibraryDomainError::missing_required_typed_input)
}

/// Judgement state for `DefinitionUseBoundaryGuard`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefinitionUseBoundaryGuardState {
    /// The guard is monitoring use-boundary violations.
    Monitoring,
    /// A safe violation marker was recorded.
    ViolationRecorded,
    /// The candidate was rejected before a safe violation could be recorded.
    RejectedCandidate,
}

impl DefinitionUseBoundaryGuardState {
    /// Applies the current-boundary violation-recording transition.
    pub fn record_violation(
        self,
        has_safe_reason: bool,
        raw_body_candidate: bool,
    ) -> Result<Self, MethodLibraryDomainError> {
        match self {
            Self::Monitoring if has_safe_reason && !raw_body_candidate => {
                Ok(Self::ViolationRecorded)
            }
            Self::Monitoring => Ok(Self::RejectedCandidate),
            _ => Err(MethodLibraryDomainError::invalid_transition()),
        }
    }

    /// Rejects the rejected-branch state with the exact policy error kind.
    pub fn assert_not_rejected(self) -> Result<Self, MethodLibraryDomainError> {
        if matches!(self, Self::RejectedCandidate) {
            return Err(MethodLibraryDomainError::policy_rejected());
        }

        Ok(self)
    }
}

/// Shared pure-domain shell for the Definition-vs-Use guard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionUseBoundaryGuard {
    /// The opaque stable guard anchor.
    pub guard_ref: MethodLibraryTypedBoundaryRef,
    /// The protected method-definition anchor.
    pub protected_definition_ref: MethodLibraryTypedBoundaryRef,
    /// The protected formal-version anchor.
    pub protected_formal_version_ref: MethodLibraryTypedBoundaryRef,
    /// The controlled consumption-context anchor.
    pub consumption_context_ref: MethodLibraryTypedBoundaryRef,
    /// The related downstream-boundary anchor.
    pub boundary_ref: MethodLibraryTypedBoundaryRef,
    /// The safe reason marker carried by the shell.
    pub guard_reason_marker: MethodLibrarySafeMarker,
    /// The current judgement state.
    pub state: DefinitionUseBoundaryGuardState,
}

impl DefinitionUseBoundaryGuard {
    /// Builds the current-boundary guard shell.
    pub fn try_new(
        guard_ref: Option<MethodLibraryTypedBoundaryRef>,
        protected_definition_ref: Option<MethodLibraryTypedBoundaryRef>,
        protected_formal_version_ref: Option<MethodLibraryTypedBoundaryRef>,
        consumption_context_ref: Option<MethodLibraryTypedBoundaryRef>,
        boundary_ref: Option<MethodLibraryTypedBoundaryRef>,
        guard_reason_marker: Option<MethodLibrarySafeMarker>,
        state: DefinitionUseBoundaryGuardState,
    ) -> Result<Self, MethodLibraryDomainError> {
        Ok(Self {
            guard_ref: required_ref(guard_ref)?,
            protected_definition_ref: required_ref(protected_definition_ref)?,
            protected_formal_version_ref: required_ref(protected_formal_version_ref)?,
            consumption_context_ref: required_ref(consumption_context_ref)?,
            boundary_ref: required_ref(boundary_ref)?,
            guard_reason_marker: required_marker(guard_reason_marker)?,
            state,
        })
    }

    /// Evaluates a violation candidate without introducing later-boundary state.
    pub fn evaluate_violation_candidate(
        &self,
        safe_reason_marker: Option<MethodLibrarySafeMarker>,
        raw_body_candidate: bool,
    ) -> Result<DefinitionUseBoundaryGuardState, MethodLibraryDomainError> {
        self.state
            .record_violation(safe_reason_marker.is_some(), raw_body_candidate)?
            .assert_not_rejected()
    }
}

/// Judgement state for `DownstreamConsumptionBoundary`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DownstreamConsumptionBoundaryState {
    /// The boundary is registered and readable.
    Registered,
    /// The boundary is readable but constrained.
    Constrained,
    /// The boundary is currently unavailable.
    Unavailable,
}

impl DownstreamConsumptionBoundaryState {
    /// Creates the first persisted boundary state from the registration branch.
    pub const fn register(constrained: bool) -> Self {
        if constrained {
            Self::Constrained
        } else {
            Self::Registered
        }
    }

    /// Applies the current-boundary adjustment branch.
    pub const fn adjust(self, next: Self) -> Self {
        let _ = self;
        next
    }
}

/// Shared pure-domain shell for a downstream consumption boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownstreamConsumptionBoundary {
    /// The opaque boundary anchor.
    pub boundary_ref: MethodLibraryTypedBoundaryRef,
    /// The controlled consumption-context anchor.
    pub consumption_context_ref: MethodLibraryTypedBoundaryRef,
    /// The safe reason marker carried by the boundary shell.
    pub boundary_reason_marker: MethodLibrarySafeMarker,
    /// The current boundary judgement state.
    pub state: DownstreamConsumptionBoundaryState,
}

impl DownstreamConsumptionBoundary {
    /// Builds the current-boundary consumption shell.
    pub fn try_new(
        boundary_ref: Option<MethodLibraryTypedBoundaryRef>,
        consumption_context_ref: Option<MethodLibraryTypedBoundaryRef>,
        boundary_reason_marker: Option<MethodLibrarySafeMarker>,
        state: DownstreamConsumptionBoundaryState,
    ) -> Result<Self, MethodLibraryDomainError> {
        Ok(Self {
            boundary_ref: required_ref(boundary_ref)?,
            consumption_context_ref: required_ref(consumption_context_ref)?,
            boundary_reason_marker: required_marker(boundary_reason_marker)?,
            state,
        })
    }

    /// Applies a legal current-boundary state adjustment.
    pub fn adjust(mut self, next: DownstreamConsumptionBoundaryState) -> Self {
        self.state = self.state.adjust(next);
        self
    }
}

/// Current-boundary judgement for consistency protection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsistencyProtectionJudgement {
    /// Protection is established.
    ProtectionEstablished,
    /// Impact is unknown and must remain pending.
    UnknownImpactPending,
    /// Protection requires a constrained follow-up path.
    ProtectionConstrained,
    /// Input was insufficient for a safe judgement.
    InputRejected,
}

impl ConsistencyProtectionJudgement {
    /// Reconciles a previously pending or constrained judgement.
    pub fn reconcile(self, next: Self) -> Result<Self, MethodLibraryDomainError> {
        if self == next {
            return Ok(next);
        }

        match (self, next) {
            (
                Self::UnknownImpactPending | Self::ProtectionConstrained,
                Self::ProtectionEstablished
                | Self::UnknownImpactPending
                | Self::ProtectionConstrained,
            ) => Ok(next),
            _ => Err(MethodLibraryDomainError::invalid_transition()),
        }
    }
}

/// Shared pure-domain shell for the consistency-protection policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsistencyProtectionPolicy {
    /// The opaque policy anchor.
    pub policy_ref: MethodLibraryTypedBoundaryRef,
    /// The protected formal-version anchor.
    pub protected_version_ref: MethodLibraryTypedBoundaryRef,
    /// Optional impact-summary anchor.
    pub impact_summary_ref: Option<MethodLibraryTypedBoundaryRef>,
    /// Optional trace-material anchor.
    pub trace_material_ref: Option<MethodLibraryTypedBoundaryRef>,
    /// The safe decision marker carried by the shell.
    pub decision_marker: MethodLibrarySafeMarker,
    /// The current protection judgement.
    pub state: ConsistencyProtectionJudgement,
}

impl ConsistencyProtectionPolicy {
    /// Builds the current-boundary consistency shell.
    pub fn try_new(
        policy_ref: Option<MethodLibraryTypedBoundaryRef>,
        protected_version_ref: Option<MethodLibraryTypedBoundaryRef>,
        impact_summary_ref: Option<MethodLibraryTypedBoundaryRef>,
        trace_material_ref: Option<MethodLibraryTypedBoundaryRef>,
        decision_marker: Option<MethodLibrarySafeMarker>,
        state: ConsistencyProtectionJudgement,
    ) -> Result<Self, MethodLibraryDomainError> {
        Ok(Self {
            policy_ref: required_ref(policy_ref)?,
            protected_version_ref: required_ref(protected_version_ref)?,
            impact_summary_ref,
            trace_material_ref,
            decision_marker: required_marker(decision_marker)?,
            state,
        })
    }

    /// Applies a legal current-boundary protection reconciliation.
    pub fn reconcile(
        mut self,
        next: ConsistencyProtectionJudgement,
    ) -> Result<Self, MethodLibraryDomainError> {
        self.state = self.state.reconcile(next)?;
        Ok(self)
    }
}

/// Current-boundary judgement for relation integrity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationIntegrityJudgement {
    /// The relation satisfies its integrity constraints.
    IntegritySatisfied,
    /// The relation is waiting for additional formal inputs.
    IntegrityPending,
    /// A violation marker was copied from a formal diagnostic.
    ViolationMarked,
    /// The candidate was rejected before a safe violation could be formed.
    IntegrityRejected,
}

impl RelationIntegrityJudgement {
    /// Marks an integrity violation from a legal source state.
    pub fn mark_violation(self) -> Result<Self, MethodLibraryDomainError> {
        match self {
            Self::IntegritySatisfied | Self::IntegrityPending => Ok(Self::ViolationMarked),
            _ => Err(MethodLibraryDomainError::invalid_transition()),
        }
    }
}

/// Shared pure-domain shell for the relation-integrity rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationIntegrityRule {
    /// The opaque rule anchor.
    pub rule_ref: MethodLibraryTypedBoundaryRef,
    /// The protected relation anchor.
    pub relation_ref: MethodLibraryTypedBoundaryRef,
    /// The source-definition anchor.
    pub source_definition_ref: MethodLibraryTypedBoundaryRef,
    /// The target-definition anchor.
    pub target_definition_ref: MethodLibraryTypedBoundaryRef,
    /// Optional distribution-boundary anchor.
    pub distribution_boundary_ref: Option<MethodLibraryTypedBoundaryRef>,
    /// The safe decision marker carried by the rule shell.
    pub decision_marker: MethodLibrarySafeMarker,
    /// The current integrity judgement.
    pub state: RelationIntegrityJudgement,
}

impl RelationIntegrityRule {
    /// Builds the current-boundary relation-integrity shell.
    pub fn try_new(
        rule_ref: Option<MethodLibraryTypedBoundaryRef>,
        relation_ref: Option<MethodLibraryTypedBoundaryRef>,
        source_definition_ref: Option<MethodLibraryTypedBoundaryRef>,
        target_definition_ref: Option<MethodLibraryTypedBoundaryRef>,
        distribution_boundary_ref: Option<MethodLibraryTypedBoundaryRef>,
        decision_marker: Option<MethodLibrarySafeMarker>,
        state: RelationIntegrityJudgement,
    ) -> Result<Self, MethodLibraryDomainError> {
        Ok(Self {
            rule_ref: required_ref(rule_ref)?,
            relation_ref: required_ref(relation_ref)?,
            source_definition_ref: required_ref(source_definition_ref)?,
            target_definition_ref: required_ref(target_definition_ref)?,
            distribution_boundary_ref,
            decision_marker: required_marker(decision_marker)?,
            state,
        })
    }

    /// Marks a safe integrity violation from a legal source state.
    pub fn mark_violation(mut self) -> Result<Self, MethodLibraryDomainError> {
        self.state = self.state.mark_violation()?;
        Ok(self)
    }
}

/// Current-boundary state for the external body-boundary rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalBodyBoundaryState {
    /// The candidate was asserted to be body-free.
    AssertedBodyFree,
    /// The candidate was rejected because it would carry body content.
    BodyCandidateRejected,
    /// The candidate inputs were incomplete for a safe judgement.
    InvalidCandidate,
}

impl ExternalBodyBoundaryState {
    /// Applies the initial body-free assertion branch.
    pub const fn assert_candidate(has_candidate_ref: bool, body_free_assertion: bool) -> Self {
        if has_candidate_ref && body_free_assertion {
            Self::AssertedBodyFree
        } else {
            Self::InvalidCandidate
        }
    }

    /// Applies the rejection branch after a legal body-free assertion.
    pub fn reject_candidate(
        self,
        has_candidate_ref: bool,
        has_safe_reason: bool,
    ) -> Result<Self, MethodLibraryDomainError> {
        match self {
            Self::AssertedBodyFree if has_candidate_ref && has_safe_reason => {
                Ok(Self::BodyCandidateRejected)
            }
            Self::AssertedBodyFree => Ok(Self::InvalidCandidate),
            _ => Err(MethodLibraryDomainError::invalid_transition()),
        }
    }
}

/// Shared pure-domain shell for the external body-boundary rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalBodyBoundaryRule {
    /// The opaque rule anchor.
    pub rule_ref: MethodLibraryTypedBoundaryRef,
    /// Optional external-source anchor.
    pub external_source_ref: Option<MethodLibraryTypedBoundaryRef>,
    /// Optional artifact/archive anchor.
    pub artifact_archive_ref: Option<MethodLibraryTypedBoundaryRef>,
    /// Optional lineage anchor.
    pub lineage_marker_ref: Option<MethodLibraryTypedBoundaryRef>,
    /// The safe boundary marker carried by the rule shell.
    pub boundary_reason_marker: MethodLibrarySafeMarker,
    /// The current body-boundary judgement.
    pub state: ExternalBodyBoundaryState,
}

impl ExternalBodyBoundaryRule {
    /// Builds the current-boundary external body-boundary shell.
    pub fn try_new(
        rule_ref: Option<MethodLibraryTypedBoundaryRef>,
        external_source_ref: Option<MethodLibraryTypedBoundaryRef>,
        artifact_archive_ref: Option<MethodLibraryTypedBoundaryRef>,
        lineage_marker_ref: Option<MethodLibraryTypedBoundaryRef>,
        boundary_reason_marker: Option<MethodLibrarySafeMarker>,
        state: ExternalBodyBoundaryState,
    ) -> Result<Self, MethodLibraryDomainError> {
        Ok(Self {
            rule_ref: required_ref(rule_ref)?,
            external_source_ref,
            artifact_archive_ref,
            lineage_marker_ref,
            boundary_reason_marker: required_marker(boundary_reason_marker)?,
            state,
        })
    }

    fn has_candidate_ref(&self) -> bool {
        self.external_source_ref.is_some() || self.artifact_archive_ref.is_some()
    }

    /// Validates the current-boundary shell invariant for body-free state.
    pub fn assert_current_boundary_invariant(&self) -> Result<(), MethodLibraryDomainError> {
        if matches!(
            self.state,
            ExternalBodyBoundaryState::AssertedBodyFree
                | ExternalBodyBoundaryState::BodyCandidateRejected
        ) && !self.has_candidate_ref()
        {
            return Err(MethodLibraryDomainError::invariant_violation());
        }

        Ok(())
    }

    /// Asserts that the current candidate remains body-free.
    pub fn assert_summary_body_free(
        &self,
        raw_body_candidate: bool,
    ) -> Result<ExternalBodyBoundaryState, MethodLibraryDomainError> {
        if !self.has_candidate_ref() {
            return Err(MethodLibraryDomainError::missing_required_typed_input());
        }

        if raw_body_candidate {
            return Err(MethodLibraryDomainError::body_free_boundary_violation());
        }

        Ok(ExternalBodyBoundaryState::AssertedBodyFree)
    }

    /// Applies the legal rejection branch after a body-free assertion.
    pub fn reject_candidate(
        mut self,
        has_safe_reason: bool,
    ) -> Result<Self, MethodLibraryDomainError> {
        self.state = self
            .state
            .reject_candidate(self.has_candidate_ref(), has_safe_reason)?;
        Ok(self)
    }
}
