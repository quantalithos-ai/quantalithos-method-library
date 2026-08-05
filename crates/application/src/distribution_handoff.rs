//! Distribution/handoff accepted-service slice for `commit-05-b`.

use std::sync::{Arc, Mutex};

use method_library_contracts::{
    ConsumptionContextRef, DistributionContextRef, DownstreamConsumptionBoundaryRef,
    MethodAssetAcceptedOperationSummaryRef, MethodAssetAdapterAvailabilityStateRef,
    MethodAssetAdapterSlotRef, MethodAssetAdapterSlotRefSet, MethodAssetApiEntryContextRef,
    MethodAssetApplicationDispatchRef, MethodAssetConsumptionAvailabilityMarker,
    MethodAssetConsumptionAvailabilityTarget, MethodAssetDedupScopeRef,
    MethodAssetDegradedDecisionRef, MethodAssetDistributionAdjustmentReasonRef,
    MethodAssetDistributionRef, MethodAssetEffectSummaryRef, MethodAssetEventCandidateAssemblyRef,
    MethodAssetEventCandidateReasonRef, MethodAssetHandoffBindingStateRef,
    MethodAssetHandoffBoundaryMarkerRef, MethodAssetHandoffMarkerRef, MethodAssetHandoffTargetRef,
    MethodAssetHandoffTargetRefSet, MethodAssetIdempotencyKeyRef,
    MethodAssetInfraSafeDiagnosticRef, MethodAssetOperationContextRef,
    MethodAssetOperationDigestRef, MethodAssetPublicationBoundaryMarkerRef,
    MethodAssetPublicationOutcomeRef, MethodAssetPublisherBindingStateRef, MethodAssetRelationRef,
    MethodAssetReplayMarkerRef, MethodAssetSafeIgnoreReasonRef, MethodAssetSafeRejectReasonRef,
    MethodAssetStoredOperationResultRef, MethodAssetTargetRegistryScopeRef,
    MethodLibraryCapabilityKind, MethodLibraryCommandShell, MethodLibrarySafeMarker,
    MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};

use crate::definition_catalog::{
    MethodAssetEffectSummaryRefSet, MethodAssetExpectedVersion,
    MethodAssetReplayEnvelopeBuildError, MethodAssetRepositoryError,
    MethodAssetStoredOperationResult, MethodAssetStoredOperationResultKind, Versioned,
    VersionedRef,
};
use crate::ports::MethodAssetStoredOperationResultRepository;
use crate::unit_of_work::{CommandUnitOfWork, MethodAssetCommitObservation, UnitOfWork};

/// Body-free application-owned post-commit seam source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDistributionHandoffSeamSource {
    pub target_registry_scope_ref: MethodAssetTargetRegistryScopeRef,
    pub required_slot_refs: MethodAssetAdapterSlotRefSet,
    pub publisher_binding_ref: MethodAssetPublisherBindingStateRef,
    pub handoff_binding_ref: Option<MethodAssetHandoffBindingStateRef>,
    pub publication_boundary_marker_ref: MethodAssetPublicationBoundaryMarkerRef,
    pub handoff_boundary_marker_ref: Option<MethodAssetHandoffBoundaryMarkerRef>,
}

impl MethodAssetDistributionHandoffSeamSource {
    fn is_valid(&self) -> bool {
        !self.required_slot_refs.is_empty()
            && self.handoff_binding_ref.is_some() == self.handoff_boundary_marker_ref.is_some()
            && self.target_registry_scope_ref.as_typed_ref().kind()
                == MethodAssetTargetRegistryScopeRef::expected_kind()
            && self.required_slot_refs.refs.iter().all(|value| {
                value.as_typed_ref().kind() == MethodAssetAdapterSlotRef::expected_kind()
            })
            && self.publisher_binding_ref.as_typed_ref().kind()
                == MethodAssetPublisherBindingStateRef::expected_kind()
            && self.handoff_binding_ref.as_ref().is_none_or(|value| {
                value.as_typed_ref().kind() == MethodAssetHandoffBindingStateRef::expected_kind()
            })
    }
}

/// Exact structured command source for current-boundary distribution commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetDistributionHandoffCommandSource {
    PrepareDistributionRef {
        relation_ref: MethodAssetRelationRef,
        requested_distribution_ref: Option<MethodAssetDistributionRef>,
        distribution_context_ref: DistributionContextRef,
        consumption_context_ref: ConsumptionContextRef,
        boundary_ref: DownstreamConsumptionBoundaryRef,
        availability_marker: MethodAssetConsumptionAvailabilityMarker,
        candidate_reason_ref: MethodAssetEventCandidateReasonRef,
    },
    AdjustDistributionContext {
        relation_ref: MethodAssetRelationRef,
        distribution_ref: MethodAssetDistributionRef,
        previous_context_ref: DistributionContextRef,
        new_context_ref: DistributionContextRef,
        adjustment_reason_ref: MethodAssetDistributionAdjustmentReasonRef,
        candidate_reason_ref: MethodAssetEventCandidateReasonRef,
        expected_distribution_version: MethodAssetExpectedVersion,
    },
    MarkDistributionAvailability {
        relation_ref: MethodAssetRelationRef,
        distribution_ref: MethodAssetDistributionRef,
        distribution_context_ref: DistributionContextRef,
        availability_marker: MethodAssetConsumptionAvailabilityMarker,
        candidate_reason_ref: MethodAssetEventCandidateReasonRef,
    },
}

fn source_is_valid(source: &MethodAssetDistributionHandoffCommandSource) -> bool {
    match source {
        MethodAssetDistributionHandoffCommandSource::PrepareDistributionRef {
            relation_ref,
            requested_distribution_ref,
            distribution_context_ref,
            consumption_context_ref,
            boundary_ref,
            ..
        } => {
            relation_ref.as_typed_ref().kind() == MethodAssetRelationRef::expected_kind()
                && requested_distribution_ref.as_ref().is_none_or(|value| {
                    value.as_typed_ref().kind() == MethodAssetDistributionRef::expected_kind()
                })
                && distribution_context_ref.as_typed_ref().kind()
                    == DistributionContextRef::expected_kind()
                && consumption_context_ref.as_typed_ref().kind()
                    == ConsumptionContextRef::expected_kind()
                && boundary_ref.as_typed_ref().kind()
                    == DownstreamConsumptionBoundaryRef::expected_kind()
        }
        MethodAssetDistributionHandoffCommandSource::AdjustDistributionContext {
            relation_ref,
            distribution_ref,
            previous_context_ref,
            new_context_ref,
            ..
        } => {
            relation_ref.as_typed_ref().kind() == MethodAssetRelationRef::expected_kind()
                && distribution_ref.as_typed_ref().kind()
                    == MethodAssetDistributionRef::expected_kind()
                && previous_context_ref.as_typed_ref().kind()
                    == DistributionContextRef::expected_kind()
                && new_context_ref.as_typed_ref().kind() == DistributionContextRef::expected_kind()
        }
        MethodAssetDistributionHandoffCommandSource::MarkDistributionAvailability {
            relation_ref,
            distribution_ref,
            distribution_context_ref,
            ..
        } => {
            relation_ref.as_typed_ref().kind() == MethodAssetRelationRef::expected_kind()
                && distribution_ref.as_typed_ref().kind()
                    == MethodAssetDistributionRef::expected_kind()
                && distribution_context_ref.as_typed_ref().kind()
                    == DistributionContextRef::expected_kind()
        }
    }
}

/// Application facade input for distribution/handoff commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDistributionHandoffCommandDispatchInput {
    pub command_shell: MethodLibraryCommandShell,
    pub command_source: MethodAssetDistributionHandoffCommandSource,
    pub seam_source: Option<MethodAssetDistributionHandoffSeamSource>,
    pub api_entry_context_ref: MethodAssetApiEntryContextRef,
    pub application_dispatch_ref: MethodAssetApplicationDispatchRef,
}

/// Replay-safe facade output copied from a stored result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDistributionHandoffCommandDispatchOutput {
    pub stored_result_ref: MethodAssetStoredOperationResultRef,
    pub result_kind: MethodAssetStoredOperationResultKind,
    pub replay_marker_ref: MethodAssetReplayMarkerRef,
    pub accepted_summary_ref: Option<MethodAssetAcceptedOperationSummaryRef>,
    pub rejected_reason_ref: Option<MethodAssetSafeRejectReasonRef>,
    pub ignored_reason_ref: Option<MethodAssetSafeIgnoreReasonRef>,
    pub effect_summary_refs: MethodAssetEffectSummaryRefSet,
}

