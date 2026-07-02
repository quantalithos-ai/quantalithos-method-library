//! Formalization/version accepted-service vertical slice for `commit-04-b`.

use std::sync::{Arc, Mutex};

use method_library_contracts::{
    FormalMethodAssetVersionRef, FormalVersionBoundarySummary, FormalizationBasisSummaryRefSet,
    FormalizationEligibilityRuleRef, FormalizationStateKind, FormalizationStateRef,
    GovernanceBasisRef, MethodAssetAcceptedOperationSummaryRef, MethodAssetApiEntryContextRef,
    MethodAssetApplicationDispatchRef, MethodAssetCatalogEntryRef, MethodAssetDedupScopeRef,
    MethodAssetDefinitionRef, MethodAssetEffectSummaryRef, MethodAssetIdempotencyKeyRef,
    MethodAssetOperationContextRef, MethodAssetOperationDigestRef, MethodAssetReplayMarkerRef,
    MethodAssetSafeIgnoreReasonRef, MethodAssetSafeRejectReasonRef,
    MethodAssetStoredOperationResultRef, MethodLibraryCapabilityKind, MethodLibraryCommandShell,
    MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRefKind,
};
use method_library_domain::{FormalMethodAssetVersion, FormalizationState};

use crate::definition_catalog::{
    MethodAssetEffectSummaryRefSet, MethodAssetExpectedVersion,
    MethodAssetReplayEnvelopeBuildError, MethodAssetRepositoryError,
    MethodAssetStoredOperationResult, MethodAssetStoredOperationResultKind, Versioned,
};
use crate::ports::MethodAssetStoredOperationResultRepository;
use crate::ports::{
    FormalMethodAssetVersionRepository, FormalizationBasisResolutionInput,
    FormalizationBasisResolverPort, FormalizationBasisSummaryRepository,
    FormalizationEligibilityDiagnostic, FormalizationStateRepository,
    MethodAssetCatalogEntryRepository, MethodAssetDefinitionRepository,
    MethodAssetPolicyDiagnosticBuilderPort,
};
use crate::unit_of_work::{CommandUnitOfWork, MethodAssetCommitObservation, UnitOfWork};

/// Application-owned facade input for the accepted service slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetFormalizationVersionCommandDispatchInput {
    /// Shared public command shell.
    pub command_shell: MethodLibraryCommandShell,
    /// Body-free structured command source.
    pub command_source: MethodAssetFormalizationVersionCommandSource,
    /// API entry context minted by the support factory.
    pub api_entry_context_ref: MethodAssetApiEntryContextRef,
    /// Opaque dispatch target marker.
    pub application_dispatch_ref: MethodAssetApplicationDispatchRef,
}

/// Application-owned facade output copied from the stored-result safe surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetFormalizationVersionCommandDispatchOutput {
    /// Stable stored-result anchor.
    pub stored_result_ref: MethodAssetStoredOperationResultRef,
    /// Replay-safe result kind.
    pub result_kind: MethodAssetStoredOperationResultKind,
    /// Replay marker anchor.
    pub replay_marker_ref: MethodAssetReplayMarkerRef,
    /// Accepted-summary anchor when the command succeeded.
    pub accepted_summary_ref: Option<MethodAssetAcceptedOperationSummaryRef>,
    /// Safe rejection reason when the command was rejected or conflicted.
    pub rejected_reason_ref: Option<MethodAssetSafeRejectReasonRef>,
    /// Safe ignore reason for no-op results.
    pub ignored_reason_ref: Option<MethodAssetSafeIgnoreReasonRef>,
    /// Body-free effect refs.
    pub effect_summary_refs: MethodAssetEffectSummaryRefSet,
}

impl From<MethodAssetStoredOperationResult>
    for MethodAssetFormalizationVersionCommandDispatchOutput
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

/// The only current-boundary selector family derived from `command_shell.boundary_ref.kind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodAssetFormalizationVersionCommandSelector {
    /// Evaluate formalization eligibility.
    EvaluateFormalizationEligibility,
    /// Initiate formalization.
    InitiateFormalization,
    /// Establish a formal method version.
    EstablishFormalVersion,
    /// Record a semantic change on an active formal method version.
    RecordFormalVersionSemanticChange,
    /// Supersede a previous formal method version.
    SupersedeFormalVersion,
    /// Retire a formal method version.
    RetireFormalVersion,
}

/// Application-owned body-free source carrier for the current boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetFormalizationVersionCommandSource {
    /// Structured evaluate payload.
    EvaluateFormalizationEligibility(EvaluateFormalizationEligibilityCommandSource),
    /// Structured initiate payload.
    InitiateFormalization(InitiateMethodAssetFormalizationCommandSource),
    /// Structured establish payload.
    EstablishFormalVersion(EstablishFormalMethodAssetVersionCommandSource),
    /// Structured semantic-change payload.
    RecordFormalVersionSemanticChange(RecordFormalVersionSemanticChangeCommandSource),
    /// Structured supersede payload.
    SupersedeFormalVersion(SupersedeFormalMethodAssetVersionCommandSource),
    /// Structured retire payload.
    RetireFormalVersion(RetireFormalMethodAssetVersionCommandSource),
}

/// Source fields for `EvaluateFormalizationEligibility`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluateFormalizationEligibilityCommandSource {
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Linked catalog-entry anchor.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Basis-summary refs to evaluate.
    pub basis_summary_refs: FormalizationBasisSummaryRefSet,
    /// Eligibility rule anchor.
    pub eligibility_rule_ref: FormalizationEligibilityRuleRef,
}

/// Source fields for `InitiateFormalization`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitiateMethodAssetFormalizationCommandSource {
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Linked catalog-entry anchor.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Explicit safe trigger marker.
    pub trigger_marker_ref: MethodLibrarySafeMarker,
    /// Basis-summary refs to carry into the pending state.
    pub basis_summary_refs: FormalizationBasisSummaryRefSet,
}

/// Source fields for `EstablishFormalVersion`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EstablishFormalMethodAssetVersionCommandSource {
    /// Eligible formalization-state anchor.
    pub formalization_state_ref: FormalizationStateRef,
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Linked catalog-entry anchor.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Body-free formal version boundary summary.
    pub version_boundary_summary: FormalVersionBoundarySummary,
}

/// Source fields for `RecordFormalVersionSemanticChange`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordFormalVersionSemanticChangeCommandSource {
    /// Existing formal method-version anchor.
    pub formal_version_ref: FormalMethodAssetVersionRef,
    /// Explicit safe semantic-change marker.
    pub semantic_change_marker_ref: MethodLibrarySafeMarker,
    /// Basis-summary refs that justify the change.
    pub basis_summary_refs: FormalizationBasisSummaryRefSet,
    /// Optional governance basis anchor.
    pub governance_basis_ref: Option<GovernanceBasisRef>,
}

