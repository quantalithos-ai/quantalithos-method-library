//! Definition/catalog accepted-service vertical slice for `commit-03-b`.

use core::fmt;
use std::sync::{Arc, Mutex};

use method_library_contracts::{
    CatalogScopeRef, ExternalSourceSummaryRefSet, MethodAssetAcceptedOperationSummaryRef,
    MethodAssetApiEntryContextRef, MethodAssetApplicabilitySummary,
    MethodAssetApplicationDispatchRef, MethodAssetCatalogClassification,
    MethodAssetCatalogEntryRef, MethodAssetCatalogEntryRefSet, MethodAssetDedupScopeRef,
    MethodAssetDefinitionKind, MethodAssetDefinitionRef, MethodAssetDefinitionSummary,
    MethodAssetEffectSummaryRef, MethodAssetIdempotencyKeyRef, MethodAssetIdentityKey,
    MethodAssetOperationContextRef, MethodAssetOperationDigestRef, MethodAssetReplayMarkerRef,
    MethodAssetSafeIgnoreReasonRef, MethodAssetSafeRejectReasonRef,
    MethodAssetStoredOperationResultRef, MethodLibraryCapabilityKind, MethodLibraryCommandShell,
    MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRefKind,
};
use method_library_domain::{MethodAssetCatalogEntry, MethodAssetDefinition};

use crate::ports::{
    ExternalSourceSummaryValidationPort, MethodAssetCatalogEntryRepository,
    MethodAssetDefinitionRepository, MethodAssetStoredOperationResultRepository,
};
use crate::unit_of_work::{CommandUnitOfWork, UnitOfWork};

fn push_unique<T>(items: &mut Vec<T>, next: T)
where
    T: PartialEq,
{
    if !items.iter().any(|existing| existing == &next) {
        items.push(next);
    }
}

/// Application-owned facade input for the accepted service slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDefinitionCatalogCommandDispatchInput {
    /// Shared public command shell.
    pub command_shell: MethodLibraryCommandShell,
    /// Body-free structured command source.
    pub command_source: MethodAssetDefinitionCatalogCommandSource,
    /// API entry context minted by the support factory.
    pub api_entry_context_ref: MethodAssetApiEntryContextRef,
    /// Opaque dispatch target marker.
    pub application_dispatch_ref: MethodAssetApplicationDispatchRef,
}

/// Application-owned facade output copied from the stored-result safe surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDefinitionCatalogCommandDispatchOutput {
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

impl From<MethodAssetStoredOperationResult> for MethodAssetDefinitionCatalogCommandDispatchOutput {
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
pub enum MethodAssetDefinitionCatalogCommandSelector {
    /// Establish a new definition truth.
    EstablishDefinition,
    /// Adjust an existing definition truth.
    AdjustDefinition,
    /// Retire an existing definition truth.
    RetireDefinition,
    /// Register a new catalog entry for a definition.
    RegisterCatalogEntry,
    /// Reclassify an existing catalog entry.
    ReclassifyCatalogEntry,
    /// Retire an existing catalog entry.
    RetireCatalogEntry,
}

/// Application-owned body-free source carrier for the current boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetDefinitionCatalogCommandSource {
    /// Structured establish payload.
    EstablishDefinition(EstablishDefinitionCommandSource),
    /// Structured adjust payload.
    AdjustDefinition(AdjustDefinitionCommandSource),
    /// Structured definition-retire payload.
    RetireDefinition(RetireDefinitionCommandSource),
    /// Structured register payload.
    RegisterCatalogEntry(RegisterCatalogEntryCommandSource),
    /// Structured reclassify payload.
    ReclassifyCatalogEntry(ReclassifyCatalogEntryCommandSource),
    /// Structured catalog-retire payload.
    RetireCatalogEntry(RetireCatalogEntryCommandSource),
}

/// Source fields for `EstablishDefinition`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EstablishDefinitionCommandSource {
    /// Definition kind carried by the accepted source.
    pub definition_kind: MethodAssetDefinitionKind,
    /// Stable identity key.
    pub identity_key: MethodAssetIdentityKey,
    /// Body-free summary.
    pub definition_summary: MethodAssetDefinitionSummary,
    /// Accepted external summary refs.
    pub source_summary_refs: ExternalSourceSummaryRefSet,
    /// Preaccepted catalog refs linked at establish time.
    pub preaccepted_catalog_entry_refs: MethodAssetCatalogEntryRefSet,
}

/// Source fields for `AdjustDefinition`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdjustDefinitionCommandSource {
    /// Existing definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Replacement body-free summary.
    pub replacement_definition_summary: MethodAssetDefinitionSummary,
    /// Replacement external summary refs.
    pub replacement_source_summary_refs: ExternalSourceSummaryRefSet,
}

/// Source fields for `RetireDefinition`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetireDefinitionCommandSource {
    /// Existing definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Safe retirement marker.
    pub retirement_marker_ref: MethodLibrarySafeMarker,
}