impl From<MethodAssetStoredOperationResult>
    for MethodAssetDistributionHandoffCommandDispatchOutput
{
    fn from(value: MethodAssetStoredOperationResult) -> Self {
        Self {
            stored_result_ref: value.stored_result_ref,
            result_kind: value.result_kind,
            replay_marker_ref: value.replay_marker_ref,
            accepted_summary_ref: value.accepted_summary_ref,
            rejected_reason_ref: value.rejected_reason_ref,
            ignored_reason_ref: value.ignored_reason_ref,
            effect_summary_refs: value.effect_summary_refs,
        }
    }
}

/// Exact selector family derived from the command shell boundary-ref kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodAssetDistributionHandoffCommandSelector {
    PrepareDistributionRef,
    AdjustDistributionContext,
    MarkDistributionAvailability,
}

/// Exact selected service inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetDistributionHandoffServiceInput {
    PrepareDistributionRef(PrepareMethodAssetDistributionRefInput),
    AdjustDistributionContext(AdjustMethodAssetDistributionContextInput),
    MarkDistributionAvailability(MarkMethodAssetDistributionAvailabilityInput),
}

/// Replay fields plus prepare-distribution source fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareMethodAssetDistributionRefInput {
    pub operation_context_ref: MethodAssetOperationContextRef,
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    pub relation_ref: MethodAssetRelationRef,
    pub requested_distribution_ref: Option<MethodAssetDistributionRef>,
    pub distribution_context_ref: DistributionContextRef,
    pub consumption_context_ref: ConsumptionContextRef,
    pub boundary_ref: DownstreamConsumptionBoundaryRef,
    pub availability_marker: MethodAssetConsumptionAvailabilityMarker,
    pub candidate_reason_ref: MethodAssetEventCandidateReasonRef,
    pub seam_source: Option<MethodAssetDistributionHandoffSeamSource>,
}

/// Replay fields plus adjust-context source fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdjustMethodAssetDistributionContextInput {
    pub operation_context_ref: MethodAssetOperationContextRef,
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    pub relation_ref: MethodAssetRelationRef,
    pub distribution_ref: MethodAssetDistributionRef,
    pub previous_context_ref: DistributionContextRef,
    pub new_context_ref: DistributionContextRef,
    pub adjustment_reason_ref: MethodAssetDistributionAdjustmentReasonRef,
    pub candidate_reason_ref: MethodAssetEventCandidateReasonRef,
    pub expected_distribution_version: MethodAssetExpectedVersion,
    pub seam_source: Option<MethodAssetDistributionHandoffSeamSource>,
}

/// Replay fields plus availability-mark source fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkMethodAssetDistributionAvailabilityInput {
    pub operation_context_ref: MethodAssetOperationContextRef,
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    pub relation_ref: MethodAssetRelationRef,
    pub distribution_ref: MethodAssetDistributionRef,
    pub distribution_context_ref: DistributionContextRef,
    pub availability_marker: MethodAssetConsumptionAvailabilityMarker,
    pub candidate_reason_ref: MethodAssetEventCandidateReasonRef,
    pub seam_source: Option<MethodAssetDistributionHandoffSeamSource>,
}

/// Input to the formal replay-envelope helper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDistributionHandoffReplayEnvelopeFactoryInput {
    pub command_shell: MethodLibraryCommandShell,
    pub command_source: MethodAssetDistributionHandoffCommandSource,
    pub seam_source: Option<MethodAssetDistributionHandoffSeamSource>,
    pub selector: MethodAssetDistributionHandoffCommandSelector,
    pub api_entry_context_ref: MethodAssetApiEntryContextRef,
    pub application_dispatch_ref: MethodAssetApplicationDispatchRef,
}

/// Shared replay-envelope fields copied into every selected service input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDistributionHandoffReplayEnvelope {
    pub operation_context_ref: MethodAssetOperationContextRef,
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
}

/// Read-only relation carrier for this boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetRelationReadAnchor {
    pub relation_ref: MethodAssetRelationRef,
    pub distribution_context_ref: Option<DistributionContextRef>,
}

/// Application-owned body-free persisted distribution record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDistributionRecord {
    pub distribution_ref: MethodAssetDistributionRef,
    pub relation_ref: MethodAssetRelationRef,
    pub distribution_context_ref: DistributionContextRef,
    pub consumption_context_ref: ConsumptionContextRef,
    pub boundary_ref: DownstreamConsumptionBoundaryRef,
    pub availability_marker: MethodAssetConsumptionAvailabilityMarker,
}

impl MethodAssetDistributionRecord {
    pub fn prepare(
        distribution_ref: MethodAssetDistributionRef,
        relation_ref: MethodAssetRelationRef,
        distribution_context_ref: DistributionContextRef,
        consumption_context_ref: ConsumptionContextRef,
        boundary_ref: DownstreamConsumptionBoundaryRef,
        availability_marker: MethodAssetConsumptionAvailabilityMarker,
    ) -> Self {
        Self {
            distribution_ref,
            relation_ref,
            distribution_context_ref,
            consumption_context_ref,
            boundary_ref,
            availability_marker,
        }
    }

    pub fn adjust_context(
        &mut self,
        previous_context_ref: DistributionContextRef,
        new_context_ref: DistributionContextRef,
    ) -> Result<(), ()> {
        if self.distribution_context_ref != previous_context_ref {
            return Err(());
        }
        self.distribution_context_ref = new_context_ref;
        Ok(())
    }

    pub fn apply_availability_marker(
        &mut self,
        availability_marker: MethodAssetConsumptionAvailabilityMarker,
    ) {
        self.availability_marker = availability_marker;
    }
}

/// Append-only distribution-specific candidate shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDistributionEventCandidateAssembly {
    pub assembly_ref: MethodAssetEventCandidateAssemblyRef,
    pub operation_context_ref: MethodAssetOperationContextRef,
    pub distribution_ref: MethodAssetDistributionRef,
    pub distribution_context_ref: DistributionContextRef,
    pub candidate_reason_ref: MethodAssetEventCandidateReasonRef,
    pub availability_marker: Option<MethodAssetConsumptionAvailabilityMarker>,
    pub publication_boundary_marker_ref: MethodAssetPublicationBoundaryMarkerRef,
    pub handoff_boundary_marker_ref: Option<MethodAssetHandoffBoundaryMarkerRef>,
}

/// Exact transient builder input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributionReadMaterialBuilderInput {
    pub relation_anchor: MethodAssetRelationReadAnchor,
    pub distribution_ref: MethodAssetDistributionRef,
    pub distribution_context_ref: DistributionContextRef,
    pub consumption_context_ref: ConsumptionContextRef,
    pub boundary_ref: DownstreamConsumptionBoundaryRef,
    pub availability_marker: MethodAssetConsumptionAvailabilityMarker,
}

/// Exact transient builder outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DistributionReadMaterialBuildOutcome {
    Built {
        material_summary_ref: MethodLibraryTypedBoundaryRef,
        distribution_ref: MethodAssetDistributionRef,
        distribution_context_ref: DistributionContextRef,
        availability_marker: Option<MethodAssetConsumptionAvailabilityMarker>,
        effect_summary_ref: MethodLibraryTypedBoundaryRef,
    },
    Unavailable {
        reason_ref: MethodLibrarySafeMarker,
        diagnostic_ref: Option<MethodAssetInfraSafeDiagnosticRef>,
    },
    Rejected {
        reason_ref: MethodLibrarySafeMarker,
    },
}

/// Exact adapter-slot availability summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetAdapterAvailabilitySummary {
    Available {
        availability_state_ref: MethodAssetAdapterAvailabilityStateRef,
        marker_ref: MethodLibrarySafeMarker,
    },
    Degraded {
        availability_state_ref: MethodAssetAdapterAvailabilityStateRef,
        marker_ref: MethodLibrarySafeMarker,
        diagnostic_ref: MethodAssetInfraSafeDiagnosticRef,
    },
    Unavailable {
        availability_state_ref: MethodAssetAdapterAvailabilityStateRef,
        marker_ref: MethodLibrarySafeMarker,
        diagnostic_ref: MethodAssetInfraSafeDiagnosticRef,
    },
    Disabled {
        availability_state_ref: MethodAssetAdapterAvailabilityStateRef,
        marker_ref: MethodLibrarySafeMarker,
        reason_ref: MethodLibrarySafeMarker,
        diagnostic_ref: MethodAssetInfraSafeDiagnosticRef,
    },
}