/// Source fields for `SupersedeFormalVersion`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupersedeFormalMethodAssetVersionCommandSource {
    /// Previous formal method-version anchor.
    pub previous_formal_version_ref: FormalMethodAssetVersionRef,
    /// Next formal method-version anchor.
    pub next_formal_version_ref: FormalMethodAssetVersionRef,
    /// Explicit safe supersession marker.
    pub supersession_marker_ref: MethodLibrarySafeMarker,
}

/// Source fields for `RetireFormalVersion`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetireFormalMethodAssetVersionCommandSource {
    /// Existing formal method-version anchor.
    pub formal_version_ref: FormalMethodAssetVersionRef,
    /// Explicit safe retirement marker.
    pub retirement_marker_ref: MethodLibrarySafeMarker,
}

/// Service-input enum for current-boundary dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetFormalizationVersionServiceInput {
    /// Evaluate formalization eligibility service input.
    EvaluateFormalizationEligibility(EvaluateMethodAssetFormalizationEligibilityInput),
    /// Initiate formalization service input.
    InitiateFormalization(InitiateMethodAssetFormalizationInput),
    /// Establish formal version service input.
    EstablishFormalVersion(EstablishFormalMethodAssetVersionInput),
    /// Record semantic change service input.
    RecordFormalVersionSemanticChange(RecordFormalVersionSemanticChangeInput),
    /// Supersede formal version service input.
    SupersedeFormalVersion(SupersedeFormalMethodAssetVersionInput),
    /// Retire formal version service input.
    RetireFormalVersion(RetireFormalMethodAssetVersionInput),
}

/// Shared replay-envelope fields plus evaluate payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluateMethodAssetFormalizationEligibilityInput {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency-key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Linked catalog-entry anchor.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Existing formalization-state anchor when already present.
    pub current_formalization_state_ref: Option<FormalizationStateRef>,
    /// Existing state optimistic version when already present.
    pub expected_state_version: Option<MethodAssetExpectedVersion>,
    /// Basis-summary refs to evaluate.
    pub basis_summary_refs: FormalizationBasisSummaryRefSet,
    /// Eligibility rule anchor.
    pub eligibility_rule_ref: FormalizationEligibilityRuleRef,
}

/// Shared replay-envelope fields plus initiate payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitiateMethodAssetFormalizationInput {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency-key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Linked catalog-entry anchor.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Existing formalization-state anchor when already present.
    pub current_formalization_state_ref: Option<FormalizationStateRef>,
    /// Existing state optimistic version when already present.
    pub expected_state_version: Option<MethodAssetExpectedVersion>,
    /// Explicit safe trigger marker.
    pub trigger_marker_ref: MethodLibrarySafeMarker,
    /// Basis-summary refs to carry forward.
    pub basis_summary_refs: FormalizationBasisSummaryRefSet,
}

/// Shared replay-envelope fields plus establish payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EstablishFormalMethodAssetVersionInput {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency-key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    /// Eligible formalization-state anchor.
    pub formalization_state_ref: FormalizationStateRef,
    /// Loaded optimistic version for the state owner.
    pub expected_state_version: MethodAssetExpectedVersion,
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Linked catalog-entry anchor.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Body-free formal version boundary summary.
    pub version_boundary_summary: FormalVersionBoundarySummary,
}

/// Shared replay-envelope fields plus semantic-change payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordFormalVersionSemanticChangeInput {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency-key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    /// Existing formal method-version anchor.
    pub formal_version_ref: FormalMethodAssetVersionRef,
    /// Loaded optimistic version for the formal version.
    pub expected_version: MethodAssetExpectedVersion,
    /// Explicit safe semantic-change marker.
    pub semantic_change_marker_ref: MethodLibrarySafeMarker,
    /// Basis-summary refs that justify the change.
    pub basis_summary_refs: FormalizationBasisSummaryRefSet,
    /// Optional governance basis anchor.
    pub governance_basis_ref: Option<GovernanceBasisRef>,
}

/// Shared replay-envelope fields plus supersede payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupersedeFormalMethodAssetVersionInput {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency-key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    /// Previous formal method-version anchor.
    pub previous_formal_version_ref: FormalMethodAssetVersionRef,
    /// Loaded optimistic version for the previous version.
    pub previous_expected_version: MethodAssetExpectedVersion,
    /// Next formal method-version anchor.
    pub next_formal_version_ref: FormalMethodAssetVersionRef,
    /// Loaded optimistic version for the next version.
    pub next_expected_version: MethodAssetExpectedVersion,
    /// Explicit safe supersession marker.
    pub supersession_marker_ref: MethodLibrarySafeMarker,
}

/// Shared replay-envelope fields plus retire payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetireFormalMethodAssetVersionInput {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency-key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    /// Existing formal method-version anchor.
    pub formal_version_ref: FormalMethodAssetVersionRef,
    /// Loaded optimistic version for the formal version.
    pub expected_version: MethodAssetExpectedVersion,
    /// Explicit safe retirement marker.
    pub retirement_marker_ref: MethodLibrarySafeMarker,
}

/// Input to the replay-envelope helper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetFormalizationVersionReplayEnvelopeFactoryInput {
    /// Shared public command shell.
    pub command_shell: MethodLibraryCommandShell,
    /// Body-free command source.
    pub command_source: MethodAssetFormalizationVersionCommandSource,
    /// Selector derived from the shell boundary intent.
    pub selector: MethodAssetFormalizationVersionCommandSelector,
    /// API entry context ref.
    pub api_entry_context_ref: MethodAssetApiEntryContextRef,
    /// Application dispatch marker.
    pub application_dispatch_ref: MethodAssetApplicationDispatchRef,
}

/// Shared replay-envelope fields copied into every service input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetFormalizationVersionReplayEnvelope {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency-key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
}

/// Exact helper surface for replay refs and truth refs.
pub trait MethodAssetFormalizationVersionSupportRefFactory: Send {
    /// Returns the only current-boundary dispatch marker.
    fn formalization_version_dispatch_ref(&self) -> MethodAssetApplicationDispatchRef;

    /// Mints a new API entry context ref.
    fn new_api_entry_context_ref(&mut self) -> MethodAssetApiEntryContextRef;

    /// Builds the shared replay envelope.
    fn build_formalization_version_replay_envelope(
        &mut self,
        input: MethodAssetFormalizationVersionReplayEnvelopeFactoryInput,
    ) -> Result<MethodAssetFormalizationVersionReplayEnvelope, MethodAssetReplayEnvelopeBuildError>;

    /// Mints a stored-result ref.
    fn new_stored_operation_result_ref(&mut self) -> MethodAssetStoredOperationResultRef;

    /// Mints an accepted-summary ref.
    fn new_accepted_operation_summary_ref(&mut self) -> MethodAssetAcceptedOperationSummaryRef;

    /// Mints a safe rejection reason ref.
    fn new_safe_reject_reason_ref(&mut self) -> MethodAssetSafeRejectReasonRef;