/// Source fields for `RegisterCatalogEntry`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisterCatalogEntryCommandSource {
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Catalog scope anchor.
    pub catalog_scope_ref: CatalogScopeRef,
    /// Body-free classification.
    pub catalog_classification: MethodAssetCatalogClassification,
    /// Body-free applicability summary.
    pub applicability_summary: MethodAssetApplicabilitySummary,
}

/// Source fields for `ReclassifyCatalogEntry`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReclassifyCatalogEntryCommandSource {
    /// Existing catalog-entry anchor.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Replacement classification.
    pub new_catalog_classification: MethodAssetCatalogClassification,
    /// Replacement applicability summary.
    pub new_applicability_summary: MethodAssetApplicabilitySummary,
}

/// Source fields for `RetireCatalogEntry`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetireCatalogEntryCommandSource {
    /// Existing catalog-entry anchor.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Safe retirement marker.
    pub retirement_marker_ref: MethodLibrarySafeMarker,
}

/// Service-input enum for current-boundary dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetDefinitionCatalogCommandServiceInput {
    /// Establish definition service input.
    EstablishDefinition(EstablishMethodAssetDefinitionInput),
    /// Adjust definition service input.
    AdjustDefinition(AdjustMethodAssetDefinitionInput),
    /// Retire definition service input.
    RetireDefinition(RetireMethodAssetDefinitionInput),
    /// Register catalog-entry service input.
    RegisterCatalogEntry(RegisterMethodAssetCatalogEntryInput),
    /// Reclassify catalog-entry service input.
    ReclassifyCatalogEntry(ReclassifyMethodAssetCatalogEntryInput),
    /// Retire catalog-entry service input.
    RetireCatalogEntry(RetireMethodAssetCatalogEntryInput),
}

/// Shared replay-envelope fields plus establish payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EstablishMethodAssetDefinitionInput {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    /// Definition kind carried by the source.
    pub definition_kind: MethodAssetDefinitionKind,
    /// Stable identity key.
    pub identity_key: MethodAssetIdentityKey,
    /// Body-free summary.
    pub definition_summary: MethodAssetDefinitionSummary,
    /// Accepted source-summary refs.
    pub source_summary_refs: ExternalSourceSummaryRefSet,
    /// Preaccepted catalog-entry refs.
    pub preaccepted_catalog_entry_refs: MethodAssetCatalogEntryRefSet,
}

/// Shared replay-envelope fields plus adjust payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdjustMethodAssetDefinitionInput {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    /// Existing definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Loaded optimistic version.
    pub expected_version: MethodAssetExpectedVersion,
    /// Replacement body-free summary.
    pub replacement_definition_summary: MethodAssetDefinitionSummary,
    /// Replacement external summary refs.
    pub replacement_source_summary_refs: ExternalSourceSummaryRefSet,
}

/// Shared replay-envelope fields plus definition-retire payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetireMethodAssetDefinitionInput {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    /// Existing definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Loaded optimistic version.
    pub expected_version: MethodAssetExpectedVersion,
    /// Safe retirement marker.
    pub retirement_marker_ref: MethodLibrarySafeMarker,
}

/// Shared replay-envelope fields plus register payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisterMethodAssetCatalogEntryInput {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Catalog scope anchor.
    pub catalog_scope_ref: CatalogScopeRef,
    /// Body-free classification.
    pub catalog_classification: MethodAssetCatalogClassification,
    /// Body-free applicability summary.
    pub applicability_summary: MethodAssetApplicabilitySummary,
}

/// Shared replay-envelope fields plus reclassify payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReclassifyMethodAssetCatalogEntryInput {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    /// Existing catalog-entry anchor.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Loaded optimistic version.
    pub expected_version: MethodAssetExpectedVersion,
    /// Replacement classification.
    pub new_catalog_classification: MethodAssetCatalogClassification,
    /// Replacement applicability summary.
    pub new_applicability_summary: MethodAssetApplicabilitySummary,
}

/// Shared replay-envelope fields plus catalog-retire payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetireMethodAssetCatalogEntryInput {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
    /// Existing catalog-entry anchor.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Loaded optimistic version.
    pub expected_version: MethodAssetExpectedVersion,
    /// Safe retirement marker.
    pub retirement_marker_ref: MethodLibrarySafeMarker,
}

/// Deterministic set of effect summary refs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MethodAssetEffectSummaryRefSet {
    /// Effect refs ordered by first insertion after dedup.
    pub refs: Vec<MethodAssetEffectSummaryRef>,
}

impl MethodAssetEffectSummaryRefSet {
    /// Creates an empty effect-ref set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a new effect ref after dedup.
    pub fn insert(&mut self, next: MethodAssetEffectSummaryRef) {
        push_unique(&mut self.refs, next);
    }