/// Exact collaboration target summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetCollaborationTargetSummary {
    Enabled {
        target_ref_set: MethodAssetHandoffTargetRefSet,
        summary_marker_ref: MethodLibrarySafeMarker,
    },
    Disabled {
        target_ref_set: MethodAssetHandoffTargetRefSet,
        reason_ref: MethodLibrarySafeMarker,
        diagnostic_ref: MethodAssetInfraSafeDiagnosticRef,
    },
    Blocked {
        target_ref_set: MethodAssetHandoffTargetRefSet,
        reason_ref: MethodLibrarySafeMarker,
        diagnostic_ref: MethodAssetInfraSafeDiagnosticRef,
    },
    Unavailable {
        target_ref_set: MethodAssetHandoffTargetRefSet,
        reason_ref: MethodLibrarySafeMarker,
        diagnostic_ref: MethodAssetInfraSafeDiagnosticRef,
    },
}

impl MethodAssetCollaborationTargetSummary {
    fn target_ref_set(&self) -> &MethodAssetHandoffTargetRefSet {
        match self {
            Self::Enabled { target_ref_set, .. }
            | Self::Disabled { target_ref_set, .. }
            | Self::Blocked { target_ref_set, .. }
            | Self::Unavailable { target_ref_set, .. } => target_ref_set,
        }
    }
}

/// Exact event-candidate publisher input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetEventCandidatePublicationInput {
    pub publication_ref: MethodAssetPublicationOutcomeRef,
    pub candidate: MethodAssetDistributionEventCandidateAssembly,
    pub target_summary: MethodAssetCollaborationTargetSummary,
    pub boundary_marker_ref: MethodAssetPublicationBoundaryMarkerRef,
}

/// Body-free publication outcome shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetPublicationOutcome {
    Published {
        publication_ref: MethodAssetPublicationOutcomeRef,
        candidate_ref: MethodAssetEventCandidateAssemblyRef,
        target_ref_set: MethodAssetHandoffTargetRefSet,
        boundary_marker_ref: MethodAssetPublicationBoundaryMarkerRef,
    },
    Blocked {
        publication_ref: MethodAssetPublicationOutcomeRef,
        candidate_ref: MethodAssetEventCandidateAssemblyRef,
        target_ref_set: MethodAssetHandoffTargetRefSet,
        boundary_marker_ref: MethodAssetPublicationBoundaryMarkerRef,
        reason_ref: MethodLibrarySafeMarker,
        diagnostic_ref: MethodAssetInfraSafeDiagnosticRef,
    },
    Unavailable {
        publication_ref: MethodAssetPublicationOutcomeRef,
        candidate_ref: MethodAssetEventCandidateAssemblyRef,
        target_ref_set: MethodAssetHandoffTargetRefSet,
        boundary_marker_ref: MethodAssetPublicationBoundaryMarkerRef,
        reason_ref: MethodLibrarySafeMarker,
        diagnostic_ref: MethodAssetInfraSafeDiagnosticRef,
    },
    Failed {
        publication_ref: MethodAssetPublicationOutcomeRef,
        candidate_ref: MethodAssetEventCandidateAssemblyRef,
        target_ref_set: MethodAssetHandoffTargetRefSet,
        boundary_marker_ref: MethodAssetPublicationBoundaryMarkerRef,
        reason_ref: MethodLibrarySafeMarker,
        diagnostic_ref: MethodAssetInfraSafeDiagnosticRef,
    },
}

impl MethodAssetPublicationOutcome {
    pub fn publication_ref(&self) -> &MethodAssetPublicationOutcomeRef {
        match self {
            Self::Published {
                publication_ref, ..
            }
            | Self::Blocked {
                publication_ref, ..
            }
            | Self::Unavailable {
                publication_ref, ..
            }
            | Self::Failed {
                publication_ref, ..
            } => publication_ref,
        }
    }
}

/// Exact body-free collaboration handoff input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDistributionHandoffInput {
    pub handoff_ref: MethodAssetHandoffMarkerRef,
    pub distribution_ref: MethodAssetDistributionRef,
    pub distribution_context_ref: DistributionContextRef,
    pub candidate_ref: MethodAssetEventCandidateAssemblyRef,
    pub target_ref: MethodAssetHandoffTargetRef,
    pub boundary_marker_ref: MethodAssetHandoffBoundaryMarkerRef,
    pub diagnostic_ref: Option<MethodAssetInfraSafeDiagnosticRef>,
    pub follow_up_hint_ref: Option<MethodLibrarySafeMarker>,
}

/// Body-free handoff outcome shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetHandoffOutcome {
    Prepared {
        handoff_ref: MethodAssetHandoffMarkerRef,
        target_ref: MethodAssetHandoffTargetRef,
        boundary_marker_ref: MethodAssetHandoffBoundaryMarkerRef,
        hint_ref: Option<MethodLibrarySafeMarker>,
    },
    Delivered {
        handoff_ref: MethodAssetHandoffMarkerRef,
        target_ref: MethodAssetHandoffTargetRef,
        boundary_marker_ref: MethodAssetHandoffBoundaryMarkerRef,
        receipt_marker_ref: MethodLibrarySafeMarker,
    },
    Blocked {
        handoff_ref: MethodAssetHandoffMarkerRef,
        target_ref: MethodAssetHandoffTargetRef,
        boundary_marker_ref: MethodAssetHandoffBoundaryMarkerRef,
        reason_ref: MethodLibrarySafeMarker,
        diagnostic_ref: MethodAssetInfraSafeDiagnosticRef,
    },
    Unavailable {
        handoff_ref: MethodAssetHandoffMarkerRef,
        target_ref: MethodAssetHandoffTargetRef,
        boundary_marker_ref: MethodAssetHandoffBoundaryMarkerRef,
        reason_ref: MethodLibrarySafeMarker,
        diagnostic_ref: MethodAssetInfraSafeDiagnosticRef,
    },
    Failed {
        handoff_ref: MethodAssetHandoffMarkerRef,
        target_ref: MethodAssetHandoffTargetRef,
        boundary_marker_ref: MethodAssetHandoffBoundaryMarkerRef,
        reason_ref: MethodLibrarySafeMarker,
        diagnostic_ref: MethodAssetInfraSafeDiagnosticRef,
    },
}

impl MethodAssetHandoffOutcome {
    pub fn handoff_ref(&self) -> &MethodAssetHandoffMarkerRef {
        match self {
            Self::Prepared { handoff_ref, .. }
            | Self::Delivered { handoff_ref, .. }
            | Self::Blocked { handoff_ref, .. }
            | Self::Unavailable { handoff_ref, .. }
            | Self::Failed { handoff_ref, .. } => handoff_ref,
        }
    }
}

/// Read-only relation repository for current-boundary distribution flows.
pub trait MethodAssetRelationRepository: Send + Sync {
    fn get_relation_anchor_with_version(
        &self,
        relation_ref: MethodAssetRelationRef,
    ) -> Result<Option<Versioned<MethodAssetRelationReadAnchor>>, MethodAssetRepositoryError>;
}

/// Application-owned distribution record repository.
pub trait MethodAssetDistributionRepository: Send + Sync {
    fn get_distribution_with_version(
        &self,
        distribution_ref: MethodAssetDistributionRef,
    ) -> Result<Option<Versioned<MethodAssetDistributionRecord>>, MethodAssetRepositoryError>;

    fn save_distribution(
        &self,
        record: MethodAssetDistributionRecord,
        expected_version: Option<MethodAssetExpectedVersion>,
        uow: &mut dyn CommandUnitOfWork,
    ) -> Result<VersionedRef<MethodAssetDistributionRef>, MethodAssetRepositoryError>;
}

pub trait DistributionReadMaterialBuilderPort: Send + Sync {
    fn build_distribution_read_material(
        &self,
        input: DistributionReadMaterialBuilderInput,
    ) -> DistributionReadMaterialBuildOutcome;
}

pub trait MethodAssetConsumptionAvailabilityResolverPort: Send + Sync {
    fn resolve_distribution_availability(
        &self,
        marker: MethodAssetConsumptionAvailabilityMarker,
    ) -> MethodAssetConsumptionAvailabilityMarker;
}

pub trait MethodAssetDegradedDecisionMapperPort: Send + Sync {
    fn map_degraded_decision(
        &self,
        marker: MethodAssetConsumptionAvailabilityMarker,
        diagnostic_ref: Option<MethodAssetInfraSafeDiagnosticRef>,
    ) -> Option<MethodAssetDegradedDecisionRef>;
}

pub trait MethodAssetAdapterAvailabilityPort: Send + Sync {
    fn check_required_distribution_slots(
        &self,
        required_slot_refs: MethodAssetAdapterSlotRefSet,
    ) -> MethodAssetAdapterAvailabilitySummary;
}

pub trait MethodAssetCollaborationTargetRegistryPort: Send + Sync {
    fn resolve_distribution_targets(
        &self,
        scope_ref: MethodAssetTargetRegistryScopeRef,
        publisher_binding_ref: MethodAssetPublisherBindingStateRef,
        handoff_binding_ref: Option<MethodAssetHandoffBindingStateRef>,
    ) -> MethodAssetCollaborationTargetSummary;
}