    /// Mints a safe ignore reason ref.
    fn new_safe_ignore_reason_ref(&mut self) -> MethodAssetSafeIgnoreReasonRef;

    /// Mints an effect-summary ref.
    fn new_effect_summary_ref(&mut self) -> MethodAssetEffectSummaryRef;

    /// Mints a replay-marker ref.
    fn new_replay_marker_ref(&mut self) -> MethodAssetReplayMarkerRef;

    /// Mints a new formalization-state ref for evaluate/initiate create paths.
    fn new_formalization_state_ref(
        &mut self,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> FormalizationStateRef;

    /// Mints a new formal method-version ref for establish.
    fn new_formal_method_asset_version_ref(
        &mut self,
        formalization_state_ref: FormalizationStateRef,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
        version_boundary_summary: FormalVersionBoundarySummary,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> FormalMethodAssetVersionRef;
}

/// Facade surface called by the minimal API entry.
pub trait MethodAssetFormalizationVersionCommandFacade: Send + Sync {
    /// Dispatches a body-free command shell into the accepted service slice.
    fn dispatch_formalization_version_command(
        &self,
        input: MethodAssetFormalizationVersionCommandDispatchInput,
    ) -> MethodAssetFormalizationVersionCommandDispatchOutput;
}

enum CommitUnknownReadBack {
    CurrentState {
        state_ref: Option<FormalizationStateRef>,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
    },
    ExactStateAndVersion {
        state_ref: FormalizationStateRef,
        version_ref: FormalMethodAssetVersionRef,
    },
    ExactVersions {
        version_refs: Vec<FormalMethodAssetVersionRef>,
    },
}

enum ServiceExecution {
    Persisted {
        stored_result: MethodAssetStoredOperationResult,
        read_back: CommitUnknownReadBack,
    },
    Ephemeral(MethodAssetStoredOperationResult),
}

/// Default current-boundary facade implementation.
pub struct DefaultMethodAssetFormalizationVersionCommandFacade {
    definition_repository: Arc<dyn MethodAssetDefinitionRepository>,
    catalog_repository: Arc<dyn MethodAssetCatalogEntryRepository>,
    formalization_state_repository: Arc<dyn FormalizationStateRepository>,
    formal_method_asset_version_repository: Arc<dyn FormalMethodAssetVersionRepository>,
    formalization_basis_summary_repository: Arc<dyn FormalizationBasisSummaryRepository>,
    formalization_basis_resolver: Arc<dyn FormalizationBasisResolverPort>,
    policy_diagnostic_builder: Arc<dyn MethodAssetPolicyDiagnosticBuilderPort>,
    stored_result_repository: Arc<dyn MethodAssetStoredOperationResultRepository>,
    unit_of_work: Arc<dyn UnitOfWork>,
    support_ref_factory: Arc<Mutex<Box<dyn MethodAssetFormalizationVersionSupportRefFactory>>>,
}

impl DefaultMethodAssetFormalizationVersionCommandFacade {
    /// Creates the current-boundary facade from formal ports and helper surfaces.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        definition_repository: Arc<dyn MethodAssetDefinitionRepository>,
        catalog_repository: Arc<dyn MethodAssetCatalogEntryRepository>,
        formalization_state_repository: Arc<dyn FormalizationStateRepository>,
        formal_method_asset_version_repository: Arc<dyn FormalMethodAssetVersionRepository>,
        formalization_basis_summary_repository: Arc<dyn FormalizationBasisSummaryRepository>,
        formalization_basis_resolver: Arc<dyn FormalizationBasisResolverPort>,
        policy_diagnostic_builder: Arc<dyn MethodAssetPolicyDiagnosticBuilderPort>,
        stored_result_repository: Arc<dyn MethodAssetStoredOperationResultRepository>,
        unit_of_work: Arc<dyn UnitOfWork>,
        support_ref_factory: Arc<Mutex<Box<dyn MethodAssetFormalizationVersionSupportRefFactory>>>,
    ) -> Self {
        Self {
            definition_repository,
            catalog_repository,
            formalization_state_repository,
            formal_method_asset_version_repository,
            formalization_basis_summary_repository,
            formalization_basis_resolver,
            policy_diagnostic_builder,
            stored_result_repository,
            unit_of_work,
            support_ref_factory,
        }
    }

    fn with_support_factory<R>(
        &self,
        action: impl FnOnce(&mut dyn MethodAssetFormalizationVersionSupportRefFactory) -> R,
    ) -> R {
        let mut guard = self
            .support_ref_factory
            .lock()
            .expect("support ref factory lock poisoned");
        action(guard.as_mut())
    }

    fn new_safe_reject_reason_ref(&self) -> MethodAssetSafeRejectReasonRef {
        self.with_support_factory(|factory| factory.new_safe_reject_reason_ref())
    }

    fn new_effect_summary_refs(&self) -> MethodAssetEffectSummaryRefSet {
        MethodAssetEffectSummaryRefSet::from_refs([
            self.with_support_factory(|factory| factory.new_effect_summary_ref())
        ])
    }

    fn new_result_shell(
        &self,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        result_kind: MethodAssetStoredOperationResultKind,
        accepted_summary_ref: Option<MethodAssetAcceptedOperationSummaryRef>,
        rejected_reason_ref: Option<MethodAssetSafeRejectReasonRef>,
        ignored_reason_ref: Option<MethodAssetSafeIgnoreReasonRef>,
        effect_summary_refs: MethodAssetEffectSummaryRefSet,
    ) -> MethodAssetStoredOperationResult {
        let stored_result_ref =
            self.with_support_factory(|factory| factory.new_stored_operation_result_ref());
        let replay_marker_ref =
            self.with_support_factory(|factory| factory.new_replay_marker_ref());

        MethodAssetStoredOperationResult {
            stored_result_ref,
            operation_context_ref,
            operation_digest_ref,
            result_kind,
            accepted_summary_ref,
            rejected_reason_ref,
            ignored_reason_ref,
            effect_summary_refs,
            replay_marker_ref,
        }
    }

    fn early_rejected_output(
        &self,
        reason_ref: MethodAssetSafeRejectReasonRef,
    ) -> MethodAssetFormalizationVersionCommandDispatchOutput {
        let stored_result_ref =
            self.with_support_factory(|factory| factory.new_stored_operation_result_ref());
        let replay_marker_ref =
            self.with_support_factory(|factory| factory.new_replay_marker_ref());

        MethodAssetFormalizationVersionCommandDispatchOutput {
            stored_result_ref,
            result_kind: MethodAssetStoredOperationResultKind::Rejected,
            replay_marker_ref,
            accepted_summary_ref: None,
            rejected_reason_ref: Some(reason_ref),
            ignored_reason_ref: None,
            effect_summary_refs: MethodAssetEffectSummaryRefSet::new(),
        }
    }

    fn new_ephemeral_result_from_envelope(
        &self,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        result_kind: MethodAssetStoredOperationResultKind,
        rejected_reason_ref: Option<MethodAssetSafeRejectReasonRef>,
    ) -> MethodAssetStoredOperationResult {
        self.new_result_shell(
            envelope.operation_context_ref.clone(),
            envelope.operation_digest_ref.clone(),
            result_kind,
            None,
            rejected_reason_ref,
            None,
            MethodAssetEffectSummaryRefSet::new(),
        )
    }

    fn persisted_result(
        &self,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        stored_result: MethodAssetStoredOperationResult,
        read_back: CommitUnknownReadBack,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
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
                read_back,
            },
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }

    fn persisted_rejected_result(
        &self,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        read_back: CommitUnknownReadBack,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let reason_ref = self.new_safe_reject_reason_ref();
        let stored_result = self.new_result_shell(
            envelope.operation_context_ref.clone(),
            envelope.operation_digest_ref.clone(),
            MethodAssetStoredOperationResultKind::Rejected,
            None,
            Some(reason_ref),
            None,
            MethodAssetEffectSummaryRefSet::new(),
        );
        self.persisted_result(envelope, stored_result, read_back, uow)
    }

    fn persisted_accepted_result(
        &self,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        read_back: CommitUnknownReadBack,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let accepted_summary_ref =
            self.with_support_factory(|factory| factory.new_accepted_operation_summary_ref());
        let stored_result = self.new_result_shell(
            envelope.operation_context_ref.clone(),
            envelope.operation_digest_ref.clone(),
            MethodAssetStoredOperationResultKind::Accepted,
            Some(accepted_summary_ref),
            None,
            None,
            self.new_effect_summary_refs(),
        );
        self.persisted_result(envelope, stored_result, read_back, uow)
    }

    fn ephemeral_result_from_repository_error(
        &self,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        error: MethodAssetRepositoryError,
    ) -> MethodAssetStoredOperationResult {
        match error {
            MethodAssetRepositoryError::StoredResultIntegrityViolation { .. } => {
                let reason_ref = self.new_safe_reject_reason_ref();
                self.new_ephemeral_result_from_envelope(
                    envelope,
                    MethodAssetStoredOperationResultKind::Conflict,
                    Some(reason_ref),
                )
            }
            _ => {
                let reason_ref = self.new_safe_reject_reason_ref();
                self.new_ephemeral_result_from_envelope(
                    envelope,
                    MethodAssetStoredOperationResultKind::Rejected,
                    Some(reason_ref),
                )
            }
        }
    }

    fn selector_from_shell(
        &self,
        command_shell: &MethodLibraryCommandShell,
    ) -> Result<
        MethodAssetFormalizationVersionCommandSelector,
        MethodAssetFormalizationVersionCommandDispatchOutput,
    > {
        if command_shell.capability_kind != MethodLibraryCapabilityKind::FormalizationVersion {
            return Err(self.early_rejected_output(self.new_safe_reject_reason_ref()));
        }

        match command_shell.boundary_ref.kind() {
            MethodLibraryTypedBoundaryRefKind::MethodAssetFormalizationEligibilityEvaluateIntent => {
                Ok(MethodAssetFormalizationVersionCommandSelector::EvaluateFormalizationEligibility)
            }
            MethodLibraryTypedBoundaryRefKind::MethodAssetFormalizationInitiateIntent => {
                Ok(MethodAssetFormalizationVersionCommandSelector::InitiateFormalization)
            }
            MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersionEstablishIntent => {
                Ok(MethodAssetFormalizationVersionCommandSelector::EstablishFormalVersion)
            }
            MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersionSemanticChangeRecordIntent => {
                Ok(
                    MethodAssetFormalizationVersionCommandSelector::RecordFormalVersionSemanticChange,
                )
            }
            MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersionSupersedeIntent => {
                Ok(MethodAssetFormalizationVersionCommandSelector::SupersedeFormalVersion)
            }
            MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersionRetireIntent => {
                Ok(MethodAssetFormalizationVersionCommandSelector::RetireFormalVersion)
            }
            _ => Err(self.early_rejected_output(self.new_safe_reject_reason_ref())),
        }
    }