    /// Creates a deterministic set from refs.
    pub fn from_refs(refs: impl IntoIterator<Item = MethodAssetEffectSummaryRef>) -> Self {
        let mut set = Self::new();
        for next in refs {
            set.insert(next);
        }
        set
    }
}

/// Replay-safe stored-result kinds for the current boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodAssetStoredOperationResultKind {
    /// The command accepted and mutated truth.
    Accepted,
    /// The command was safely rejected.
    Rejected,
    /// The command became a safe no-op.
    Ignored,
    /// The command conflicted with an existing replay record.
    Conflict,
}

/// Persistable or ephemeral safe result surface for the current boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetStoredOperationResult {
    /// Stable stored-result anchor.
    pub stored_result_ref: MethodAssetStoredOperationResultRef,
    /// Operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Replay-safe result kind.
    pub result_kind: MethodAssetStoredOperationResultKind,
    /// Accepted-summary anchor for successful commands.
    pub accepted_summary_ref: Option<MethodAssetAcceptedOperationSummaryRef>,
    /// Safe rejection reason when present.
    pub rejected_reason_ref: Option<MethodAssetSafeRejectReasonRef>,
    /// Safe ignore reason when present.
    pub ignored_reason_ref: Option<MethodAssetSafeIgnoreReasonRef>,
    /// Body-free effect refs.
    pub effect_summary_refs: MethodAssetEffectSummaryRefSet,
    /// Replay marker anchor.
    pub replay_marker_ref: MethodAssetReplayMarkerRef,
}

/// Opaque repository version token for the current boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MethodAssetRepositoryVersion(pub u64);

/// Expected optimistic version copied from a versioned load.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MethodAssetExpectedVersion(pub MethodAssetRepositoryVersion);

impl From<MethodAssetRepositoryVersion> for MethodAssetExpectedVersion {
    fn from(value: MethodAssetRepositoryVersion) -> Self {
        Self(value)
    }
}

/// Versioned truth loaded from a repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Versioned<T> {
    /// Persisted truth value.
    pub value: T,
    /// Opaque repository version.
    pub version: MethodAssetRepositoryVersion,
}

/// Saved ref together with the new repository version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionedRef<TRef> {
    /// Stable saved ref.
    pub value_ref: TRef,
    /// Opaque repository version after save.
    pub version: MethodAssetRepositoryVersion,
}

/// Exact repository error surface closed for the current boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetRepositoryError {
    /// Optimistic-version conflict on save.
    VersionConflict {
        /// Version expected by the caller, when any.
        expected_version: Option<MethodAssetExpectedVersion>,
        /// Version observed in storage.
        actual_version: MethodAssetRepositoryVersion,
        /// Safe conflict marker copied from the repository layer.
        conflict_marker_ref: MethodLibrarySafeMarker,
    },
    /// Duplicate stable-key conflict from a repository write.
    DuplicateKeyConflict {
        /// Safe duplicate marker copied from the repository layer.
        conflict_marker_ref: MethodLibrarySafeMarker,
    },
    /// The supplied unit of work is not active.
    TransactionNotActive {
        /// Safe failure marker copied from the repository layer.
        failure_marker_ref: MethodLibrarySafeMarker,
    },
    /// The storage layer is unavailable.
    StorageUnavailable {
        /// Safe unavailable marker copied from the repository layer.
        unavailable_marker_ref: MethodLibrarySafeMarker,
    },
    /// A stored-result lookup found inconsistent data.
    StoredResultIntegrityViolation {
        /// Stored-result ref involved in the violation, when known.
        stored_result_ref: Option<MethodAssetStoredOperationResultRef>,
        /// Safe consistency marker copied from the repository layer.
        violation_marker_ref: MethodLibrarySafeMarker,
    },
}

impl fmt::Display for MethodAssetRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VersionConflict { .. } => formatter.write_str("repository version conflict"),
            Self::DuplicateKeyConflict { .. } => {
                formatter.write_str("repository duplicate key conflict")
            }
            Self::TransactionNotActive { .. } => {
                formatter.write_str("repository transaction not active")
            }
            Self::StorageUnavailable { .. } => {
                formatter.write_str("repository storage unavailable")
            }
            Self::StoredResultIntegrityViolation { .. } => {
                formatter.write_str("stored result integrity violation")
            }
        }
    }
}

impl std::error::Error for MethodAssetRepositoryError {}

/// Input to the replay-envelope helper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDefinitionCatalogReplayEnvelopeFactoryInput {
    /// Shared public command shell.
    pub command_shell: MethodLibraryCommandShell,
    /// Body-free command source.
    pub command_source: MethodAssetDefinitionCatalogCommandSource,
    /// Selector derived from the shell boundary intent.
    pub selector: MethodAssetDefinitionCatalogCommandSelector,
    /// API entry context ref.
    pub api_entry_context_ref: MethodAssetApiEntryContextRef,
    /// Application dispatch marker.
    pub application_dispatch_ref: MethodAssetApplicationDispatchRef,
}

