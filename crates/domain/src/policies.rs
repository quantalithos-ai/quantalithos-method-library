//! Shared current-boundary policy shells and judgement carriers.

use method_library_contracts::{
    ConsumptionBoundaryReasonRef, ConsumptionContextRef, DefinitionUseBoundaryGuardRef,
    DefinitionUseBoundaryGuardState, DefinitionUseGuardReasonRef, DownstreamConsumptionBoundaryRef,
    DownstreamConsumptionBoundaryState, DownstreamForbiddenWriteKindSet,
    FormalMethodAssetVersionRef, FormalVersionRequirement, MethodAssetAllowedUseKind,
    MethodAssetAllowedUseKindSet, MethodAssetConsumptionMaterialScopeRef, MethodAssetDefinitionRef,
    MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef,
};

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

/// Shared pure-domain shell for the Definition-vs-Use guard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionUseBoundaryGuard {
    /// Stable guard anchor.
    pub guard_ref: DefinitionUseBoundaryGuardRef,
    /// Protected method-definition anchor.
    pub protected_definition_ref: MethodAssetDefinitionRef,
    /// Protected formal-version anchor.
    pub protected_formal_version_ref: FormalMethodAssetVersionRef,
    /// Controlled consumption-context anchor.
    pub consumption_context_ref: ConsumptionContextRef,
    /// Related downstream-boundary anchor.
    pub boundary_ref: DownstreamConsumptionBoundaryRef,
    /// Safe guard-reason marker wrapper.
    pub guard_reason_ref: DefinitionUseGuardReasonRef,
    /// The current judgement state.
    pub guard_state: DefinitionUseBoundaryGuardState,
}

impl DefinitionUseBoundaryGuard {
    /// Creates a monitoring guard for formal consumption.
    pub fn protect_formal_consumption(
        guard_ref: DefinitionUseBoundaryGuardRef,
        definition_ref: MethodAssetDefinitionRef,
        formal_version_ref: FormalMethodAssetVersionRef,
        boundary_ref: DownstreamConsumptionBoundaryRef,
        consumption_context_ref: ConsumptionContextRef,
        guard_reason_ref: DefinitionUseGuardReasonRef,
    ) -> Self {
        Self {
            guard_ref,
            protected_definition_ref: definition_ref,
            protected_formal_version_ref: formal_version_ref,
            consumption_context_ref,
            boundary_ref,
            guard_reason_ref,
            guard_state: DefinitionUseBoundaryGuardState::Monitoring,
        }
    }

    /// Confirms the material is anchored to the protected formal version.
    pub fn assert_material_uses_formal_version(
        &self,
        formal_version_ref: &FormalMethodAssetVersionRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if &self.protected_formal_version_ref == formal_version_ref {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invariant_violation())
        }
    }

    /// Confirms the consumption context remains within the guard boundary.
    pub fn assert_context_within_boundary(
        &self,
        consumption_context_ref: &ConsumptionContextRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if &self.consumption_context_ref == consumption_context_ref {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invariant_violation())
        }
    }

    /// Records a safe violation marker.
    pub fn mark_violation(
        &mut self,
        violation_ref: MethodLibrarySafeMarker,
    ) -> Result<(), MethodLibraryDomainError> {
        if !violation_ref.is_public_safe() {
            return Err(MethodLibraryDomainError::policy_rejected());
        }
        self.guard_state = DefinitionUseBoundaryGuardState::ViolationRecorded;
        self.guard_reason_ref = DefinitionUseGuardReasonRef::new(violation_ref);
        Ok(())
    }

    /// Rejects a downstream writeback attempt with a safe reason source.
    pub fn reject_downstream_definition_write(
        &mut self,
        write_attempt_ref: MethodLibraryTypedBoundaryRef,
        reason_ref: DefinitionUseGuardReasonRef,
    ) -> Result<(), MethodLibraryDomainError> {
        let kind = write_attempt_ref.kind();
        let next_state = match kind {
            method_library_contracts::MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition
            | method_library_contracts::MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersion => {
                DefinitionUseBoundaryGuardState::RejectedCandidate
            }
            _ => DefinitionUseBoundaryGuardState::ManualReviewRequired,
        };

        if !reason_ref.as_safe_marker().is_public_safe() {
            return Err(MethodLibraryDomainError::policy_rejected());
        }

        self.guard_state = next_state;
        self.guard_reason_ref = reason_ref;
        Ok(())
    }
}