    fn build_replay_envelope(
        &self,
        input: &MethodAssetFormalizationVersionCommandDispatchInput,
        selector: MethodAssetFormalizationVersionCommandSelector,
    ) -> Result<
        MethodAssetFormalizationVersionReplayEnvelope,
        MethodAssetFormalizationVersionCommandDispatchOutput,
    > {
        self.with_support_factory(|factory| {
            factory.build_formalization_version_replay_envelope(
                MethodAssetFormalizationVersionReplayEnvelopeFactoryInput {
                    command_shell: input.command_shell.clone(),
                    command_source: input.command_source.clone(),
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
                self.early_rejected_output(reason_ref)
            }
        })
    }

    fn duplicate_or_conflict(
        &self,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
    ) -> Result<
        Option<MethodAssetStoredOperationResult>,
        MethodAssetFormalizationVersionCommandDispatchOutput,
    > {
        match self
            .stored_result_repository
            .find_command_result_by_idempotency(
                envelope.idempotency_key_ref.clone(),
                envelope.dedup_scope_ref.clone(),
            ) {
            Ok(Some(stored_result)) => {
                if stored_result.operation_digest_ref == envelope.operation_digest_ref {
                    Ok(Some(stored_result))
                } else {
                    Err(self
                        .new_ephemeral_result_from_envelope(
                            envelope,
                            MethodAssetStoredOperationResultKind::Conflict,
                            Some(self.new_safe_reject_reason_ref()),
                        )
                        .into())
                }
            }
            Ok(None) => Ok(None),
            Err(error) => Err(self
                .ephemeral_result_from_repository_error(envelope, error)
                .into()),
        }
    }

    fn exact_state_read_back_ok(&self, state_ref: FormalizationStateRef) -> bool {
        matches!(
            self.formalization_state_repository
                .get_formalization_state_with_version(state_ref),
            Ok(Some(_))
        )
    }

    fn exact_version_read_back_ok(&self, version_ref: FormalMethodAssetVersionRef) -> bool {
        matches!(
            self.formal_method_asset_version_repository
                .get_formal_method_asset_version_with_version(version_ref),
            Ok(Some(_))
        )
    }

    fn commit_unknown_result(
        &self,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        stored_result: MethodAssetStoredOperationResult,
        read_back: CommitUnknownReadBack,
    ) -> MethodAssetStoredOperationResult {
        let replayed = self
            .stored_result_repository
            .get_stored_operation_result(stored_result.stored_result_ref.clone())
            .ok()
            .flatten();

        let truth_ok = match read_back {
            CommitUnknownReadBack::CurrentState {
                state_ref,
                definition_ref,
                catalog_entry_ref,
            } => match state_ref {
                Some(state_ref) => self.exact_state_read_back_ok(state_ref),
                None => matches!(
                    self.formalization_state_repository
                        .find_formalization_state_by_definition_catalog(
                            definition_ref,
                            catalog_entry_ref,
                        ),
                    Ok(Some(_))
                ),
            },
            CommitUnknownReadBack::ExactStateAndVersion {
                state_ref,
                version_ref,
            } => {
                self.exact_state_read_back_ok(state_ref)
                    && self.exact_version_read_back_ok(version_ref)
            }
            CommitUnknownReadBack::ExactVersions { version_refs } => version_refs
                .into_iter()
                .all(|version_ref| self.exact_version_read_back_ok(version_ref)),
        };

        match (replayed, truth_ok) {
            (Some(replayed), true) => replayed,
            _ => self.new_ephemeral_result_from_envelope(
                envelope,
                MethodAssetStoredOperationResultKind::Conflict,
                Some(self.new_safe_reject_reason_ref()),
            ),
        }
    }

    fn finalize_execution(
        &self,
        execution: ServiceExecution,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> MethodAssetStoredOperationResult {
        match execution {
            ServiceExecution::Persisted {
                stored_result,
                read_back,
            } => match uow.commit() {
                Ok(MethodAssetCommitObservation::Committed) => stored_result,
                Ok(MethodAssetCommitObservation::CommitUnknown { .. }) => {
                    self.commit_unknown_result(envelope, stored_result, read_back)
                }
                Err(()) => self.new_ephemeral_result_from_envelope(
                    envelope,
                    MethodAssetStoredOperationResultKind::Rejected,
                    Some(self.new_safe_reject_reason_ref()),
                ),
            },
            ServiceExecution::Ephemeral(stored_result) => {
                let _ = uow.rollback();
                stored_result
            }
        }
    }

    fn execute_fresh_command(
        &self,
        selector: MethodAssetFormalizationVersionCommandSelector,
        command_source: MethodAssetFormalizationVersionCommandSource,
        envelope: MethodAssetFormalizationVersionReplayEnvelope,
    ) -> MethodAssetStoredOperationResult {
        let mut uow = self.unit_of_work.begin_command_uow();
        let execution = match selector {
            MethodAssetFormalizationVersionCommandSelector::EvaluateFormalizationEligibility => {
                match command_source {
                    MethodAssetFormalizationVersionCommandSource::EvaluateFormalizationEligibility(
                        source,
                    ) => self.evaluate_formalization_eligibility(source, &envelope, uow.as_mut()),
                    _ => ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                        &envelope,
                        MethodAssetStoredOperationResultKind::Rejected,
                        Some(self.new_safe_reject_reason_ref()),
                    )),
                }
            }
            MethodAssetFormalizationVersionCommandSelector::InitiateFormalization => {
                match command_source {
                    MethodAssetFormalizationVersionCommandSource::InitiateFormalization(source) => {
                        self.initiate_formalization(source, &envelope, uow.as_mut())
                    }
                    _ => ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                        &envelope,
                        MethodAssetStoredOperationResultKind::Rejected,
                        Some(self.new_safe_reject_reason_ref()),
                    )),
                }
            }
            MethodAssetFormalizationVersionCommandSelector::EstablishFormalVersion => {
                match command_source {
                    MethodAssetFormalizationVersionCommandSource::EstablishFormalVersion(source) => {
                        self.establish_formal_version(source, &envelope, uow.as_mut())
                    }
                    _ => ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                        &envelope,
                        MethodAssetStoredOperationResultKind::Rejected,
                        Some(self.new_safe_reject_reason_ref()),
                    )),
                }
            }
            MethodAssetFormalizationVersionCommandSelector::RecordFormalVersionSemanticChange => {
                match command_source {
                    MethodAssetFormalizationVersionCommandSource::RecordFormalVersionSemanticChange(
                        source,
                    ) => self.record_formal_version_semantic_change(source, &envelope, uow.as_mut()),
                    _ => ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                        &envelope,
                        MethodAssetStoredOperationResultKind::Rejected,
                        Some(self.new_safe_reject_reason_ref()),
                    )),
                }
            }
            MethodAssetFormalizationVersionCommandSelector::SupersedeFormalVersion => {
                match command_source {
                    MethodAssetFormalizationVersionCommandSource::SupersedeFormalVersion(source) => {
                        self.supersede_formal_version(source, &envelope, uow.as_mut())
                    }
                    _ => ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                        &envelope,
                        MethodAssetStoredOperationResultKind::Rejected,
                        Some(self.new_safe_reject_reason_ref()),
                    )),
                }
            }
            MethodAssetFormalizationVersionCommandSelector::RetireFormalVersion => {
                match command_source {
                    MethodAssetFormalizationVersionCommandSource::RetireFormalVersion(source) => {
                        self.retire_formal_version(source, &envelope, uow.as_mut())
                    }
                    _ => ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                        &envelope,
                        MethodAssetStoredOperationResultKind::Rejected,
                        Some(self.new_safe_reject_reason_ref()),
                    )),
                }
            }
        };

        self.finalize_execution(execution, &envelope, uow.as_mut())
    }

    fn load_definition_and_catalog(
        &self,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> Result<
        (
            Versioned<method_library_domain::MethodAssetDefinition>,
            Versioned<method_library_domain::MethodAssetCatalogEntry>,
        ),
        ServiceExecution,
    > {
        let definition = match self
            .definition_repository
            .get_definition_with_version(definition_ref.clone())
        {
            Ok(Some(value)) => value,
            Ok(None) => {
                return Err(self.persisted_rejected_result(
                    envelope,
                    CommitUnknownReadBack::CurrentState {
                        state_ref: None,
                        definition_ref,
                        catalog_entry_ref,
                    },
                    uow,
                ));
            }
            Err(error) => {
                return Err(ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                ));
            }
        };
        let catalog_entry = match self
            .catalog_repository
            .get_catalog_entry_with_version(catalog_entry_ref.clone())
        {
            Ok(Some(value)) => value,
            Ok(None) => {
                return Err(self.persisted_rejected_result(
                    envelope,
                    CommitUnknownReadBack::CurrentState {
                        state_ref: None,
                        definition_ref,
                        catalog_entry_ref,
                    },
                    uow,
                ));
            }
            Err(error) => {
                return Err(ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                ));
            }
        };
        if catalog_entry
            .value
            .assert_for_definition(&definition.value.definition_ref)
            .is_err()
        {
            return Err(self.persisted_rejected_result(
                envelope,
                CommitUnknownReadBack::CurrentState {
                    state_ref: None,
                    definition_ref,
                    catalog_entry_ref,
                },
                uow,
            ));
        }

        Ok((definition, catalog_entry))
    }

    fn ensure_basis_summaries_exist(
        &self,
        basis_summary_refs: &FormalizationBasisSummaryRefSet,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        read_back: CommitUnknownReadBack,
        uow: &mut dyn CommandUnitOfWork,
    ) -> Result<(), ServiceExecution> {
        for basis_summary_ref in &basis_summary_refs.refs {
            match self
                .formalization_basis_summary_repository
                .get_formalization_basis_summary_with_version(basis_summary_ref.clone())
            {
                Ok(Some(_)) => {}
                Ok(None) => return Err(self.persisted_rejected_result(envelope, read_back, uow)),
                Err(error) => {
                    return Err(ServiceExecution::Ephemeral(
                        self.ephemeral_result_from_repository_error(envelope, error),
                    ));
                }
            }
        }

        Ok(())
    }

    fn apply_eligibility_diagnostic(
        &self,
        state: &mut FormalizationState,
        diagnostic: FormalizationEligibilityDiagnostic,
    ) -> Result<(), ()> {
        match diagnostic.target_state_kind {
            FormalizationStateKind::AssessmentPending => state
                .mark_assessment_pending(
                    diagnostic.reason_summary.clone(),
                    diagnostic.reason_summary.basis_summary_refs.clone(),
                )
                .map_err(|_| ()),
            FormalizationStateKind::Eligible => state
                .mark_eligible(diagnostic.reason_summary.basis_summary_refs.clone())
                .map_err(|_| ()),
            FormalizationStateKind::Ineligible => {
                state.block(diagnostic.reason_summary).map_err(|_| ())
            }
            FormalizationStateKind::VersionEstablished => Err(()),
        }
    }

    fn evaluate_formalization_eligibility(
        &self,
        source: EvaluateFormalizationEligibilityCommandSource,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let read_back = CommitUnknownReadBack::CurrentState {
            state_ref: None,
            definition_ref: source.definition_ref.clone(),
            catalog_entry_ref: source.catalog_entry_ref.clone(),
        };
        let (definition, catalog_entry) = match self.load_definition_and_catalog(
            source.definition_ref.clone(),
            source.catalog_entry_ref.clone(),
            envelope,
            uow,
        ) {
            Ok(value) => value,
            Err(execution) => return execution,
        };
        if let Err(execution) = self.ensure_basis_summaries_exist(
            &source.basis_summary_refs,
            envelope,
            CommitUnknownReadBack::CurrentState {
                state_ref: None,
                definition_ref: source.definition_ref.clone(),
                catalog_entry_ref: source.catalog_entry_ref.clone(),
            },
            uow,
        ) {
            return execution;
        }

        let current_state = match self
            .formalization_state_repository
            .find_formalization_state_by_definition_catalog(
                source.definition_ref.clone(),
                source.catalog_entry_ref.clone(),
            ) {
            Ok(value) => value,
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };

        let basis_resolution = match self
            .formalization_basis_resolver
            .resolve_formalization_basis(FormalizationBasisResolutionInput {
                definition_ref: source.definition_ref.clone(),
                catalog_entry_ref: Some(source.catalog_entry_ref.clone()),
                basis_summary_refs: source.basis_summary_refs.clone(),
                governance_basis_ref: None,
            }) {
            Ok(value) => value,
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };

        let diagnostic = match self
            .policy_diagnostic_builder
            .build_formalization_eligibility_diagnostic(
                &definition.value,
                &catalog_entry.value,
                &basis_resolution,
                source.eligibility_rule_ref,
            ) {
            Ok(value) => value,
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };

        let mut state = if let Some(current_state) = &current_state {
            current_state.value.clone()
        } else {
            let formalization_state_ref = self.with_support_factory(|factory| {
                factory.new_formalization_state_ref(
                    source.definition_ref.clone(),
                    source.catalog_entry_ref.clone(),
                    envelope.operation_context_ref.clone(),
                    envelope.operation_digest_ref.clone(),
                    envelope.dedup_scope_ref.clone(),
                )
            });
            FormalizationState::pending_for_definition(
                formalization_state_ref,
                source.definition_ref.clone(),
                source.catalog_entry_ref.clone(),
                diagnostic.reason_summary.clone(),
            )
        };
        if self
            .apply_eligibility_diagnostic(&mut state, diagnostic)
            .is_err()
        {
            return self.persisted_rejected_result(envelope, read_back, uow);
        }

        let read_back = CommitUnknownReadBack::CurrentState {
            state_ref: Some(state.formalization_state_ref.clone()),
            definition_ref: source.definition_ref,
            catalog_entry_ref: source.catalog_entry_ref,
        };

        match self
            .formalization_state_repository
            .save_formalization_state(state, current_state.map(|value| value.version.into()), uow)
        {
            Ok(_) => self.persisted_accepted_result(envelope, read_back, uow),
            Err(MethodAssetRepositoryError::VersionConflict { .. }) => {
                ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                    envelope,
                    MethodAssetStoredOperationResultKind::Rejected,
                    Some(self.new_safe_reject_reason_ref()),
                ))
            }
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }

    fn initiate_formalization(
        &self,
        source: InitiateMethodAssetFormalizationCommandSource,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let (definition, catalog_entry) = match self.load_definition_and_catalog(
            source.definition_ref.clone(),
            source.catalog_entry_ref.clone(),
            envelope,
            uow,
        ) {
            Ok(value) => value,
            Err(execution) => return execution,
        };
        let _ = definition;
        let _ = catalog_entry;
        let current_state = match self
            .formalization_state_repository
            .find_formalization_state_by_definition_catalog(
                source.definition_ref.clone(),
                source.catalog_entry_ref.clone(),
            ) {
            Ok(value) => value,
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };

        if let Some(current_state) = &current_state {
            if current_state.value.state_kind == FormalizationStateKind::VersionEstablished {
                return self.persisted_rejected_result(
                    envelope,
                    CommitUnknownReadBack::CurrentState {
                        state_ref: Some(current_state.value.formalization_state_ref.clone()),
                        definition_ref: source.definition_ref,
                        catalog_entry_ref: source.catalog_entry_ref,
                    },
                    uow,
                );
            }
        }
        if !source.trigger_marker_ref.is_public_safe() {
            return self.persisted_rejected_result(
                envelope,
                CommitUnknownReadBack::CurrentState {
                    state_ref: current_state
                        .as_ref()
                        .map(|value| value.value.formalization_state_ref.clone()),
                    definition_ref: source.definition_ref,
                    catalog_entry_ref: source.catalog_entry_ref,
                },
                uow,
            );
        }
        if let Err(execution) = self.ensure_basis_summaries_exist(
            &source.basis_summary_refs,
            envelope,
            CommitUnknownReadBack::CurrentState {
                state_ref: current_state
                    .as_ref()
                    .map(|value| value.value.formalization_state_ref.clone()),
                definition_ref: source.definition_ref.clone(),
                catalog_entry_ref: source.catalog_entry_ref.clone(),
            },
            uow,
        ) {
            return execution;
        }

        let basis_resolution = match self
            .formalization_basis_resolver
            .resolve_formalization_basis(FormalizationBasisResolutionInput {
                definition_ref: source.definition_ref.clone(),
                catalog_entry_ref: Some(source.catalog_entry_ref.clone()),
                basis_summary_refs: source.basis_summary_refs,
                governance_basis_ref: None,
            }) {
            Ok(value) => value,
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };

        let reason_summary = method_library_contracts::FormalizationStateReasonSummary::new(
            source.trigger_marker_ref,
            basis_resolution.accepted_basis_summary_refs.clone(),
            None,
        );
        let mut state = if let Some(ref current_state) = current_state {
            let mut state = current_state.value.clone();
            if state
                .mark_assessment_pending(
                    reason_summary.clone(),
                    basis_resolution.accepted_basis_summary_refs.clone(),
                )
                .is_err()
            {
                return self.persisted_rejected_result(
                    envelope,
                    CommitUnknownReadBack::CurrentState {
                        state_ref: Some(state.formalization_state_ref.clone()),
                        definition_ref: source.definition_ref,
                        catalog_entry_ref: source.catalog_entry_ref,
                    },
                    uow,
                );
            }
            state
        } else {
            let formalization_state_ref = self.with_support_factory(|factory| {
                factory.new_formalization_state_ref(
                    source.definition_ref.clone(),
                    source.catalog_entry_ref.clone(),
                    envelope.operation_context_ref.clone(),
                    envelope.operation_digest_ref.clone(),
                    envelope.dedup_scope_ref.clone(),
                )
            });
            FormalizationState::pending_for_definition(
                formalization_state_ref,
                source.definition_ref.clone(),
                source.catalog_entry_ref.clone(),
                reason_summary,
            )
        };
        state.basis_summary_refs = basis_resolution.accepted_basis_summary_refs;

        let read_back = CommitUnknownReadBack::CurrentState {
            state_ref: Some(state.formalization_state_ref.clone()),
            definition_ref: source.definition_ref,
            catalog_entry_ref: source.catalog_entry_ref,
        };
        match self
            .formalization_state_repository
            .save_formalization_state(state, current_state.map(|value| value.version.into()), uow)
        {
            Ok(_) => self.persisted_accepted_result(envelope, read_back, uow),
            Err(MethodAssetRepositoryError::VersionConflict { .. }) => {
                ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                    envelope,
                    MethodAssetStoredOperationResultKind::Rejected,
                    Some(self.new_safe_reject_reason_ref()),
                ))
            }
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }

    fn establish_formal_version(
        &self,
        source: EstablishFormalMethodAssetVersionCommandSource,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let loaded_state = match self
            .formalization_state_repository
            .get_formalization_state_with_version(source.formalization_state_ref.clone())
        {
            Ok(Some(value)) => value,
            Ok(None) => {
                return self.persisted_rejected_result(
                    envelope,
                    CommitUnknownReadBack::CurrentState {
                        state_ref: Some(source.formalization_state_ref),
                        definition_ref: source.definition_ref,
                        catalog_entry_ref: source.catalog_entry_ref,
                    },
                    uow,
                );
            }
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };
        let (_, _) = match self.load_definition_and_catalog(
            source.definition_ref.clone(),
            source.catalog_entry_ref.clone(),
            envelope,
            uow,
        ) {
            Ok(value) => value,
            Err(execution) => return execution,
        };

        if loaded_state.value.state_kind != FormalizationStateKind::Eligible
            || loaded_state.value.definition_ref != source.definition_ref
            || loaded_state.value.catalog_entry_ref != source.catalog_entry_ref
            || source.version_boundary_summary.definition_ref != source.definition_ref
            || source.version_boundary_summary.catalog_entry_ref != source.catalog_entry_ref
            || source.version_boundary_summary.formalization_state_ref
                != source.formalization_state_ref
            || source.version_boundary_summary.basis_summary_refs
                != loaded_state.value.basis_summary_refs
        {
            return self.persisted_rejected_result(
                envelope,
                CommitUnknownReadBack::CurrentState {
                    state_ref: Some(loaded_state.value.formalization_state_ref.clone()),
                    definition_ref: source.definition_ref,
                    catalog_entry_ref: source.catalog_entry_ref,
                },
                uow,
            );
        }

        match self
            .formal_method_asset_version_repository
            .find_current_formal_method_asset_version(source.formalization_state_ref.clone())
        {
            Ok(Some(_)) => {
                return self.persisted_rejected_result(
                    envelope,
                    CommitUnknownReadBack::CurrentState {
                        state_ref: Some(loaded_state.value.formalization_state_ref.clone()),
                        definition_ref: source.definition_ref,
                        catalog_entry_ref: source.catalog_entry_ref,
                    },
                    uow,
                );
            }
            Ok(None) => {}
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        }

        let formal_version_ref = self.with_support_factory(|factory| {
            factory.new_formal_method_asset_version_ref(
                source.formalization_state_ref.clone(),
                source.definition_ref.clone(),
                source.catalog_entry_ref.clone(),
                source.version_boundary_summary.clone(),
                envelope.operation_context_ref.clone(),
                envelope.operation_digest_ref.clone(),
                envelope.dedup_scope_ref.clone(),
            )
        });
        let version = FormalMethodAssetVersion::from_formalization_state(
            formal_version_ref.clone(),
            source.version_boundary_summary,
        );
        let mut state = loaded_state.value;
        if state.mark_formalized(formal_version_ref.clone()).is_err() {
            return self.persisted_rejected_result(
                envelope,
                CommitUnknownReadBack::CurrentState {
                    state_ref: Some(state.formalization_state_ref.clone()),
                    definition_ref: source.definition_ref,
                    catalog_entry_ref: source.catalog_entry_ref,
                },
                uow,
            );
        }

        if self
            .formal_method_asset_version_repository
            .save_formal_method_asset_version(version, None, uow)
            .is_err()
        {
            return ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                envelope,
                MethodAssetStoredOperationResultKind::Rejected,
                Some(self.new_safe_reject_reason_ref()),
            ));
        }
        match self
            .formalization_state_repository
            .save_formalization_state(state.clone(), Some(loaded_state.version.into()), uow)
        {
            Ok(_) => self.persisted_accepted_result(
                envelope,
                CommitUnknownReadBack::ExactStateAndVersion {
                    state_ref: state.formalization_state_ref,
                    version_ref: formal_version_ref,
                },
                uow,
            ),
            Err(MethodAssetRepositoryError::VersionConflict { .. }) => {
                ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                    envelope,
                    MethodAssetStoredOperationResultKind::Rejected,
                    Some(self.new_safe_reject_reason_ref()),
                ))
            }
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }

    fn record_formal_version_semantic_change(
        &self,
        source: RecordFormalVersionSemanticChangeCommandSource,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let loaded_version = match self
            .formal_method_asset_version_repository
            .get_formal_method_asset_version_with_version(source.formal_version_ref.clone())
        {
            Ok(Some(value)) => value,
            Ok(None) => {
                return self.persisted_rejected_result(
                    envelope,
                    CommitUnknownReadBack::ExactVersions {
                        version_refs: vec![source.formal_version_ref],
                    },
                    uow,
                );
            }
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };
        if let Err(execution) = self.ensure_basis_summaries_exist(
            &source.basis_summary_refs,
            envelope,
            CommitUnknownReadBack::ExactVersions {
                version_refs: vec![source.formal_version_ref.clone()],
            },
            uow,
        ) {
            return execution;
        }
        if loaded_version.value.version_state
            != method_library_contracts::FormalMethodAssetVersionState::Active
        {
            return self.persisted_rejected_result(
                envelope,
                CommitUnknownReadBack::ExactVersions {
                    version_refs: vec![loaded_version.value.formal_version_ref.clone()],
                },
                uow,
            );
        }

        let diagnostic = match self
            .policy_diagnostic_builder
            .build_formal_version_change_diagnostic(
                &loaded_version.value,
                &source.basis_summary_refs,
                source.governance_basis_ref,
                source.semantic_change_marker_ref,
            ) {
            Ok(value) => value,
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };
        if diagnostic.blocking_reason_ref.is_some()
            || !diagnostic.accepted_change_marker_ref.is_public_safe()
        {
            return self.persisted_rejected_result(
                envelope,
                CommitUnknownReadBack::ExactVersions {
                    version_refs: vec![loaded_version.value.formal_version_ref.clone()],
                },
                uow,
            );
        }

        let mut version = loaded_version.value;
        version.version_state = method_library_contracts::FormalMethodAssetVersionState::Active;

        match self
            .formal_method_asset_version_repository
            .save_formal_method_asset_version(
                version.clone(),
                Some(loaded_version.version.into()),
                uow,
            ) {
            Ok(_) => self.persisted_accepted_result(
                envelope,
                CommitUnknownReadBack::ExactVersions {
                    version_refs: vec![version.formal_version_ref],
                },
                uow,
            ),
            Err(MethodAssetRepositoryError::VersionConflict { .. }) => {
                ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                    envelope,
                    MethodAssetStoredOperationResultKind::Rejected,
                    Some(self.new_safe_reject_reason_ref()),
                ))
            }
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }

    fn supersede_formal_version(
        &self,
        source: SupersedeFormalMethodAssetVersionCommandSource,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let previous = match self
            .formal_method_asset_version_repository
            .get_formal_method_asset_version_with_version(
                source.previous_formal_version_ref.clone(),
            ) {
            Ok(Some(value)) => value,
            Ok(None) => {
                return self.persisted_rejected_result(
                    envelope,
                    CommitUnknownReadBack::ExactVersions {
                        version_refs: vec![
                            source.previous_formal_version_ref,
                            source.next_formal_version_ref,
                        ],
                    },
                    uow,
                );
            }
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };
        let next = match self
            .formal_method_asset_version_repository
            .get_formal_method_asset_version_with_version(source.next_formal_version_ref.clone())
        {
            Ok(Some(value)) => value,
            Ok(None) => {
                return self.persisted_rejected_result(
                    envelope,
                    CommitUnknownReadBack::ExactVersions {
                        version_refs: vec![
                            source.previous_formal_version_ref,
                            source.next_formal_version_ref,
                        ],
                    },
                    uow,
                );
            }
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };
        if !source.supersession_marker_ref.is_public_safe()
            || previous.value.formal_version_ref == next.value.formal_version_ref
            || previous.value.definition_ref != next.value.definition_ref
            || next.value.supersedes_version_ref != Some(previous.value.formal_version_ref.clone())
            || previous.value.version_state
                == method_library_contracts::FormalMethodAssetVersionState::Retired
        {
            return self.persisted_rejected_result(
                envelope,
                CommitUnknownReadBack::ExactVersions {
                    version_refs: vec![
                        previous.value.formal_version_ref,
                        next.value.formal_version_ref,
                    ],
                },
                uow,
            );
        }

        let mut previous_version = previous.value;
        if previous_version
            .supersede_with(&next.value.formal_version_ref)
            .is_err()
        {
            return self.persisted_rejected_result(
                envelope,
                CommitUnknownReadBack::ExactVersions {
                    version_refs: vec![
                        previous_version.formal_version_ref,
                        next.value.formal_version_ref,
                    ],
                },
                uow,
            );
        }

        match self
            .formal_method_asset_version_repository
            .save_formal_method_asset_version(
                previous_version.clone(),
                Some(previous.version.into()),
                uow,
            ) {
            Ok(_) => self.persisted_accepted_result(
                envelope,
                CommitUnknownReadBack::ExactVersions {
                    version_refs: vec![
                        previous_version.formal_version_ref,
                        next.value.formal_version_ref,
                    ],
                },
                uow,
            ),
            Err(MethodAssetRepositoryError::VersionConflict { .. }) => {
                ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                    envelope,
                    MethodAssetStoredOperationResultKind::Rejected,
                    Some(self.new_safe_reject_reason_ref()),
                ))
            }
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }

    fn retire_formal_version(
        &self,
        source: RetireFormalMethodAssetVersionCommandSource,
        envelope: &MethodAssetFormalizationVersionReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let loaded_version = match self
            .formal_method_asset_version_repository
            .get_formal_method_asset_version_with_version(source.formal_version_ref.clone())
        {
            Ok(Some(value)) => value,
            Ok(None) => {
                return self.persisted_rejected_result(
                    envelope,
                    CommitUnknownReadBack::ExactVersions {
                        version_refs: vec![source.formal_version_ref],
                    },
                    uow,
                );
            }
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };

        let mut version = loaded_version.value;
        if !source.retirement_marker_ref.is_public_safe() || version.mark_retired().is_err() {
            return self.persisted_rejected_result(
                envelope,
                CommitUnknownReadBack::ExactVersions {
                    version_refs: vec![version.formal_version_ref],
                },
                uow,
            );
        }

        match self
            .formal_method_asset_version_repository
            .save_formal_method_asset_version(
                version.clone(),
                Some(loaded_version.version.into()),
                uow,
            ) {
            Ok(_) => self.persisted_accepted_result(
                envelope,
                CommitUnknownReadBack::ExactVersions {
                    version_refs: vec![version.formal_version_ref],
                },
                uow,
            ),
            Err(MethodAssetRepositoryError::VersionConflict { .. }) => {
                ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                    envelope,
                    MethodAssetStoredOperationResultKind::Rejected,
                    Some(self.new_safe_reject_reason_ref()),
                ))
            }
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }
}

impl MethodAssetFormalizationVersionCommandFacade
    for DefaultMethodAssetFormalizationVersionCommandFacade
{
    fn dispatch_formalization_version_command(
        &self,
        input: MethodAssetFormalizationVersionCommandDispatchInput,
    ) -> MethodAssetFormalizationVersionCommandDispatchOutput {
        let selector = match self.selector_from_shell(&input.command_shell) {
            Ok(selector) => selector,
            Err(output) => return output,
        };
        let envelope = match self.build_replay_envelope(&input, selector) {
            Ok(envelope) => envelope,
            Err(output) => return output,
        };
        match self.duplicate_or_conflict(&envelope) {
            Ok(Some(replayed)) => return replayed.into(),
            Ok(None) => {}
            Err(output) => return output,
        }

        self.execute_fresh_command(selector, input.command_source, envelope)
            .into()
    }
}