pub trait MethodAssetEventCandidatePublisherPort: Send + Sync {
    fn publish_event_candidate(
        &self,
        input: MethodAssetEventCandidatePublicationInput,
    ) -> MethodAssetPublicationOutcome;
}

pub trait MethodAssetCollaborationHandoffPort: Send + Sync {
    fn prepare_distribution_handoff(
        &self,
        input: MethodAssetDistributionHandoffInput,
    ) -> MethodAssetHandoffOutcome;
}

pub trait MethodAssetEventCandidateAssemblyRepository: Send + Sync {
    fn append_event_candidate_assembly(
        &self,
        assembly: MethodAssetDistributionEventCandidateAssembly,
        uow: &mut dyn CommandUnitOfWork,
    ) -> Result<VersionedRef<MethodAssetEventCandidateAssemblyRef>, MethodAssetRepositoryError>;

    fn get_event_candidate_assembly(
        &self,
        assembly_ref: MethodAssetEventCandidateAssemblyRef,
    ) -> Result<
        Option<Versioned<MethodAssetDistributionEventCandidateAssembly>>,
        MethodAssetRepositoryError,
    >;
}

pub trait MethodAssetPublicationOutcomeRepository: Send + Sync {
    fn save_publication_outcome(
        &self,
        outcome: MethodAssetPublicationOutcome,
        uow: &mut dyn CommandUnitOfWork,
    ) -> Result<VersionedRef<MethodAssetPublicationOutcomeRef>, MethodAssetRepositoryError>;

    fn get_publication_outcome(
        &self,
        publication_ref: MethodAssetPublicationOutcomeRef,
    ) -> Result<Option<Versioned<MethodAssetPublicationOutcome>>, MethodAssetRepositoryError>;
}

pub trait MethodAssetHandoffMarkerRepository: Send + Sync {
    fn save_handoff_marker(
        &self,
        outcome: MethodAssetHandoffOutcome,
        uow: &mut dyn CommandUnitOfWork,
    ) -> Result<VersionedRef<MethodAssetHandoffMarkerRef>, MethodAssetRepositoryError>;

    fn get_handoff_marker(
        &self,
        handoff_ref: MethodAssetHandoffMarkerRef,
    ) -> Result<Option<Versioned<MethodAssetHandoffOutcome>>, MethodAssetRepositoryError>;
}