/// Shared pure-domain shell for a downstream consumption boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownstreamConsumptionBoundary {
    /// Stable boundary anchor.
    pub boundary_ref: DownstreamConsumptionBoundaryRef,
    /// Controlled consumption-context anchor.
    pub consumption_context_ref: ConsumptionContextRef,
    /// Controlled formal-version state requirement.
    pub formal_version_requirement: FormalVersionRequirement,
    /// Allowed downstream use families.
    pub allowed_use_kind_set: MethodAssetAllowedUseKindSet,
    /// Forbidden downstream truth writeback families.
    pub forbidden_write_kind_set: DownstreamForbiddenWriteKindSet,
    /// Allowed material scope.
    pub material_scope_ref: MethodAssetConsumptionMaterialScopeRef,
    /// Safe boundary-reason marker wrapper.
    pub boundary_reason_ref: ConsumptionBoundaryReasonRef,
    /// The current boundary judgement state.
    pub boundary_state: DownstreamConsumptionBoundaryState,
}

impl DownstreamConsumptionBoundary {
    /// Creates a registered downstream boundary for a controlled consumption context.
    pub fn for_consumption_context(
        boundary_ref: DownstreamConsumptionBoundaryRef,
        consumption_context_ref: ConsumptionContextRef,
        formal_version_requirement: FormalVersionRequirement,
        allowed_use_kind_set: MethodAssetAllowedUseKindSet,
        forbidden_write_kind_set: DownstreamForbiddenWriteKindSet,
        material_scope_ref: MethodAssetConsumptionMaterialScopeRef,
        boundary_reason_ref: ConsumptionBoundaryReasonRef,
    ) -> Result<Self, MethodLibraryDomainError> {
        if allowed_use_kind_set.is_empty() || forbidden_write_kind_set.is_empty() {
            return Err(MethodLibraryDomainError::invariant_violation());
        }

        Ok(Self {
            boundary_ref,
            consumption_context_ref,
            formal_version_requirement,
            allowed_use_kind_set,
            forbidden_write_kind_set,
            material_scope_ref,
            boundary_reason_ref,
            boundary_state: DownstreamConsumptionBoundaryState::Registered,
        })
    }

    /// Confirms the consumption context is the one registered by the boundary.
    pub fn assert_context_allowed(
        &self,
        consumption_context_ref: &ConsumptionContextRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if &self.consumption_context_ref == consumption_context_ref {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::policy_rejected())
        }
    }

    /// Confirms the use kind is allowed by the current boundary.
    pub fn assert_use_kind_allowed(
        &self,
        use_kind: MethodAssetAllowedUseKind,
    ) -> Result<(), MethodLibraryDomainError> {
        if self.allowed_use_kind_set.allowed_kinds.contains(&use_kind) {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::policy_rejected())
        }
    }

    /// Rejects a downstream writeback family that the boundary forbids.
    pub fn reject_forbidden_write(
        &self,
        forbidden_write_kind_set: &DownstreamForbiddenWriteKindSet,
    ) -> Result<(), MethodLibraryDomainError> {
        if forbidden_write_kind_set
            .forbidden_kinds
            .iter()
            .any(|kind| self.forbidden_write_kind_set.forbidden_kinds.contains(kind))
        {
            Err(MethodLibraryDomainError::policy_rejected())
        } else {
            Ok(())
        }
    }

    /// Moves the boundary into a constrained state with a safe reason.
    pub fn scope_limited(
        &mut self,
        reason_ref: ConsumptionBoundaryReasonRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if !reason_ref.as_safe_marker().is_public_safe() {
            return Err(MethodLibraryDomainError::policy_rejected());
        }
        self.boundary_reason_ref = reason_ref;
        self.boundary_state = DownstreamConsumptionBoundaryState::Constrained;
        Ok(())
    }

    /// Moves the boundary into an unavailable state with a safe reason.
    pub fn unavailable(
        &mut self,
        reason_ref: ConsumptionBoundaryReasonRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if !reason_ref.as_safe_marker().is_public_safe() {
            return Err(MethodLibraryDomainError::policy_rejected());
        }
        self.boundary_reason_ref = reason_ref;
        self.boundary_state = DownstreamConsumptionBoundaryState::Unavailable;
        Ok(())
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