/// Shared replay-envelope fields copied into every service input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDefinitionCatalogReplayEnvelope {
    /// Opaque operation-context anchor.
    pub operation_context_ref: MethodAssetOperationContextRef,
    /// Idempotency-key anchor.
    pub idempotency_key_ref: MethodAssetIdempotencyKeyRef,
    /// Canonical operation digest.
    pub operation_digest_ref: MethodAssetOperationDigestRef,
    /// Dedup scope anchor.
    pub dedup_scope_ref: MethodAssetDedupScopeRef,
}

/// Exact replay-envelope build failures closed for the current boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MethodAssetReplayEnvelopeBuildError {
    /// Idempotency key is missing from command metadata.
    MissingIdempotencyKey {
        /// Safe rejection reason.
        reason_ref: MethodAssetSafeRejectReasonRef,
    },
    /// The dispatch target is not the current-boundary accepted marker.
    UnsupportedDispatchTarget {
        /// Safe rejection reason.
        reason_ref: MethodAssetSafeRejectReasonRef,
    },
    /// The shell selector and structured source variant disagree.
    SourceSelectorMismatch {
        /// Safe rejection reason.
        reason_ref: MethodAssetSafeRejectReasonRef,
    },
    /// Opaque-ref generation is temporarily unavailable.
    OpaqueRefGenerationUnavailable {
        /// Safe rejection reason.
        reason_ref: MethodAssetSafeRejectReasonRef,
    },
}

/// Exact helper surface for replay refs and truth refs.
pub trait MethodAssetDefinitionCatalogSupportRefFactory: Send {
    /// Returns the only current-boundary dispatch marker.
    fn definition_catalog_dispatch_ref(&self) -> MethodAssetApplicationDispatchRef;

    /// Mints a new API entry context ref.
    fn new_api_entry_context_ref(&mut self) -> MethodAssetApiEntryContextRef;

    /// Builds the shared replay envelope.
    fn build_definition_catalog_replay_envelope(
        &mut self,
        input: MethodAssetDefinitionCatalogReplayEnvelopeFactoryInput,
    ) -> Result<MethodAssetDefinitionCatalogReplayEnvelope, MethodAssetReplayEnvelopeBuildError>;

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