/// Formal factory for replay, result, distribution and post-commit seam identities.
pub trait MethodAssetDistributionHandoffSupportRefFactory: Send {
    fn distribution_handoff_dispatch_ref(&self) -> MethodAssetApplicationDispatchRef;
    fn new_api_entry_context_ref(&mut self) -> MethodAssetApiEntryContextRef;
    fn build_distribution_handoff_replay_envelope(
        &mut self,
        input: MethodAssetDistributionHandoffReplayEnvelopeFactoryInput,
    ) -> Result<MethodAssetDistributionHandoffReplayEnvelope, MethodAssetReplayEnvelopeBuildError>;
    fn new_stored_operation_result_ref(&mut self) -> MethodAssetStoredOperationResultRef;
    fn new_accepted_operation_summary_ref(&mut self) -> MethodAssetAcceptedOperationSummaryRef;
    fn new_safe_reject_reason_ref(&mut self) -> MethodAssetSafeRejectReasonRef;
    fn new_safe_ignore_reason_ref(&mut self) -> MethodAssetSafeIgnoreReasonRef;
    fn new_effect_summary_ref(&mut self) -> MethodAssetEffectSummaryRef;
    fn new_replay_marker_ref(&mut self) -> MethodAssetReplayMarkerRef;
    fn new_distribution_ref(
        &mut self,
        relation_ref: MethodAssetRelationRef,
        distribution_context_ref: DistributionContextRef,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> MethodAssetDistributionRef;
    fn new_event_candidate_assembly_ref(
        &mut self,
        distribution_ref: MethodAssetDistributionRef,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> MethodAssetEventCandidateAssemblyRef;
    fn new_publication_outcome_ref(
        &mut self,
        candidate_ref: MethodAssetEventCandidateAssemblyRef,
        target_ref_set: MethodAssetHandoffTargetRefSet,
    ) -> MethodAssetPublicationOutcomeRef;
    fn new_handoff_marker_ref(
        &mut self,
        candidate_ref: MethodAssetEventCandidateAssemblyRef,
        target_ref: MethodAssetHandoffTargetRef,
    ) -> MethodAssetHandoffMarkerRef;
}

pub trait MethodAssetDistributionHandoffCommandFacade: Send + Sync {
    fn dispatch_distribution_handoff_command(
        &self,
        input: MethodAssetDistributionHandoffCommandDispatchInput,
    ) -> MethodAssetDistributionHandoffCommandDispatchOutput;
}

enum ServiceExecution {
    Persisted {
        stored_result: MethodAssetStoredOperationResult,
        distribution_ref: Option<MethodAssetDistributionRef>,
        candidate: Option<(
            MethodAssetEventCandidateAssemblyRef,
            MethodAssetDistributionHandoffSeamSource,
        )>,
    },
    Ephemeral(MethodAssetStoredOperationResult),
}

/// Default distribution/handoff command facade.
pub struct DefaultMethodAssetDistributionHandoffCommandFacade {
    relation_repository: Arc<dyn MethodAssetRelationRepository>,
    distribution_repository: Arc<dyn MethodAssetDistributionRepository>,
    builder: Arc<dyn DistributionReadMaterialBuilderPort>,
    availability_resolver: Arc<dyn MethodAssetConsumptionAvailabilityResolverPort>,
    degraded_mapper: Arc<dyn MethodAssetDegradedDecisionMapperPort>,
    adapter_availability: Arc<dyn MethodAssetAdapterAvailabilityPort>,
    target_registry: Arc<dyn MethodAssetCollaborationTargetRegistryPort>,
    publisher: Arc<dyn MethodAssetEventCandidatePublisherPort>,
    handoff: Arc<dyn MethodAssetCollaborationHandoffPort>,
    candidate_repository: Arc<dyn MethodAssetEventCandidateAssemblyRepository>,
    publication_repository: Arc<dyn MethodAssetPublicationOutcomeRepository>,
    handoff_repository: Arc<dyn MethodAssetHandoffMarkerRepository>,
    stored_result_repository: Arc<dyn MethodAssetStoredOperationResultRepository>,
    unit_of_work: Arc<dyn UnitOfWork>,
    support_ref_factory: Arc<Mutex<Box<dyn MethodAssetDistributionHandoffSupportRefFactory>>>,
}

impl DefaultMethodAssetDistributionHandoffCommandFacade {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        relation_repository: Arc<dyn MethodAssetRelationRepository>,
        distribution_repository: Arc<dyn MethodAssetDistributionRepository>,
        builder: Arc<dyn DistributionReadMaterialBuilderPort>,
        availability_resolver: Arc<dyn MethodAssetConsumptionAvailabilityResolverPort>,
        degraded_mapper: Arc<dyn MethodAssetDegradedDecisionMapperPort>,
        adapter_availability: Arc<dyn MethodAssetAdapterAvailabilityPort>,
        target_registry: Arc<dyn MethodAssetCollaborationTargetRegistryPort>,
        publisher: Arc<dyn MethodAssetEventCandidatePublisherPort>,
        handoff: Arc<dyn MethodAssetCollaborationHandoffPort>,
        candidate_repository: Arc<dyn MethodAssetEventCandidateAssemblyRepository>,
        publication_repository: Arc<dyn MethodAssetPublicationOutcomeRepository>,
        handoff_repository: Arc<dyn MethodAssetHandoffMarkerRepository>,
        stored_result_repository: Arc<dyn MethodAssetStoredOperationResultRepository>,
        unit_of_work: Arc<dyn UnitOfWork>,
        support_ref_factory: Arc<Mutex<Box<dyn MethodAssetDistributionHandoffSupportRefFactory>>>,
    ) -> Self {
        Self {
            relation_repository,
            distribution_repository,
            builder,
            availability_resolver,
            degraded_mapper,
            adapter_availability,
            target_registry,
            publisher,
            handoff,
            candidate_repository,
            publication_repository,
            handoff_repository,
            stored_result_repository,
            unit_of_work,
            support_ref_factory,
        }
    }

    fn with_factory<R>(
        &self,
        action: impl FnOnce(&mut dyn MethodAssetDistributionHandoffSupportRefFactory) -> R,
    ) -> R {
        let mut factory = self
            .support_ref_factory
            .lock()
            .expect("support ref factory lock poisoned");
        action(factory.as_mut())
    }

    fn new_safe_reject_reason_ref(&self) -> MethodAssetSafeRejectReasonRef {
        self.with_factory(|factory| factory.new_safe_reject_reason_ref())
    }

    fn new_result(
        &self,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        result_kind: MethodAssetStoredOperationResultKind,
        accepted_summary_ref: Option<MethodAssetAcceptedOperationSummaryRef>,
        rejected_reason_ref: Option<MethodAssetSafeRejectReasonRef>,
        effect_summary_refs: MethodAssetEffectSummaryRefSet,
    ) -> MethodAssetStoredOperationResult {
        MethodAssetStoredOperationResult {
            stored_result_ref: self
                .with_factory(|factory| factory.new_stored_operation_result_ref()),
            operation_context_ref,
            operation_digest_ref,
            result_kind,
            accepted_summary_ref,
            rejected_reason_ref,
            ignored_reason_ref: None,
            effect_summary_refs,
            replay_marker_ref: self.with_factory(|factory| factory.new_replay_marker_ref()),
        }
    }

    fn ephemeral_rejection(
        &self,
        envelope: &MethodAssetDistributionHandoffReplayEnvelope,
        kind: MethodAssetStoredOperationResultKind,
    ) -> MethodAssetStoredOperationResult {
        self.new_result(
            envelope.operation_context_ref.clone(),
            envelope.operation_digest_ref.clone(),
            kind,
            None,
            Some(self.new_safe_reject_reason_ref()),
            MethodAssetEffectSummaryRefSet::new(),
        )
    }

    fn early_rejection(
        &self,
        reason_ref: MethodAssetSafeRejectReasonRef,
    ) -> MethodAssetDistributionHandoffCommandDispatchOutput {
        MethodAssetDistributionHandoffCommandDispatchOutput {
            stored_result_ref: self
                .with_factory(|factory| factory.new_stored_operation_result_ref()),
            result_kind: MethodAssetStoredOperationResultKind::Rejected,
            replay_marker_ref: self.with_factory(|factory| factory.new_replay_marker_ref()),
            accepted_summary_ref: None,
            rejected_reason_ref: Some(reason_ref),
            ignored_reason_ref: None,
            effect_summary_refs: MethodAssetEffectSummaryRefSet::new(),
        }
    }

    fn persisted_result(
        &self,
        envelope: &MethodAssetDistributionHandoffReplayEnvelope,
        accepted: bool,
        uow: &mut dyn CommandUnitOfWork,
        distribution_ref: Option<MethodAssetDistributionRef>,
        candidate: Option<(
            MethodAssetEventCandidateAssemblyRef,
            MethodAssetDistributionHandoffSeamSource,
        )>,
    ) -> ServiceExecution {
        let (kind, accepted_summary_ref, rejected_reason_ref, effect_summary_refs) = if accepted {
            (
                MethodAssetStoredOperationResultKind::Accepted,
                Some(self.with_factory(|factory| factory.new_accepted_operation_summary_ref())),
                None,
                MethodAssetEffectSummaryRefSet::from_refs([
                    self.with_factory(|factory| factory.new_effect_summary_ref())
                ]),
            )
        } else {
            (
                MethodAssetStoredOperationResultKind::Rejected,
                None,
                Some(self.new_safe_reject_reason_ref()),
                MethodAssetEffectSummaryRefSet::new(),
            )
        };
        let stored_result = self.new_result(
            envelope.operation_context_ref.clone(),
            envelope.operation_digest_ref.clone(),
            kind,
            accepted_summary_ref,
            rejected_reason_ref,
            effect_summary_refs,
        );
        match self
            .stored_result_repository
            .save_command_result_for_idempotency(
                envelope.idempotency_key_ref.clone(),
                envelope.dedup_scope_ref.clone(),
                envelope.operation_digest_ref.clone(),
                stored_result.clone(),
                uow,
            ) {
            Ok(_) => ServiceExecution::Persisted {
                stored_result,
                distribution_ref,
                candidate,
            },
            Err(error) => {
                ServiceExecution::Ephemeral(self.ephemeral_from_repository_error(envelope, error))
            }
        }
    }

    fn ephemeral_from_repository_error(
        &self,
        envelope: &MethodAssetDistributionHandoffReplayEnvelope,
        error: MethodAssetRepositoryError,
    ) -> MethodAssetStoredOperationResult {
        let kind = if matches!(
            error,
            MethodAssetRepositoryError::StoredResultIntegrityViolation { .. }
        ) {
            MethodAssetStoredOperationResultKind::Conflict
        } else {
            MethodAssetStoredOperationResultKind::Rejected
        };
        self.ephemeral_rejection(envelope, kind)
    }

    fn selector_from_shell(
        &self,
        command_shell: &MethodLibraryCommandShell,
    ) -> Result<
        MethodAssetDistributionHandoffCommandSelector,
        MethodAssetDistributionHandoffCommandDispatchOutput,
    > {
        if command_shell.capability_kind != MethodLibraryCapabilityKind::RelationDistribution {
            return Err(self.early_rejection(self.new_safe_reject_reason_ref()));
        }
        match command_shell.boundary_ref.kind() {
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionRefPrepareIntent => {
                Ok(MethodAssetDistributionHandoffCommandSelector::PrepareDistributionRef)
            }
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionContextAdjustIntent => {
                Ok(MethodAssetDistributionHandoffCommandSelector::AdjustDistributionContext)
            }
            MethodLibraryTypedBoundaryRefKind::MethodAssetDistributionAvailabilityMarkIntent => {
                Ok(MethodAssetDistributionHandoffCommandSelector::MarkDistributionAvailability)
            }
            _ => Err(self.early_rejection(self.new_safe_reject_reason_ref())),
        }
    }

    fn build_replay_envelope(
        &self,
        input: &MethodAssetDistributionHandoffCommandDispatchInput,
        selector: MethodAssetDistributionHandoffCommandSelector,
    ) -> Result<
        MethodAssetDistributionHandoffReplayEnvelope,
        MethodAssetDistributionHandoffCommandDispatchOutput,
    > {
        self.with_factory(|factory| {
            factory.build_distribution_handoff_replay_envelope(
                MethodAssetDistributionHandoffReplayEnvelopeFactoryInput {
                    command_shell: input.command_shell.clone(),
                    command_source: input.command_source.clone(),
                    seam_source: input.seam_source.clone(),
                    selector,
                    api_entry_context_ref: input.api_entry_context_ref.clone(),
                    application_dispatch_ref: input.application_dispatch_ref.clone(),
                },
            )
        })
        .map_err(|error| match error {
            MethodAssetReplayEnvelopeBuildError::MissingIdempotencyKey { reason_ref }
            | MethodAssetReplayEnvelopeBuildError::UnsupportedDispatchTarget { reason_ref }
            | MethodAssetReplayEnvelopeBuildError::SourceSelectorMismatch { reason_ref }
            | MethodAssetReplayEnvelopeBuildError::OpaqueRefGenerationUnavailable { reason_ref } => {
                self.early_rejection(reason_ref)
            }
        })
    }

    fn duplicate_or_conflict(
        &self,
        envelope: &MethodAssetDistributionHandoffReplayEnvelope,
    ) -> Result<
        Option<MethodAssetStoredOperationResult>,
        MethodAssetDistributionHandoffCommandDispatchOutput,
    > {
        match self
            .stored_result_repository
            .find_command_result_by_idempotency(
                envelope.idempotency_key_ref.clone(),
                envelope.dedup_scope_ref.clone(),
            ) {
            Ok(Some(result)) if result.operation_digest_ref == envelope.operation_digest_ref => {
                Ok(Some(result))
            }
            Ok(Some(_)) => Err(self
                .ephemeral_rejection(envelope, MethodAssetStoredOperationResultKind::Conflict)
                .into()),
            Ok(None) => Ok(None),
            Err(error) => Err(self.ephemeral_from_repository_error(envelope, error).into()),
        }
    }

    fn relation_anchor(
        &self,
        relation_ref: MethodAssetRelationRef,
    ) -> Result<Option<MethodAssetRelationReadAnchor>, MethodAssetRepositoryError> {
        Ok(self
            .relation_repository
            .get_relation_anchor_with_version(relation_ref)?
            .map(|value| value.value)
            .filter(|value| {
                value.relation_ref.as_typed_ref().kind() == MethodAssetRelationRef::expected_kind()
                    && value
                        .distribution_context_ref
                        .as_ref()
                        .is_none_or(|context_ref| {
                            context_ref.as_typed_ref().kind()
                                == DistributionContextRef::expected_kind()
                        })
            }))
    }

    fn candidate_for(
        &self,
        envelope: &MethodAssetDistributionHandoffReplayEnvelope,
        distribution_ref: MethodAssetDistributionRef,
        distribution_context_ref: DistributionContextRef,
        candidate_reason_ref: MethodAssetEventCandidateReasonRef,
        availability_marker: Option<MethodAssetConsumptionAvailabilityMarker>,
        seam_source: &MethodAssetDistributionHandoffSeamSource,
    ) -> MethodAssetDistributionEventCandidateAssembly {
        let assembly_ref = self.with_factory(|factory| {
            factory.new_event_candidate_assembly_ref(
                distribution_ref.clone(),
                envelope.operation_context_ref.clone(),
                envelope.operation_digest_ref.clone(),
                envelope.dedup_scope_ref.clone(),
            )
        });
        MethodAssetDistributionEventCandidateAssembly {
            assembly_ref,
            operation_context_ref: envelope.operation_context_ref.clone(),
            distribution_ref,
            distribution_context_ref,
            candidate_reason_ref,
            availability_marker,
            publication_boundary_marker_ref: seam_source.publication_boundary_marker_ref.clone(),
            handoff_boundary_marker_ref: seam_source.handoff_boundary_marker_ref.clone(),
        }
    }

    fn prepare_distribution(
        &self,
        input: PrepareMethodAssetDistributionRefInput,
        envelope: &MethodAssetDistributionHandoffReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        if input
            .seam_source
            .as_ref()
            .is_some_and(|value| !value.is_valid())
        {
            return self.persisted_result(envelope, false, uow, None, None);
        }
        let relation_anchor = match self.relation_anchor(input.relation_ref.clone()) {
            Ok(Some(value)) => value,
            Ok(None) => return self.persisted_result(envelope, false, uow, None, None),
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_from_repository_error(envelope, error),
                );
            }
        };
        if relation_anchor.relation_ref != input.relation_ref
            || relation_anchor
                .distribution_context_ref
                .as_ref()
                .is_some_and(|value| value != &input.distribution_context_ref)
        {
            return self.persisted_result(envelope, false, uow, None, None);
        }