    /// Mints a new definition truth ref for the establish flow.
    fn new_definition_ref(
        &mut self,
        identity_key: MethodAssetIdentityKey,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> MethodAssetDefinitionRef;

    /// Mints a new catalog-entry truth ref for the register flow.
    fn new_catalog_entry_ref(
        &mut self,
        definition_ref: MethodAssetDefinitionRef,
        catalog_scope_ref: CatalogScopeRef,
        catalog_classification: MethodAssetCatalogClassification,
        applicability_summary: MethodAssetApplicabilitySummary,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> MethodAssetCatalogEntryRef;
}

/// Facade surface called by the minimal API entry.
pub trait MethodAssetDefinitionCatalogCommandFacade: Send + Sync {
    /// Dispatches a body-free command shell into the accepted service slice.
    fn dispatch_definition_catalog_command(
        &self,
        input: MethodAssetDefinitionCatalogCommandDispatchInput,
    ) -> MethodAssetDefinitionCatalogCommandDispatchOutput;
}

enum ServiceExecution {
    Persisted(MethodAssetStoredOperationResult),
    Ephemeral(MethodAssetStoredOperationResult),
}

/// Default current-boundary facade implementation.
pub struct DefaultMethodAssetDefinitionCatalogCommandFacade {
    definition_repository: Arc<dyn MethodAssetDefinitionRepository>,
    catalog_repository: Arc<dyn MethodAssetCatalogEntryRepository>,
    stored_result_repository: Arc<dyn MethodAssetStoredOperationResultRepository>,
    external_summary_validation: Arc<dyn ExternalSourceSummaryValidationPort>,
    unit_of_work: Arc<dyn UnitOfWork>,
    support_ref_factory: Arc<Mutex<Box<dyn MethodAssetDefinitionCatalogSupportRefFactory>>>,
}

impl DefaultMethodAssetDefinitionCatalogCommandFacade {
    /// Creates the current-boundary facade from formal ports and helper surfaces.
    pub fn new(
        definition_repository: Arc<dyn MethodAssetDefinitionRepository>,
        catalog_repository: Arc<dyn MethodAssetCatalogEntryRepository>,
        stored_result_repository: Arc<dyn MethodAssetStoredOperationResultRepository>,
        external_summary_validation: Arc<dyn ExternalSourceSummaryValidationPort>,
        unit_of_work: Arc<dyn UnitOfWork>,
        support_ref_factory: Arc<Mutex<Box<dyn MethodAssetDefinitionCatalogSupportRefFactory>>>,
    ) -> Self {
        Self {
            definition_repository,
            catalog_repository,
            stored_result_repository,
            external_summary_validation,
            unit_of_work,
            support_ref_factory,
        }
    }

    fn with_support_factory<R>(
        &self,
        action: impl FnOnce(&mut dyn MethodAssetDefinitionCatalogSupportRefFactory) -> R,
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
    ) -> MethodAssetDefinitionCatalogCommandDispatchOutput {
        let stored_result_ref =
            self.with_support_factory(|factory| factory.new_stored_operation_result_ref());
        let replay_marker_ref =
            self.with_support_factory(|factory| factory.new_replay_marker_ref());

        MethodAssetDefinitionCatalogCommandDispatchOutput {
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
        envelope: &MethodAssetDefinitionCatalogReplayEnvelope,
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
        envelope: &MethodAssetDefinitionCatalogReplayEnvelope,
        stored_result: MethodAssetStoredOperationResult,
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
            Ok(_) => ServiceExecution::Persisted(stored_result),
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }

    fn persisted_rejected_result(
        &self,
        envelope: &MethodAssetDefinitionCatalogReplayEnvelope,
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
        self.persisted_result(envelope, stored_result, uow)
    }

    fn persisted_accepted_result(
        &self,
        envelope: &MethodAssetDefinitionCatalogReplayEnvelope,
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
        self.persisted_result(envelope, stored_result, uow)
    }

    fn ephemeral_result_from_repository_error(
        &self,
        envelope: &MethodAssetDefinitionCatalogReplayEnvelope,
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
        MethodAssetDefinitionCatalogCommandSelector,
        MethodAssetDefinitionCatalogCommandDispatchOutput,
    > {
        if command_shell.capability_kind != MethodLibraryCapabilityKind::DefinitionCatalog {
            return Err(self.early_rejected_output(self.new_safe_reject_reason_ref()));
        }

        match command_shell.boundary_ref.kind() {
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionEstablishIntent => {
                Ok(MethodAssetDefinitionCatalogCommandSelector::EstablishDefinition)
            }
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionAdjustIntent => {
                Ok(MethodAssetDefinitionCatalogCommandSelector::AdjustDefinition)
            }
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionRetireIntent => {
                Ok(MethodAssetDefinitionCatalogCommandSelector::RetireDefinition)
            }
            MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntryRegisterIntent => {
                Ok(MethodAssetDefinitionCatalogCommandSelector::RegisterCatalogEntry)
            }
            MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntryReclassifyIntent => {
                Ok(MethodAssetDefinitionCatalogCommandSelector::ReclassifyCatalogEntry)
            }
            MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntryRetireIntent => {
                Ok(MethodAssetDefinitionCatalogCommandSelector::RetireCatalogEntry)
            }
            _ => Err(self.early_rejected_output(self.new_safe_reject_reason_ref())),
        }
    }

    fn build_replay_envelope(
        &self,
        input: &MethodAssetDefinitionCatalogCommandDispatchInput,
        selector: MethodAssetDefinitionCatalogCommandSelector,
    ) -> Result<
        MethodAssetDefinitionCatalogReplayEnvelope,
        MethodAssetDefinitionCatalogCommandDispatchOutput,
    > {
        self.with_support_factory(|factory| {
            factory.build_definition_catalog_replay_envelope(
                MethodAssetDefinitionCatalogReplayEnvelopeFactoryInput {
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
        envelope: &MethodAssetDefinitionCatalogReplayEnvelope,
    ) -> Result<
        Option<MethodAssetStoredOperationResult>,
        MethodAssetDefinitionCatalogCommandDispatchOutput,
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

    fn execute_fresh_command(
        &self,
        selector: MethodAssetDefinitionCatalogCommandSelector,
        command_source: MethodAssetDefinitionCatalogCommandSource,
        envelope: MethodAssetDefinitionCatalogReplayEnvelope,
    ) -> MethodAssetStoredOperationResult {
        let mut uow = self.unit_of_work.begin_command_uow();
        let execution = match selector {
            MethodAssetDefinitionCatalogCommandSelector::EstablishDefinition => {
                match command_source {
                    MethodAssetDefinitionCatalogCommandSource::EstablishDefinition(source) => {
                        self.establish_definition(source, &envelope, uow.as_mut())
                    }
                    _ => ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                        &envelope,
                        MethodAssetStoredOperationResultKind::Rejected,
                        Some(self.new_safe_reject_reason_ref()),
                    )),
                }
            }
            MethodAssetDefinitionCatalogCommandSelector::AdjustDefinition => match command_source {
                MethodAssetDefinitionCatalogCommandSource::AdjustDefinition(source) => {
                    self.adjust_definition(source, &envelope, uow.as_mut())
                }
                _ => ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                    &envelope,
                    MethodAssetStoredOperationResultKind::Rejected,
                    Some(self.new_safe_reject_reason_ref()),
                )),
            },
            MethodAssetDefinitionCatalogCommandSelector::RetireDefinition => match command_source {
                MethodAssetDefinitionCatalogCommandSource::RetireDefinition(source) => {
                    self.retire_definition(source, &envelope, uow.as_mut())
                }
                _ => ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                    &envelope,
                    MethodAssetStoredOperationResultKind::Rejected,
                    Some(self.new_safe_reject_reason_ref()),
                )),
            },
            MethodAssetDefinitionCatalogCommandSelector::RegisterCatalogEntry => {
                match command_source {
                    MethodAssetDefinitionCatalogCommandSource::RegisterCatalogEntry(source) => {
                        self.register_catalog_entry(source, &envelope, uow.as_mut())
                    }
                    _ => ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                        &envelope,
                        MethodAssetStoredOperationResultKind::Rejected,
                        Some(self.new_safe_reject_reason_ref()),
                    )),
                }
            }
            MethodAssetDefinitionCatalogCommandSelector::ReclassifyCatalogEntry => {
                match command_source {
                    MethodAssetDefinitionCatalogCommandSource::ReclassifyCatalogEntry(source) => {
                        self.reclassify_catalog_entry(source, &envelope, uow.as_mut())
                    }
                    _ => ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                        &envelope,
                        MethodAssetStoredOperationResultKind::Rejected,
                        Some(self.new_safe_reject_reason_ref()),
                    )),
                }
            }
            MethodAssetDefinitionCatalogCommandSelector::RetireCatalogEntry => match command_source
            {
                MethodAssetDefinitionCatalogCommandSource::RetireCatalogEntry(source) => {
                    self.retire_catalog_entry(source, &envelope, uow.as_mut())
                }
                _ => ServiceExecution::Ephemeral(self.new_ephemeral_result_from_envelope(
                    &envelope,
                    MethodAssetStoredOperationResultKind::Rejected,
                    Some(self.new_safe_reject_reason_ref()),
                )),
            },
        };

        match execution {
            ServiceExecution::Persisted(stored_result) => {
                let _ = uow.commit();
                stored_result
            }
            ServiceExecution::Ephemeral(stored_result) => {
                let _ = uow.rollback();
                stored_result
            }
        }
    }

    fn establish_definition(
        &self,
        source: EstablishDefinitionCommandSource,
        envelope: &MethodAssetDefinitionCatalogReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let input = EstablishMethodAssetDefinitionInput {
            operation_context_ref: envelope.operation_context_ref.clone(),
            idempotency_key_ref: envelope.idempotency_key_ref.clone(),
            operation_digest_ref: envelope.operation_digest_ref.clone(),
            dedup_scope_ref: envelope.dedup_scope_ref.clone(),
            definition_kind: source.definition_kind,
            identity_key: source.identity_key,
            definition_summary: source.definition_summary,
            source_summary_refs: source.source_summary_refs,
            preaccepted_catalog_entry_refs: source.preaccepted_catalog_entry_refs,
        };

        if self
            .external_summary_validation
            .validate_named_refs(&input.source_summary_refs)
            .is_err()
        {
            return self.persisted_rejected_result(envelope, uow);
        }

        if input.definition_kind != input.identity_key.definition_kind
            || input.definition_summary.definition_kind != input.definition_kind
        {
            return self.persisted_rejected_result(envelope, uow);
        }

        match self
            .definition_repository
            .find_definition_by_identity_key(input.identity_key.clone())
        {
            Ok(Some(_)) => return self.persisted_rejected_result(envelope, uow),
            Ok(None) => {}
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        }

        let definition_ref = self.with_support_factory(|factory| {
            factory.new_definition_ref(
                input.identity_key.clone(),
                input.operation_context_ref.clone(),
                input.operation_digest_ref.clone(),
                input.dedup_scope_ref.clone(),
            )
        });

        let mut definition = MethodAssetDefinition::create(
            definition_ref,
            input.identity_key,
            input.definition_summary,
        );
        if definition.assert_body_free().is_err() {
            return self.persisted_rejected_result(envelope, uow);
        }

        for source_summary_ref in input.source_summary_refs.refs {
            definition.accept_source_summary(source_summary_ref);
        }
        for catalog_entry_ref in input.preaccepted_catalog_entry_refs.refs {
            definition.link_catalog_entry(catalog_entry_ref);
        }

        match self
            .definition_repository
            .save_definition(definition, None, uow)
        {
            Ok(_) => self.persisted_accepted_result(envelope, uow),
            Err(MethodAssetRepositoryError::VersionConflict { .. })
            | Err(MethodAssetRepositoryError::DuplicateKeyConflict { .. }) => {
                self.persisted_rejected_result(envelope, uow)
            }
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }

    fn adjust_definition(
        &self,
        source: AdjustDefinitionCommandSource,
        envelope: &MethodAssetDefinitionCatalogReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let loaded_definition = match self
            .definition_repository
            .get_definition_with_version(source.definition_ref.clone())
        {
            Ok(Some(value)) => value,
            Ok(None) => return self.persisted_rejected_result(envelope, uow),
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };

        if self
            .external_summary_validation
            .validate_named_refs(&source.replacement_source_summary_refs)
            .is_err()
        {
            return self.persisted_rejected_result(envelope, uow);
        }

        let input = AdjustMethodAssetDefinitionInput {
            operation_context_ref: envelope.operation_context_ref.clone(),
            idempotency_key_ref: envelope.idempotency_key_ref.clone(),
            operation_digest_ref: envelope.operation_digest_ref.clone(),
            dedup_scope_ref: envelope.dedup_scope_ref.clone(),
            definition_ref: source.definition_ref,
            expected_version: loaded_definition.version.into(),
            replacement_definition_summary: source.replacement_definition_summary,
            replacement_source_summary_refs: source.replacement_source_summary_refs,
        };

        let mut definition = loaded_definition.value;
        if definition.assert_active_for_adjust().is_err() {
            return self.persisted_rejected_result(envelope, uow);
        }
        if definition
            .apply_adjustment(
                input.replacement_definition_summary,
                input.replacement_source_summary_refs,
            )
            .is_err()
        {
            return self.persisted_rejected_result(envelope, uow);
        }
        if definition.assert_body_free().is_err() {
            return self.persisted_rejected_result(envelope, uow);
        }

        match self.definition_repository.save_definition(
            definition,
            Some(input.expected_version),
            uow,
        ) {
            Ok(_) => self.persisted_accepted_result(envelope, uow),
            Err(MethodAssetRepositoryError::VersionConflict { .. }) => {
                self.persisted_rejected_result(envelope, uow)
            }
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }

    fn retire_definition(
        &self,
        source: RetireDefinitionCommandSource,
        envelope: &MethodAssetDefinitionCatalogReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let loaded_definition = match self
            .definition_repository
            .get_definition_with_version(source.definition_ref.clone())
        {
            Ok(Some(value)) => value,
            Ok(None) => return self.persisted_rejected_result(envelope, uow),
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };

        let input = RetireMethodAssetDefinitionInput {
            operation_context_ref: envelope.operation_context_ref.clone(),
            idempotency_key_ref: envelope.idempotency_key_ref.clone(),
            operation_digest_ref: envelope.operation_digest_ref.clone(),
            dedup_scope_ref: envelope.dedup_scope_ref.clone(),
            definition_ref: source.definition_ref,
            expected_version: loaded_definition.version.into(),
            retirement_marker_ref: source.retirement_marker_ref,
        };

        let mut definition = loaded_definition.value;
        if definition
            .mark_retired(input.retirement_marker_ref)
            .is_err()
        {
            return self.persisted_rejected_result(envelope, uow);
        }

        match self.definition_repository.save_definition(
            definition,
            Some(input.expected_version),
            uow,
        ) {
            Ok(_) => self.persisted_accepted_result(envelope, uow),
            Err(MethodAssetRepositoryError::VersionConflict { .. }) => {
                self.persisted_rejected_result(envelope, uow)
            }
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }

    fn register_catalog_entry(
        &self,
        source: RegisterCatalogEntryCommandSource,
        envelope: &MethodAssetDefinitionCatalogReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        match self
            .definition_repository
            .get_definition_with_version(source.definition_ref.clone())
        {
            Ok(Some(_)) => {}
            Ok(None) => return self.persisted_rejected_result(envelope, uow),
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        }

        let input = RegisterMethodAssetCatalogEntryInput {
            operation_context_ref: envelope.operation_context_ref.clone(),
            idempotency_key_ref: envelope.idempotency_key_ref.clone(),
            operation_digest_ref: envelope.operation_digest_ref.clone(),
            dedup_scope_ref: envelope.dedup_scope_ref.clone(),
            definition_ref: source.definition_ref,
            catalog_scope_ref: source.catalog_scope_ref,
            catalog_classification: source.catalog_classification,
            applicability_summary: source.applicability_summary,
        };

        match self
            .catalog_repository
            .find_catalog_entry_by_definition_scope(
                input.definition_ref.clone(),
                input.catalog_scope_ref.clone(),
            ) {
            Ok(Some(_)) => return self.persisted_rejected_result(envelope, uow),
            Ok(None) => {}
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        }

        let catalog_entry_ref = self.with_support_factory(|factory| {
            factory.new_catalog_entry_ref(
                input.definition_ref.clone(),
                input.catalog_scope_ref.clone(),
                input.catalog_classification.clone(),
                input.applicability_summary.clone(),
                input.operation_context_ref.clone(),
                input.operation_digest_ref.clone(),
                input.dedup_scope_ref.clone(),
            )
        });

        let catalog_entry = match MethodAssetCatalogEntry::create_for_definition(
            catalog_entry_ref,
            input.definition_ref,
            input.catalog_scope_ref,
            input.catalog_classification,
            input.applicability_summary,
        ) {
            Ok(value) => value,
            Err(_) => return self.persisted_rejected_result(envelope, uow),
        };

        match self
            .catalog_repository
            .save_catalog_entry(catalog_entry, None, uow)
        {
            Ok(_) => self.persisted_accepted_result(envelope, uow),
            Err(MethodAssetRepositoryError::VersionConflict { .. })
            | Err(MethodAssetRepositoryError::DuplicateKeyConflict { .. }) => {
                self.persisted_rejected_result(envelope, uow)
            }
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }

    fn reclassify_catalog_entry(
        &self,
        source: ReclassifyCatalogEntryCommandSource,
        envelope: &MethodAssetDefinitionCatalogReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let loaded_catalog_entry = match self
            .catalog_repository
            .get_catalog_entry_with_version(source.catalog_entry_ref.clone())
        {
            Ok(Some(value)) => value,
            Ok(None) => return self.persisted_rejected_result(envelope, uow),
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };

        let input = ReclassifyMethodAssetCatalogEntryInput {
            operation_context_ref: envelope.operation_context_ref.clone(),
            idempotency_key_ref: envelope.idempotency_key_ref.clone(),
            operation_digest_ref: envelope.operation_digest_ref.clone(),
            dedup_scope_ref: envelope.dedup_scope_ref.clone(),
            catalog_entry_ref: source.catalog_entry_ref,
            expected_version: loaded_catalog_entry.version.into(),
            new_catalog_classification: source.new_catalog_classification,
            new_applicability_summary: source.new_applicability_summary,
        };

        let mut catalog_entry = loaded_catalog_entry.value;
        if catalog_entry
            .reclassify(
                input.new_catalog_classification,
                input.new_applicability_summary,
            )
            .is_err()
        {
            return self.persisted_rejected_result(envelope, uow);
        }

        match self.catalog_repository.save_catalog_entry(
            catalog_entry,
            Some(input.expected_version),
            uow,
        ) {
            Ok(_) => self.persisted_accepted_result(envelope, uow),
            Err(MethodAssetRepositoryError::VersionConflict { .. }) => {
                self.persisted_rejected_result(envelope, uow)
            }
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }

    fn retire_catalog_entry(
        &self,
        source: RetireCatalogEntryCommandSource,
        envelope: &MethodAssetDefinitionCatalogReplayEnvelope,
        uow: &mut dyn CommandUnitOfWork,
    ) -> ServiceExecution {
        let loaded_catalog_entry = match self
            .catalog_repository
            .get_catalog_entry_with_version(source.catalog_entry_ref.clone())
        {
            Ok(Some(value)) => value,
            Ok(None) => return self.persisted_rejected_result(envelope, uow),
            Err(error) => {
                return ServiceExecution::Ephemeral(
                    self.ephemeral_result_from_repository_error(envelope, error),
                );
            }
        };

        let input = RetireMethodAssetCatalogEntryInput {
            operation_context_ref: envelope.operation_context_ref.clone(),
            idempotency_key_ref: envelope.idempotency_key_ref.clone(),
            operation_digest_ref: envelope.operation_digest_ref.clone(),
            dedup_scope_ref: envelope.dedup_scope_ref.clone(),
            catalog_entry_ref: source.catalog_entry_ref,
            expected_version: loaded_catalog_entry.version.into(),
            retirement_marker_ref: source.retirement_marker_ref,
        };

        let mut catalog_entry = loaded_catalog_entry.value;
        if catalog_entry
            .mark_retired(input.retirement_marker_ref)
            .is_err()
        {
            return self.persisted_rejected_result(envelope, uow);
        }

        match self.catalog_repository.save_catalog_entry(
            catalog_entry,
            Some(input.expected_version),
            uow,
        ) {
            Ok(_) => self.persisted_accepted_result(envelope, uow),
            Err(MethodAssetRepositoryError::VersionConflict { .. }) => {
                self.persisted_rejected_result(envelope, uow)
            }
            Err(error) => ServiceExecution::Ephemeral(
                self.ephemeral_result_from_repository_error(envelope, error),
            ),
        }
    }
}

impl MethodAssetDefinitionCatalogCommandFacade
    for DefaultMethodAssetDefinitionCatalogCommandFacade
{
    fn dispatch_definition_catalog_command(
        &self,
        input: MethodAssetDefinitionCatalogCommandDispatchInput,
    ) -> MethodAssetDefinitionCatalogCommandDispatchOutput {
        let selector = match self.selector_from_shell(&input.command_shell) {
            Ok(selector) => selector,
            Err(stored_result) => return stored_result.into(),
        };

        let envelope = match self.build_replay_envelope(&input, selector) {
            Ok(envelope) => envelope,
            Err(stored_result) => return stored_result.into(),
        };

        match self.duplicate_or_conflict(&envelope) {
            Ok(Some(stored_result)) => return stored_result.into(),
            Ok(None) => {}
            Err(stored_result) => return stored_result.into(),
        }

        self.execute_fresh_command(selector, input.command_source, envelope)
            .into()
    }
}