        let distribution_ref = input.requested_distribution_ref.unwrap_or_else(|| {
            self.with_factory(|factory| {
                factory.new_distribution_ref(
                    input.relation_ref.clone(),
                    input.distribution_context_ref.clone(),
                    input.operation_context_ref.clone(),
                    input.operation_digest_ref.clone(),
                    input.dedup_scope_ref.clone(),
                )
            })
        });
        match self
            .distribution_repository
            .get_distribution_with_version(distribution_ref.clone())
        {
            Ok(Some(_)) => return self.persisted_result(envelope, false, uow, None, None),
            Ok(None) => {}
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_from_repository_error(envelope, error),
                );
            }
        }

        let record = MethodAssetDistributionRecord::prepare(
            distribution_ref.clone(),
            input.relation_ref,
            input.distribution_context_ref.clone(),
            input.consumption_context_ref.clone(),
            input.boundary_ref.clone(),
            input.availability_marker.clone(),
        );
        let built_availability_marker = match self.builder.build_distribution_read_material(
            DistributionReadMaterialBuilderInput {
                relation_anchor,
                distribution_ref: distribution_ref.clone(),
                distribution_context_ref: input.distribution_context_ref.clone(),
                consumption_context_ref: input.consumption_context_ref,
                boundary_ref: input.boundary_ref,
                availability_marker: input.availability_marker.clone(),
            },
        ) {
            DistributionReadMaterialBuildOutcome::Built {
                distribution_ref: built_distribution_ref,
                distribution_context_ref: built_context_ref,
                availability_marker,
                ..
            } if built_distribution_ref == record.distribution_ref
                && built_context_ref == record.distribution_context_ref =>
            {
                availability_marker
            }
            DistributionReadMaterialBuildOutcome::Built { .. }
            | DistributionReadMaterialBuildOutcome::Unavailable { .. }
            | DistributionReadMaterialBuildOutcome::Rejected { .. } => {
                return self.persisted_result(envelope, false, uow, None, None);
            }
        };

        let candidate = input.seam_source.as_ref().map(|seam_source| {
            self.candidate_for(
                envelope,
                distribution_ref.clone(),
                input.distribution_context_ref,
                input.candidate_reason_ref,
                built_availability_marker,
                seam_source,
            )
        });
        if let Err(error) = self
            .distribution_repository
            .save_distribution(record, None, uow)
        {
            return ServiceExecution::Ephemeral(
                self.ephemeral_from_repository_error(envelope, error),
            );
        }
        let candidate_context =
            if let (Some(candidate), Some(seam_source)) = (candidate, input.seam_source) {
                if let Err(error) = self
                    .candidate_repository
                    .append_event_candidate_assembly(candidate.clone(), uow)
                {
                    return ServiceExecution::Ephemeral(
                        self.ephemeral_from_repository_error(envelope, error),
                    );
                }
                Some((candidate.assembly_ref, seam_source))
            } else {
                None
            };
        self.persisted_result(
            envelope,
            true,
            uow,
            Some(distribution_ref),
            candidate_context,
        )
    }

    fn adjust_distribution(
        &self,
        input: AdjustMethodAssetDistributionContextInput,
        envelope: &MethodAssetDistributionHandoffReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        if input
            .seam_source
            .as_ref()
            .is_some_and(|value| !value.is_valid())
        {
            return self.persisted_result(envelope, false, uow, None, None);
        }
        let relation_anchor = match self.relation_anchor(input.relation_ref.clone()) {
            Ok(Some(value)) => value,
            Ok(None) => return self.persisted_result(envelope, false, uow, None, None),
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_from_repository_error(envelope, error),
                );
            }
        };
        let loaded = match self
            .distribution_repository
            .get_distribution_with_version(input.distribution_ref.clone())
        {
            Ok(Some(value)) => value,
            Ok(None) => return self.persisted_result(envelope, false, uow, None, None),
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_from_repository_error(envelope, error),
                );
            }
        };
        if relation_anchor.relation_ref != input.relation_ref
            || loaded.value.relation_ref != input.relation_ref
            || MethodAssetExpectedVersion::from(loaded.version)
                != input.expected_distribution_version
            || relation_anchor
                .distribution_context_ref
                .as_ref()
                .is_some_and(|value| value != &input.previous_context_ref)
        {
            return self.persisted_result(envelope, false, uow, None, None);
        }

        let mut record = loaded.value;
        if record
            .adjust_context(input.previous_context_ref, input.new_context_ref.clone())
            .is_err()
        {
            return self.persisted_result(envelope, false, uow, None, None);
        }
        let candidate = input.seam_source.as_ref().map(|seam_source| {
            self.candidate_for(
                envelope,
                input.distribution_ref.clone(),
                input.new_context_ref,
                input.candidate_reason_ref,
                None,
                seam_source,
            )
        });
        if let Err(error) = self.distribution_repository.save_distribution(
            record,
            Some(input.expected_distribution_version),
            uow,
        ) {
            return ServiceExecution::Ephemeral(
                self.ephemeral_from_repository_error(envelope, error),
            );
        }
        let candidate_context =
            if let (Some(candidate), Some(seam_source)) = (candidate, input.seam_source) {
                if let Err(error) = self
                    .candidate_repository
                    .append_event_candidate_assembly(candidate.clone(), uow)
                {
                    return ServiceExecution::Ephemeral(
                        self.ephemeral_from_repository_error(envelope, error),
                    );
                }
                Some((candidate.assembly_ref, seam_source))
            } else {
                None
            };
        self.persisted_result(
            envelope,
            true,
            uow,
            Some(input.distribution_ref),
            candidate_context,
        )
    }

    fn mark_distribution_availability(
        &self,
        input: MarkMethodAssetDistributionAvailabilityInput,
        envelope: &MethodAssetDistributionHandoffReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        if input
            .seam_source
            .as_ref()
            .is_some_and(|value| !value.is_valid())
        {
            return self.persisted_result(envelope, false, uow, None, None);
        }
        let relation_anchor = match self.relation_anchor(input.relation_ref.clone()) {
            Ok(Some(value)) => value,
            Ok(None) => return self.persisted_result(envelope, false, uow, None, None),
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_from_repository_error(envelope, error),
                );
            }
        };
        let loaded = match self
            .distribution_repository
            .get_distribution_with_version(input.distribution_ref.clone())
        {
            Ok(Some(value)) => value,
            Ok(None) => return self.persisted_result(envelope, false, uow, None, None),
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_from_repository_error(envelope, error),
                );
            }
        };
        if relation_anchor.relation_ref != input.relation_ref
            || loaded.value.relation_ref != input.relation_ref
            || loaded.value.distribution_context_ref != input.distribution_context_ref
            || relation_anchor
                .distribution_context_ref
                .as_ref()
                .is_some_and(|value| value != &input.distribution_context_ref)
        {
            return self.persisted_result(envelope, false, uow, None, None);
        }
        let marker = self
            .availability_resolver
            .resolve_distribution_availability(input.availability_marker);
        let degraded_decision = self
            .degraded_mapper
            .map_degraded_decision(marker.clone(), None);
        let decision_required = !matches!(
            marker.target_state,
            MethodAssetConsumptionAvailabilityTarget::Ready
        );
        if decision_required != degraded_decision.is_some() {
            return self.persisted_result(envelope, false, uow, None, None);
        }
        let expected_version = MethodAssetExpectedVersion::from(loaded.version);
        let mut record = loaded.value;
        record.apply_availability_marker(marker.clone());
        let candidate = input.seam_source.as_ref().map(|seam_source| {
            self.candidate_for(
                envelope,
                input.distribution_ref.clone(),
                input.distribution_context_ref,
                input.candidate_reason_ref,
                Some(marker),
                seam_source,
            )
        });
        if let Err(error) =
            self.distribution_repository
                .save_distribution(record, Some(expected_version), uow)
        {
            return ServiceExecution::Ephemeral(
                self.ephemeral_from_repository_error(envelope, error),
            );
        }
        let candidate_context =
            if let (Some(candidate), Some(seam_source)) = (candidate, input.seam_source) {
                if let Err(error) = self
                    .candidate_repository
                    .append_event_candidate_assembly(candidate.clone(), uow)
                {
                    return ServiceExecution::Ephemeral(
                        self.ephemeral_from_repository_error(envelope, error),
                    );
                }
                Some((candidate.assembly_ref, seam_source))
            } else {
                None
            };
        self.persisted_result(
            envelope,
            true,
            uow,
            Some(input.distribution_ref),
            candidate_context,
        )
    }

    fn commit_unknown_is_durable(
        &self,
        stored_result: &MethodAssetStoredOperationResult,
        distribution_ref: Option<MethodAssetDistributionRef>,
        candidate_ref: Option<MethodAssetEventCandidateAssemblyRef>,
    ) -> bool {
        let stored = self
            .stored_result_repository
            .get_stored_operation_result(stored_result.stored_result_ref.clone())
            .ok()
            .flatten()
            .is_some_and(|value| value.operation_digest_ref == stored_result.operation_digest_ref);
        let distribution = distribution_ref.is_none_or(|value| {
            self.distribution_repository
                .get_distribution_with_version(value)
                .ok()
                .flatten()
                .is_some()
        });
        let candidate = candidate_ref.is_none_or(|value| {
            self.candidate_repository
                .get_event_candidate_assembly(value)
                .ok()
                .flatten()
                .is_some()
        });
        stored && distribution && candidate
    }

    fn execute_fresh(
        &self,
        selector: MethodAssetDistributionHandoffCommandSelector,
        source: MethodAssetDistributionHandoffCommandSource,
        seam_source: Option<MethodAssetDistributionHandoffSeamSource>,
        envelope: MethodAssetDistributionHandoffReplayEnvelope,
    ) -> MethodAssetStoredOperationResult {
        let mut uow = self.unit_of_work.begin_command_uow();
        let execution = match (selector, source) {
            (
                MethodAssetDistributionHandoffCommandSelector::PrepareDistributionRef,
                MethodAssetDistributionHandoffCommandSource::PrepareDistributionRef {
                    relation_ref,
                    requested_distribution_ref,
                    distribution_context_ref,
                    consumption_context_ref,
                    boundary_ref,
                    availability_marker,
                    candidate_reason_ref,
                },
            ) => self.prepare_distribution(
                PrepareMethodAssetDistributionRefInput {
                    operation_context_ref: envelope.operation_context_ref.clone(),
                    idempotency_key_ref: envelope.idempotency_key_ref.clone(),
                    operation_digest_ref: envelope.operation_digest_ref.clone(),
                    dedup_scope_ref: envelope.dedup_scope_ref.clone(),
                    relation_ref,
                    requested_distribution_ref,
                    distribution_context_ref,
                    consumption_context_ref,
                    boundary_ref,
                    availability_marker,
                    candidate_reason_ref,
                    seam_source,
                },
                &envelope,
                uow.as_mut(),
            ),
            (
                MethodAssetDistributionHandoffCommandSelector::AdjustDistributionContext,
                MethodAssetDistributionHandoffCommandSource::AdjustDistributionContext {
                    relation_ref,
                    distribution_ref,
                    previous_context_ref,
                    new_context_ref,
                    adjustment_reason_ref,
                    candidate_reason_ref,
                    expected_distribution_version,
                },
            ) => self.adjust_distribution(
                AdjustMethodAssetDistributionContextInput {
                    operation_context_ref: envelope.operation_context_ref.clone(),
                    idempotency_key_ref: envelope.idempotency_key_ref.clone(),
                    operation_digest_ref: envelope.operation_digest_ref.clone(),
                    dedup_scope_ref: envelope.dedup_scope_ref.clone(),
                    relation_ref,
                    distribution_ref,
                    previous_context_ref,
                    new_context_ref,
                    adjustment_reason_ref,
                    candidate_reason_ref,
                    expected_distribution_version,
                    seam_source,
                },
                &envelope,
                uow.as_mut(),
            ),
            (
                MethodAssetDistributionHandoffCommandSelector::MarkDistributionAvailability,
                MethodAssetDistributionHandoffCommandSource::MarkDistributionAvailability {
                    relation_ref,
                    distribution_ref,
                    distribution_context_ref,
                    availability_marker,
                    candidate_reason_ref,
                },
            ) => self.mark_distribution_availability(
                MarkMethodAssetDistributionAvailabilityInput {
                    operation_context_ref: envelope.operation_context_ref.clone(),
                    idempotency_key_ref: envelope.idempotency_key_ref.clone(),
                    operation_digest_ref: envelope.operation_digest_ref.clone(),
                    dedup_scope_ref: envelope.dedup_scope_ref.clone(),
                    relation_ref,
                    distribution_ref,
                    distribution_context_ref,
                    availability_marker,
                    candidate_reason_ref,
                    seam_source,
                },
                &envelope,
                uow.as_mut(),
            ),
            _ => ServiceExecution::Ephemeral(
                self.ephemeral_rejection(&envelope, MethodAssetStoredOperationResultKind::Rejected),
            ),
        };

        match execution {
            ServiceExecution::Persisted {
                stored_result,
                distribution_ref,
                candidate,
            } => {
                let candidate_ref = candidate.as_ref().map(|value| value.0.clone());
                match uow.commit() {
                    Ok(MethodAssetCommitObservation::Committed) => {
                        if stored_result.result_kind
                            == MethodAssetStoredOperationResultKind::Accepted
                        {
                            if let Some((candidate_ref, seam_source)) = candidate {
                                self.process_post_commit_seam(candidate_ref, seam_source);
                            }
                        }
                        stored_result
                    }
                    Ok(MethodAssetCommitObservation::CommitUnknown { .. })
                        if self.commit_unknown_is_durable(
                            &stored_result,
                            distribution_ref,
                            candidate_ref,
                        ) =>
                    {
                        if stored_result.result_kind
                            == MethodAssetStoredOperationResultKind::Accepted
                        {
                            if let Some((candidate_ref, seam_source)) = candidate {
                                self.process_post_commit_seam(candidate_ref, seam_source);
                            }
                        }
                        stored_result
                    }
                    _ => self.ephemeral_rejection(
                        &envelope,
                        MethodAssetStoredOperationResultKind::Conflict,
                    ),
                }
            }
            ServiceExecution::Ephemeral(result) => {
                let _ = uow.rollback();
                result
            }
        }
    }

    fn save_publication_outcome(&self, outcome: MethodAssetPublicationOutcome) -> bool {
        let publication_ref = outcome.publication_ref().clone();
        let mut uow = self.unit_of_work.begin_command_uow();
        let saved_ref = match self
            .publication_repository
            .save_publication_outcome(outcome, uow.as_mut())
        {
            Ok(saved_ref) => saved_ref,
            Err(_) => {
                let _ = uow.rollback();
                return false;
            }
        };
        if saved_ref.value_ref != publication_ref {
            let _ = uow.rollback();
            return false;
        }
        match uow.commit() {
            Ok(MethodAssetCommitObservation::Committed) => true,
            Ok(MethodAssetCommitObservation::CommitUnknown { .. }) => self
                .publication_repository
                .get_publication_outcome(publication_ref.clone())
                .ok()
                .flatten()
                .is_some_and(|value| value.value.publication_ref() == &publication_ref),
            Err(_) => {
                let _ = uow.rollback();
                false
            }
        }
    }

    fn save_handoff_outcome(&self, outcome: MethodAssetHandoffOutcome) -> bool {
        let handoff_ref = outcome.handoff_ref().clone();
        let mut uow = self.unit_of_work.begin_command_uow();
        let saved_ref = match self
            .handoff_repository
            .save_handoff_marker(outcome, uow.as_mut())
        {
            Ok(saved_ref) => saved_ref,
            Err(_) => {
                let _ = uow.rollback();
                return false;
            }
        };
        if saved_ref.value_ref != handoff_ref {
            let _ = uow.rollback();
            return false;
        }
        match uow.commit() {
            Ok(MethodAssetCommitObservation::Committed) => true,
            Ok(MethodAssetCommitObservation::CommitUnknown { .. }) => self
                .handoff_repository
                .get_handoff_marker(handoff_ref.clone())
                .ok()
                .flatten()
                .is_some_and(|value| value.value.handoff_ref() == &handoff_ref),
            Err(_) => {
                let _ = uow.rollback();
                false
            }
        }
    }

    fn process_post_commit_seam(
        &self,
        candidate_ref: MethodAssetEventCandidateAssemblyRef,
        seam_source: MethodAssetDistributionHandoffSeamSource,
    ) {
        if !seam_source.is_valid() {
            return;
        }
        let Some(candidate) = self
            .candidate_repository
            .get_event_candidate_assembly(candidate_ref.clone())
            .ok()
            .flatten()
            .map(|value| value.value)
        else {
            return;
        };
        if candidate.assembly_ref.as_typed_ref().kind()
            != MethodAssetEventCandidateAssemblyRef::expected_kind()
            || candidate.distribution_ref.as_typed_ref().kind()
                != MethodAssetDistributionRef::expected_kind()
            || candidate.distribution_context_ref.as_typed_ref().kind()
                != DistributionContextRef::expected_kind()
        {
            return;
        }
        if candidate
            .availability_marker
            .as_ref()
            .is_some_and(|marker| {
                let decision_required = !matches!(
                    marker.target_state,
                    MethodAssetConsumptionAvailabilityTarget::Ready
                );
                decision_required
                    && self
                        .degraded_mapper
                        .map_degraded_decision(marker.clone(), None)
                        .is_none()
            })
        {
            return;
        }
        let adapter_summary = self
            .adapter_availability
            .check_required_distribution_slots(seam_source.required_slot_refs.clone());
        let target_summary = self.target_registry.resolve_distribution_targets(
            seam_source.target_registry_scope_ref,
            seam_source.publisher_binding_ref,
            seam_source.handoff_binding_ref.clone(),
        );
        if !target_summary.target_ref_set().refs.iter().all(|value| {
            value.as_typed_ref().kind() == MethodAssetHandoffTargetRef::expected_kind()
        }) {
            return;
        }
        let target_ref_set = target_summary.target_ref_set().clone();
        let publication_ref = self.with_factory(|factory| {
            factory.new_publication_outcome_ref(candidate_ref.clone(), target_ref_set.clone())
        });
        let precheck_outcome = match &adapter_summary {
            MethodAssetAdapterAvailabilitySummary::Degraded {
                marker_ref,
                diagnostic_ref,
                ..
            } => Some(MethodAssetPublicationOutcome::Blocked {
                publication_ref: publication_ref.clone(),
                candidate_ref: candidate_ref.clone(),
                target_ref_set: target_ref_set.clone(),
                boundary_marker_ref: seam_source.publication_boundary_marker_ref.clone(),
                reason_ref: marker_ref.clone(),
                diagnostic_ref: diagnostic_ref.clone(),
            }),
            MethodAssetAdapterAvailabilitySummary::Unavailable {
                marker_ref,
                diagnostic_ref,
                ..
            } => Some(MethodAssetPublicationOutcome::Unavailable {
                publication_ref: publication_ref.clone(),
                candidate_ref: candidate_ref.clone(),
                target_ref_set: target_ref_set.clone(),
                boundary_marker_ref: seam_source.publication_boundary_marker_ref.clone(),
                reason_ref: marker_ref.clone(),
                diagnostic_ref: diagnostic_ref.clone(),
            }),
            MethodAssetAdapterAvailabilitySummary::Disabled {
                reason_ref,
                diagnostic_ref,
                ..
            } => Some(MethodAssetPublicationOutcome::Blocked {
                publication_ref: publication_ref.clone(),
                candidate_ref: candidate_ref.clone(),
                target_ref_set: target_ref_set.clone(),
                boundary_marker_ref: seam_source.publication_boundary_marker_ref.clone(),
                reason_ref: reason_ref.clone(),
                diagnostic_ref: diagnostic_ref.clone(),
            }),
            MethodAssetAdapterAvailabilitySummary::Available { .. } => match &target_summary {
                MethodAssetCollaborationTargetSummary::Disabled {
                    reason_ref,
                    diagnostic_ref,
                    ..
                }
                | MethodAssetCollaborationTargetSummary::Blocked {
                    reason_ref,
                    diagnostic_ref,
                    ..
                } => Some(MethodAssetPublicationOutcome::Blocked {
                    publication_ref: publication_ref.clone(),
                    candidate_ref: candidate_ref.clone(),
                    target_ref_set: target_ref_set.clone(),
                    boundary_marker_ref: seam_source.publication_boundary_marker_ref.clone(),
                    reason_ref: reason_ref.clone(),
                    diagnostic_ref: diagnostic_ref.clone(),
                }),
                MethodAssetCollaborationTargetSummary::Unavailable {
                    reason_ref,
                    diagnostic_ref,
                    ..
                } => Some(MethodAssetPublicationOutcome::Unavailable {
                    publication_ref: publication_ref.clone(),
                    candidate_ref: candidate_ref.clone(),
                    target_ref_set: target_ref_set.clone(),
                    boundary_marker_ref: seam_source.publication_boundary_marker_ref.clone(),
                    reason_ref: reason_ref.clone(),
                    diagnostic_ref: diagnostic_ref.clone(),
                }),
                MethodAssetCollaborationTargetSummary::Enabled { .. } => None,
            },
        };
        if let Some(outcome) = precheck_outcome {
            let _ = self.save_publication_outcome(outcome);
            return;
        }

        let publication_outcome =
            self.publisher
                .publish_event_candidate(MethodAssetEventCandidatePublicationInput {
                    publication_ref,
                    candidate: candidate.clone(),
                    target_summary: target_summary.clone(),
                    boundary_marker_ref: seam_source.publication_boundary_marker_ref,
                });
        let publication_accepted = matches!(
            publication_outcome,
            MethodAssetPublicationOutcome::Published { .. }
        );
        let publication_durable = self.save_publication_outcome(publication_outcome);
        if !publication_accepted || !publication_durable {
            return;
        }

        let (Some(_), Some(handoff_boundary_marker_ref)) = (
            seam_source.handoff_binding_ref,
            seam_source.handoff_boundary_marker_ref,
        ) else {
            return;
        };
        let MethodAssetCollaborationTargetSummary::Enabled { target_ref_set, .. } = target_summary
        else {
            return;
        };
        for target_ref in target_ref_set.refs {
            let handoff_ref = self.with_factory(|factory| {
                factory.new_handoff_marker_ref(candidate_ref.clone(), target_ref.clone())
            });
            let outcome =
                self.handoff
                    .prepare_distribution_handoff(MethodAssetDistributionHandoffInput {
                        handoff_ref,
                        distribution_ref: candidate.distribution_ref.clone(),
                        distribution_context_ref: candidate.distribution_context_ref.clone(),
                        candidate_ref: candidate_ref.clone(),
                        target_ref,
                        boundary_marker_ref: handoff_boundary_marker_ref.clone(),
                        diagnostic_ref: None,
                        follow_up_hint_ref: None,
                    });
            if !self.save_handoff_outcome(outcome) {
                break;
            }
        }
    }
}

impl MethodAssetDistributionHandoffCommandFacade
    for DefaultMethodAssetDistributionHandoffCommandFacade
{
    fn dispatch_distribution_handoff_command(
        &self,
        input: MethodAssetDistributionHandoffCommandDispatchInput,
    ) -> MethodAssetDistributionHandoffCommandDispatchOutput {
        let selector = match self.selector_from_shell(&input.command_shell) {
            Ok(value) => value,
            Err(output) => return output,
        };
        if !source_is_valid(&input.command_source)
            || input
                .seam_source
                .as_ref()
                .is_some_and(|value| !value.is_valid())
        {
            return self.early_rejection(self.new_safe_reject_reason_ref());
        }
        let envelope = match self.build_replay_envelope(&input, selector) {
            Ok(value) => value,
            Err(output) => return output,
        };
        match self.duplicate_or_conflict(&envelope) {
            Ok(Some(result)) => return result.into(),
            Ok(None) => {}
            Err(output) => return output,
        }
        self.execute_fresh(selector, input.command_source, input.seam_source, envelope)
            .into()
    }
}
