//! In-memory fake/runtime support for the `commit-05-b` distribution/handoff slice.

use core::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use method_library_application::ports::MethodAssetStoredOperationResultRepository;
use method_library_application::{
    DefaultMethodAssetDistributionHandoffCommandFacade, DistributionReadMaterialBuildOutcome,
    DistributionReadMaterialBuilderInput, DistributionReadMaterialBuilderPort,
    MethodAssetAdapterAvailabilityPort, MethodAssetAdapterAvailabilitySummary,
    MethodAssetCollaborationHandoffPort, MethodAssetCollaborationTargetRegistryPort,
    MethodAssetCollaborationTargetSummary, MethodAssetCommitObservation,
    MethodAssetConsumptionAvailabilityResolverPort, MethodAssetDegradedDecisionMapperPort,
    MethodAssetDistributionEventCandidateAssembly, MethodAssetDistributionHandoffCommandFacade,
    MethodAssetDistributionHandoffCommandSelector, MethodAssetDistributionHandoffCommandSource,
    MethodAssetDistributionHandoffInput, MethodAssetDistributionHandoffReplayEnvelope,
    MethodAssetDistributionHandoffReplayEnvelopeFactoryInput,
    MethodAssetDistributionHandoffSeamSource, MethodAssetDistributionHandoffSupportRefFactory,
    MethodAssetDistributionRecord, MethodAssetDistributionRepository,
    MethodAssetEventCandidateAssemblyRepository, MethodAssetEventCandidatePublicationInput,
    MethodAssetEventCandidatePublisherPort, MethodAssetExpectedVersion,
    MethodAssetHandoffMarkerRepository, MethodAssetHandoffOutcome, MethodAssetPublicationOutcome,
    MethodAssetPublicationOutcomeRepository, MethodAssetRelationReadAnchor,
    MethodAssetRelationRepository, MethodAssetReplayEnvelopeBuildError, MethodAssetRepositoryError,
    MethodAssetRepositoryVersion, MethodAssetStoredOperationResult, UnitOfWork, Versioned,
    VersionedRef,
};
use method_library_contracts::{
    DistributionContextRef, MethodAssetAcceptedOperationSummaryRef,
    MethodAssetAdapterAvailabilityStateRef, MethodAssetAdapterSlotRefSet,
    MethodAssetApiEntryContextRef, MethodAssetApplicationDispatchRef,
    MethodAssetConsumptionAvailabilityMarker, MethodAssetDedupScopeRef,
    MethodAssetDegradedDecisionRef, MethodAssetDistributionRef, MethodAssetEffectSummaryRef,
    MethodAssetEventCandidateAssemblyRef, MethodAssetHandoffBindingStateRef,
    MethodAssetHandoffMarkerRef, MethodAssetHandoffTargetRef, MethodAssetHandoffTargetRefSet,
    MethodAssetIdempotencyKeyRef, MethodAssetInfraSafeDiagnosticRef,
    MethodAssetOperationContextRef, MethodAssetOperationDigestRef,
    MethodAssetPublicationOutcomeRef, MethodAssetPublisherBindingStateRef, MethodAssetRelationRef,
    MethodAssetReplayMarkerRef, MethodAssetSafeIgnoreReasonRef, MethodAssetSafeRejectReasonRef,
    MethodAssetStoredOperationResultRef, MethodAssetTargetRegistryScopeRef,
    MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};

fn stable_hash(input: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn canonical_typed_ref(value: &MethodLibraryTypedBoundaryRef) -> String {
    format!("{:?}:{}", value.kind(), value.as_public_ref())
}

fn canonical_marker(value: &MethodLibrarySafeMarker) -> String {
    format!(
        "{:?}:{}",
        value.marker_kind(),
        canonical_typed_ref(&value.source_ref)
    )
}

fn canonical_availability_marker(value: &MethodAssetConsumptionAvailabilityMarker) -> String {
    format!(
        "{}|{:?}|{:?}|{}|{}",
        canonical_marker(&value.marker_ref),
        value.target_state,
        value.source_kind,
        canonical_marker(&value.source_marker_ref),
        value
            .reason_ref
            .as_ref()
            .map(canonical_marker)
            .unwrap_or_else(|| "-".to_owned()),
    )
}

fn canonical_slot_set(value: &MethodAssetAdapterSlotRefSet) -> String {
    value
        .refs
        .iter()
        .map(|item| item.as_public_ref().to_owned())
        .collect::<Vec<_>>()
        .join(",")
}

fn canonical_target_set(value: &MethodAssetHandoffTargetRefSet) -> String {
    value
        .refs
        .iter()
        .map(|item| item.as_public_ref().to_owned())
        .collect::<Vec<_>>()
        .join(",")
}

fn canonical_seam_source(value: &Option<MethodAssetDistributionHandoffSeamSource>) -> String {
    let Some(value) = value else {
        return "none".to_owned();
    };
    format!(
        "some|{}|{}|{}|{}|{}|{}",
        value.target_registry_scope_ref.as_public_ref(),
        canonical_slot_set(&value.required_slot_refs),
        value.publisher_binding_ref.as_public_ref(),
        value
            .handoff_binding_ref
            .as_ref()
            .map(|item| item.as_public_ref().to_owned())
            .unwrap_or_else(|| "-".to_owned()),
        canonical_marker(value.publication_boundary_marker_ref.as_safe_marker()),
        value
            .handoff_boundary_marker_ref
            .as_ref()
            .map(|item| canonical_marker(item.as_safe_marker()))
            .unwrap_or_else(|| "-".to_owned()),
    )
}

fn canonical_source(value: &MethodAssetDistributionHandoffCommandSource) -> String {
    match value {
        MethodAssetDistributionHandoffCommandSource::PrepareDistributionRef {
            relation_ref,
            requested_distribution_ref,
            distribution_context_ref,
            consumption_context_ref,
            boundary_ref,
            availability_marker,
            candidate_reason_ref,
        } => format!(
            "prepare|{}|{}|{}|{}|{}|{}|{}",
            relation_ref.as_public_ref(),
            requested_distribution_ref
                .as_ref()
                .map(|item| item.as_public_ref().to_owned())
                .unwrap_or_else(|| "-".to_owned()),
            distribution_context_ref.as_public_ref(),
            consumption_context_ref.as_public_ref(),
            boundary_ref.as_public_ref(),
            canonical_availability_marker(availability_marker),
            canonical_marker(candidate_reason_ref.as_safe_marker()),
        ),
        MethodAssetDistributionHandoffCommandSource::AdjustDistributionContext {
            relation_ref,
            distribution_ref,
            previous_context_ref,
            new_context_ref,
            adjustment_reason_ref,
            candidate_reason_ref,
            expected_distribution_version,
        } => format!(
            "adjust|{}|{}|{}|{}|{}|{}|{}",
            relation_ref.as_public_ref(),
            distribution_ref.as_public_ref(),
            previous_context_ref.as_public_ref(),
            new_context_ref.as_public_ref(),
            canonical_marker(adjustment_reason_ref.as_safe_marker()),
            canonical_marker(candidate_reason_ref.as_safe_marker()),
            (expected_distribution_version.0).0,
        ),
        MethodAssetDistributionHandoffCommandSource::MarkDistributionAvailability {
            relation_ref,
            distribution_ref,
            distribution_context_ref,
            availability_marker,
            candidate_reason_ref,
        } => format!(
            "mark|{}|{}|{}|{}|{}",
            relation_ref.as_public_ref(),
            distribution_ref.as_public_ref(),
            distribution_context_ref.as_public_ref(),
            canonical_availability_marker(availability_marker),
            canonical_marker(candidate_reason_ref.as_safe_marker()),
        ),
    }
}

fn canonical_selector(value: MethodAssetDistributionHandoffCommandSelector) -> &'static str {
    match value {
        MethodAssetDistributionHandoffCommandSelector::PrepareDistributionRef => {
            "prepare_distribution_ref"
        }
        MethodAssetDistributionHandoffCommandSelector::AdjustDistributionContext => {
            "adjust_distribution_context"
        }
        MethodAssetDistributionHandoffCommandSelector::MarkDistributionAvailability => {
            "mark_distribution_availability"
        }
    }
}

fn selector_matches_source(
    selector: MethodAssetDistributionHandoffCommandSelector,
    source: &MethodAssetDistributionHandoffCommandSource,
) -> bool {
    matches!(
        (selector, source),
        (
            MethodAssetDistributionHandoffCommandSelector::PrepareDistributionRef,
            MethodAssetDistributionHandoffCommandSource::PrepareDistributionRef { .. }
        ) | (
            MethodAssetDistributionHandoffCommandSelector::AdjustDistributionContext,
            MethodAssetDistributionHandoffCommandSource::AdjustDistributionContext { .. }
        ) | (
            MethodAssetDistributionHandoffCommandSelector::MarkDistributionAvailability,
            MethodAssetDistributionHandoffCommandSource::MarkDistributionAvailability { .. }
        )
    )
}

fn canonical_dedup_scope(
    selector: MethodAssetDistributionHandoffCommandSelector,
    source: &MethodAssetDistributionHandoffCommandSource,
) -> String {
    match source {
        MethodAssetDistributionHandoffCommandSource::PrepareDistributionRef {
            relation_ref,
            ..
        } => format!(
            "distribution_handoff|{}|relation|{}",
            canonical_selector(selector),
            relation_ref.as_public_ref()
        ),
        MethodAssetDistributionHandoffCommandSource::AdjustDistributionContext {
            distribution_ref,
            ..
        }
        | MethodAssetDistributionHandoffCommandSource::MarkDistributionAvailability {
            distribution_ref,
            ..
        } => format!(
            "distribution_handoff|{}|distribution|{}",
            canonical_selector(selector),
            distribution_ref.as_public_ref()
        ),
    }
}

fn canonical_operation_digest(
    input: &MethodAssetDistributionHandoffReplayEnvelopeFactoryInput,
) -> String {
    let typed_refs = input
        .command_shell
        .typed_refs
        .iter()
        .map(canonical_typed_ref)
        .collect::<Vec<_>>()
        .join(",");
    let safe_markers = input
        .command_shell
        .safe_markers
        .iter()
        .map(canonical_marker)
        .collect::<Vec<_>>()
        .join(",");
    let idempotency = input
        .command_shell
        .metadata
        .request
        .idempotency_key
        .as_ref()
        .map(|value| value.as_str())
        .unwrap_or("-");
    format!(
        "{:?}|{:?}|{}|{}|{}|{}|{}|{}|{}",
        input.command_shell.capability_kind,
        input.command_shell.boundary_ref.kind(),
        canonical_selector(input.selector),
        canonical_source(&input.command_source),
        canonical_seam_source(&input.seam_source),
        input.application_dispatch_ref.as_public_ref(),
        idempotency,
        typed_refs,
        safe_markers,
    )
}

fn repository_marker(label: &str) -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::boundary(MethodLibraryTypedBoundaryRef::new(
        MethodLibraryTypedBoundaryRefKind::MethodAssetApplicationDispatch,
        format!("method-library-infra:{label}"),
    ))
}

#[derive(Clone)]
struct StoredResultLookupEntry {
    stored_result_ref: MethodAssetStoredOperationResultRef,
    operation_digest_ref: MethodAssetOperationDigestRef,
}

#[derive(Default)]
struct InMemoryDistributionHandoffState {
    relations: HashMap<String, Versioned<MethodAssetRelationReadAnchor>>,
    distributions: HashMap<String, Versioned<MethodAssetDistributionRecord>>,
    candidates: HashMap<String, Versioned<MethodAssetDistributionEventCandidateAssembly>>,
    publication_outcomes: HashMap<String, Versioned<MethodAssetPublicationOutcome>>,
    handoff_outcomes: HashMap<String, Versioned<MethodAssetHandoffOutcome>>,
    stored_results: HashMap<String, MethodAssetStoredOperationResult>,
    stored_result_lookup: HashMap<String, StoredResultLookupEntry>,
    commit_unknown_once: bool,
    publication_outcome_storage_unavailable_once: bool,
    handoff_outcome_storage_unavailable_once: bool,
}

enum StagedOperation {
    SaveDistribution {
        record: MethodAssetDistributionRecord,
        version: MethodAssetRepositoryVersion,
    },
    AppendCandidate {
        candidate: MethodAssetDistributionEventCandidateAssembly,
        version: MethodAssetRepositoryVersion,
    },
    SavePublicationOutcome {
        outcome: MethodAssetPublicationOutcome,
        version: MethodAssetRepositoryVersion,
    },
    SaveHandoffOutcome {
        outcome: MethodAssetHandoffOutcome,
        version: MethodAssetRepositoryVersion,
    },
    SaveStoredResult {
        idempotency_key_ref: MethodAssetIdempotencyKeyRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        stored_result: MethodAssetStoredOperationResult,
    },
}

struct InMemoryDistributionHandoffCommandUnitOfWork {
    state: Arc<Mutex<InMemoryDistributionHandoffState>>,
    staged: Vec<StagedOperation>,
    active: bool,
}

fn ensure_uow(
    uow: &mut dyn method_library_application::CommandUnitOfWork,
) -> Result<&mut InMemoryDistributionHandoffCommandUnitOfWork, MethodAssetRepositoryError> {
    let any = uow as &mut dyn Any;
    any.downcast_mut::<InMemoryDistributionHandoffCommandUnitOfWork>()
        .ok_or_else(|| MethodAssetRepositoryError::TransactionNotActive {
            failure_marker_ref: repository_marker("wrong-uow-type"),
        })
}

impl method_library_application::CommandUnitOfWork
    for InMemoryDistributionHandoffCommandUnitOfWork
{
    fn commit(&mut self) -> Result<MethodAssetCommitObservation, ()> {
        if !self.active {
            return Err(());
        }
        let mut state = self.state.lock().expect("in-memory state lock poisoned");
        for operation in self.staged.drain(..) {
            match operation {
                StagedOperation::SaveDistribution { record, version } => {
                    state.distributions.insert(
                        record.distribution_ref.as_public_ref().to_owned(),
                        Versioned {
                            value: record,
                            version,
                        },
                    );
                }
                StagedOperation::AppendCandidate { candidate, version } => {
                    state.candidates.insert(
                        candidate.assembly_ref.as_public_ref().to_owned(),
                        Versioned {
                            value: candidate,
                            version,
                        },
                    );
                }
                StagedOperation::SavePublicationOutcome { outcome, version } => {
                    state.publication_outcomes.insert(
                        outcome.publication_ref().as_public_ref().to_owned(),
                        Versioned {
                            value: outcome,
                            version,
                        },
                    );
                }
                StagedOperation::SaveHandoffOutcome { outcome, version } => {
                    state.handoff_outcomes.insert(
                        outcome.handoff_ref().as_public_ref().to_owned(),
                        Versioned {
                            value: outcome,
                            version,
                        },
                    );
                }
                StagedOperation::SaveStoredResult {
                    idempotency_key_ref,
                    dedup_scope_ref,
                    operation_digest_ref,
                    stored_result,
                } => {
                    let lookup_key = format!(
                        "{}|{}",
                        idempotency_key_ref.as_public_ref(),
                        dedup_scope_ref.as_public_ref()
                    );
                    state.stored_result_lookup.insert(
                        lookup_key,
                        StoredResultLookupEntry {
                            stored_result_ref: stored_result.stored_result_ref.clone(),
                            operation_digest_ref,
                        },
                    );
                    state.stored_results.insert(
                        stored_result.stored_result_ref.as_public_ref().to_owned(),
                        stored_result,
                    );
                }
            }
        }
        self.active = false;
        if state.commit_unknown_once {
            state.commit_unknown_once = false;
            Ok(MethodAssetCommitObservation::CommitUnknown {
                unknown_marker_ref: repository_marker("commit-unknown"),
            })
        } else {
            Ok(MethodAssetCommitObservation::Committed)
        }
    }

    fn rollback(&mut self) -> Result<(), ()> {
        if !self.active {
            return Err(());
        }
        self.staged.clear();
        self.active = false;
        Ok(())
    }
}

/// Command/outcome UoW factory backed by one shared in-memory state.
pub struct InMemoryDistributionHandoffUnitOfWorkFactory {
    state: Arc<Mutex<InMemoryDistributionHandoffState>>,
}

impl UnitOfWork for InMemoryDistributionHandoffUnitOfWorkFactory {
    fn begin_command_uow(&self) -> Box<dyn method_library_application::CommandUnitOfWork> {
        Box::new(InMemoryDistributionHandoffCommandUnitOfWork {
            state: Arc::clone(&self.state),
            staged: Vec::new(),
            active: true,
        })
    }
}

/// Read-only in-memory relation-anchor repository.
pub struct InMemoryMethodAssetRelationRepository {
    state: Arc<Mutex<InMemoryDistributionHandoffState>>,
}

impl MethodAssetRelationRepository for InMemoryMethodAssetRelationRepository {
    fn get_relation_anchor_with_version(
        &self,
        relation_ref: MethodAssetRelationRef,
    ) -> Result<Option<Versioned<MethodAssetRelationReadAnchor>>, MethodAssetRepositoryError> {
        Ok(self
            .state
            .lock()
            .expect("in-memory state lock poisoned")
            .relations
            .get(relation_ref.as_public_ref())
            .cloned())
    }
}

/// Versioned distribution-record repository with staged-write semantics.
pub struct InMemoryMethodAssetDistributionRepository {
    state: Arc<Mutex<InMemoryDistributionHandoffState>>,
}

impl MethodAssetDistributionRepository for InMemoryMethodAssetDistributionRepository {
    fn get_distribution_with_version(
        &self,
        distribution_ref: MethodAssetDistributionRef,
    ) -> Result<Option<Versioned<MethodAssetDistributionRecord>>, MethodAssetRepositoryError> {
        Ok(self
            .state
            .lock()
            .expect("in-memory state lock poisoned")
            .distributions
            .get(distribution_ref.as_public_ref())
            .cloned())
    }

    fn save_distribution(
        &self,
        record: MethodAssetDistributionRecord,
        expected_version: Option<MethodAssetExpectedVersion>,
        uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<VersionedRef<MethodAssetDistributionRef>, MethodAssetRepositoryError> {
        let uow = ensure_uow(uow)?;
        if !uow.active {
            return Err(MethodAssetRepositoryError::TransactionNotActive {
                failure_marker_ref: repository_marker("distribution-save-inactive"),
            });
        }
        let current = self
            .state
            .lock()
            .expect("in-memory state lock poisoned")
            .distributions
            .get(record.distribution_ref.as_public_ref())
            .cloned();
        let actual_version = current
            .as_ref()
            .map(|value| value.version)
            .unwrap_or(MethodAssetRepositoryVersion(0));
        let version_matches = match (current.as_ref(), expected_version) {
            (None, None) => true,
            (Some(current), Some(expected)) => current.version == expected.0,
            _ => false,
        };
        if !version_matches {
            return Err(MethodAssetRepositoryError::VersionConflict {
                expected_version,
                actual_version,
                conflict_marker_ref: repository_marker("distribution-version-conflict"),
            });
        }
        let next_version = MethodAssetRepositoryVersion(actual_version.0 + 1);
        let value_ref = record.distribution_ref.clone();
        uow.staged.push(StagedOperation::SaveDistribution {
            record,
            version: next_version,
        });
        Ok(VersionedRef {
            value_ref,
            version: next_version,
        })
    }
}

/// Append-only in-memory candidate repository.
pub struct InMemoryMethodAssetEventCandidateAssemblyRepository {
    state: Arc<Mutex<InMemoryDistributionHandoffState>>,
}

impl MethodAssetEventCandidateAssemblyRepository
    for InMemoryMethodAssetEventCandidateAssemblyRepository
{
    fn append_event_candidate_assembly(
        &self,
        assembly: MethodAssetDistributionEventCandidateAssembly,
        uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<VersionedRef<MethodAssetEventCandidateAssemblyRef>, MethodAssetRepositoryError>
    {
        let uow = ensure_uow(uow)?;
        if !uow.active {
            return Err(MethodAssetRepositoryError::TransactionNotActive {
                failure_marker_ref: repository_marker("candidate-append-inactive"),
            });
        }
        if self
            .state
            .lock()
            .expect("in-memory state lock poisoned")
            .candidates
            .contains_key(assembly.assembly_ref.as_public_ref())
        {
            return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                conflict_marker_ref: repository_marker("candidate-duplicate"),
            });
        }
        let value_ref = assembly.assembly_ref.clone();
        let version = MethodAssetRepositoryVersion(1);
        uow.staged.push(StagedOperation::AppendCandidate {
            candidate: assembly,
            version,
        });
        Ok(VersionedRef { value_ref, version })
    }

    fn get_event_candidate_assembly(
        &self,
        assembly_ref: MethodAssetEventCandidateAssemblyRef,
    ) -> Result<
        Option<Versioned<MethodAssetDistributionEventCandidateAssembly>>,
        MethodAssetRepositoryError,
    > {
        Ok(self
            .state
            .lock()
            .expect("in-memory state lock poisoned")
            .candidates
            .get(assembly_ref.as_public_ref())
            .cloned())
    }
}

/// Versioned publication-outcome repository.
pub struct InMemoryMethodAssetPublicationOutcomeRepository {
    state: Arc<Mutex<InMemoryDistributionHandoffState>>,
}

impl MethodAssetPublicationOutcomeRepository for InMemoryMethodAssetPublicationOutcomeRepository {
    fn save_publication_outcome(
        &self,
        outcome: MethodAssetPublicationOutcome,
        uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<VersionedRef<MethodAssetPublicationOutcomeRef>, MethodAssetRepositoryError> {
        let uow = ensure_uow(uow)?;
        if !uow.active {
            return Err(MethodAssetRepositoryError::TransactionNotActive {
                failure_marker_ref: repository_marker("publication-save-inactive"),
            });
        }
        {
            let mut state = self.state.lock().expect("in-memory state lock poisoned");
            if state.publication_outcome_storage_unavailable_once {
                state.publication_outcome_storage_unavailable_once = false;
                return Err(MethodAssetRepositoryError::StorageUnavailable {
                    unavailable_marker_ref: repository_marker("publication-save-unavailable"),
                });
            }
        }
        let value_ref = outcome.publication_ref().clone();
        if self
            .state
            .lock()
            .expect("in-memory state lock poisoned")
            .publication_outcomes
            .contains_key(value_ref.as_public_ref())
        {
            return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                conflict_marker_ref: repository_marker("publication-duplicate"),
            });
        }
        let version = MethodAssetRepositoryVersion(1);
        uow.staged
            .push(StagedOperation::SavePublicationOutcome { outcome, version });
        Ok(VersionedRef { value_ref, version })
    }

    fn get_publication_outcome(
        &self,
        publication_ref: MethodAssetPublicationOutcomeRef,
    ) -> Result<Option<Versioned<MethodAssetPublicationOutcome>>, MethodAssetRepositoryError> {
        Ok(self
            .state
            .lock()
            .expect("in-memory state lock poisoned")
            .publication_outcomes
            .get(publication_ref.as_public_ref())
            .cloned())
    }
}

/// Versioned handoff-marker repository.
pub struct InMemoryMethodAssetHandoffMarkerRepository {
    state: Arc<Mutex<InMemoryDistributionHandoffState>>,
}

impl MethodAssetHandoffMarkerRepository for InMemoryMethodAssetHandoffMarkerRepository {
    fn save_handoff_marker(
        &self,
        outcome: MethodAssetHandoffOutcome,
        uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<VersionedRef<MethodAssetHandoffMarkerRef>, MethodAssetRepositoryError> {
        let uow = ensure_uow(uow)?;
        if !uow.active {
            return Err(MethodAssetRepositoryError::TransactionNotActive {
                failure_marker_ref: repository_marker("handoff-save-inactive"),
            });
        }
        {
            let mut state = self.state.lock().expect("in-memory state lock poisoned");
            if state.handoff_outcome_storage_unavailable_once {
                state.handoff_outcome_storage_unavailable_once = false;
                return Err(MethodAssetRepositoryError::StorageUnavailable {
                    unavailable_marker_ref: repository_marker("handoff-save-unavailable"),
                });
            }
        }
        let value_ref = outcome.handoff_ref().clone();
        if self
            .state
            .lock()
            .expect("in-memory state lock poisoned")
            .handoff_outcomes
            .contains_key(value_ref.as_public_ref())
        {
            return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                conflict_marker_ref: repository_marker("handoff-duplicate"),
            });
        }
        let version = MethodAssetRepositoryVersion(1);
        uow.staged
            .push(StagedOperation::SaveHandoffOutcome { outcome, version });
        Ok(VersionedRef { value_ref, version })
    }

    fn get_handoff_marker(
        &self,
        handoff_ref: MethodAssetHandoffMarkerRef,
    ) -> Result<Option<Versioned<MethodAssetHandoffOutcome>>, MethodAssetRepositoryError> {
        Ok(self
            .state
            .lock()
            .expect("in-memory state lock poisoned")
            .handoff_outcomes
            .get(handoff_ref.as_public_ref())
            .cloned())
    }
}

/// Shared stored-result fake with replay-integrity checks.
pub struct InMemoryDistributionHandoffStoredOperationResultRepository {
    state: Arc<Mutex<InMemoryDistributionHandoffState>>,
}

impl MethodAssetStoredOperationResultRepository
    for InMemoryDistributionHandoffStoredOperationResultRepository
{
    fn find_command_result_by_idempotency(
        &self,
        idempotency_key_ref: MethodAssetIdempotencyKeyRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> Result<Option<MethodAssetStoredOperationResult>, MethodAssetRepositoryError> {
        let state = self.state.lock().expect("in-memory state lock poisoned");
        let lookup_key = format!(
            "{}|{}",
            idempotency_key_ref.as_public_ref(),
            dedup_scope_ref.as_public_ref()
        );
        let Some(entry) = state.stored_result_lookup.get(&lookup_key) else {
            return Ok(None);
        };
        let Some(result) = state
            .stored_results
            .get(entry.stored_result_ref.as_public_ref())
        else {
            return Err(MethodAssetRepositoryError::StoredResultIntegrityViolation {
                stored_result_ref: Some(entry.stored_result_ref.clone()),
                violation_marker_ref: repository_marker("stored-result-missing"),
            });
        };
        if result.operation_digest_ref != entry.operation_digest_ref {
            return Err(MethodAssetRepositoryError::StoredResultIntegrityViolation {
                stored_result_ref: Some(entry.stored_result_ref.clone()),
                violation_marker_ref: repository_marker("stored-result-digest-mismatch"),
            });
        }
        Ok(Some(result.clone()))
    }

    fn get_stored_operation_result(
        &self,
        stored_result_ref: MethodAssetStoredOperationResultRef,
    ) -> Result<Option<MethodAssetStoredOperationResult>, MethodAssetRepositoryError> {
        Ok(self
            .state
            .lock()
            .expect("in-memory state lock poisoned")
            .stored_results
            .get(stored_result_ref.as_public_ref())
            .cloned())
    }

    fn save_command_result_for_idempotency(
        &self,
        idempotency_key_ref: MethodAssetIdempotencyKeyRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        stored_result: MethodAssetStoredOperationResult,
        uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<MethodAssetStoredOperationResultRef, MethodAssetRepositoryError> {
        let uow = ensure_uow(uow)?;
        if !uow.active {
            return Err(MethodAssetRepositoryError::TransactionNotActive {
                failure_marker_ref: repository_marker("stored-result-save-inactive"),
            });
        }
        let lookup_key = format!(
            "{}|{}",
            idempotency_key_ref.as_public_ref(),
            dedup_scope_ref.as_public_ref()
        );
        if self
            .state
            .lock()
            .expect("in-memory state lock poisoned")
            .stored_result_lookup
            .contains_key(&lookup_key)
        {
            return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                conflict_marker_ref: repository_marker("stored-result-duplicate"),
            });
        }
        let value_ref = stored_result.stored_result_ref.clone();
        uow.staged.push(StagedOperation::SaveStoredResult {
            idempotency_key_ref,
            dedup_scope_ref,
            operation_digest_ref,
            stored_result,
        });
        Ok(value_ref)
    }
}

/// Configured body-free distribution builder fake.
pub struct InMemoryDistributionReadMaterialBuilderPort {
    material_summary_ref: MethodLibraryTypedBoundaryRef,
    effect_summary_ref: MethodLibraryTypedBoundaryRef,
    calls: Mutex<u64>,
}

impl InMemoryDistributionReadMaterialBuilderPort {
    fn new() -> Self {
        Self {
            material_summary_ref: MethodLibraryTypedBoundaryRef::new(
                MethodLibraryTypedBoundaryRefKind::MethodAssetDistribution,
                "fake-distribution-material-summary",
            ),
            effect_summary_ref: MethodLibraryTypedBoundaryRef::new(
                MethodLibraryTypedBoundaryRefKind::MethodAssetEffectSummary,
                "fake-distribution-effect-summary",
            ),
            calls: Mutex::new(0),
        }
    }

    pub fn call_count(&self) -> u64 {
        *self.calls.lock().expect("builder call lock poisoned")
    }
}

impl DistributionReadMaterialBuilderPort for InMemoryDistributionReadMaterialBuilderPort {
    fn build_distribution_read_material(
        &self,
        input: DistributionReadMaterialBuilderInput,
    ) -> DistributionReadMaterialBuildOutcome {
        *self.calls.lock().expect("builder call lock poisoned") += 1;
        DistributionReadMaterialBuildOutcome::Built {
            material_summary_ref: self.material_summary_ref.clone(),
            distribution_ref: input.distribution_ref,
            distribution_context_ref: input.distribution_context_ref,
            availability_marker: Some(input.availability_marker),
            effect_summary_ref: self.effect_summary_ref.clone(),
        }
    }
}

/// Copy-only availability resolver fake.
pub struct InMemoryMethodAssetConsumptionAvailabilityResolverPort;

impl MethodAssetConsumptionAvailabilityResolverPort
    for InMemoryMethodAssetConsumptionAvailabilityResolverPort
{
    fn resolve_distribution_availability(
        &self,
        marker: MethodAssetConsumptionAvailabilityMarker,
    ) -> MethodAssetConsumptionAvailabilityMarker {
        marker
    }
}

/// Mapper-owned degraded-decision fake.
pub struct InMemoryMethodAssetDegradedDecisionMapperPort;

impl MethodAssetDegradedDecisionMapperPort for InMemoryMethodAssetDegradedDecisionMapperPort {
    fn map_degraded_decision(
        &self,
        marker: MethodAssetConsumptionAvailabilityMarker,
        _diagnostic_ref: Option<MethodAssetInfraSafeDiagnosticRef>,
    ) -> Option<MethodAssetDegradedDecisionRef> {
        let _ = marker;
        None
    }
}

/// Configured exact availability-summary fake.
pub struct InMemoryMethodAssetAdapterAvailabilityPort {
    summary: Mutex<MethodAssetAdapterAvailabilitySummary>,
    calls: Mutex<u64>,
}

impl InMemoryMethodAssetAdapterAvailabilityPort {
    pub fn set_summary(&self, summary: MethodAssetAdapterAvailabilitySummary) {
        *self
            .summary
            .lock()
            .expect("availability summary lock poisoned") = summary;
    }

    pub fn call_count(&self) -> u64 {
        *self.calls.lock().expect("availability call lock poisoned")
    }
}

impl MethodAssetAdapterAvailabilityPort for InMemoryMethodAssetAdapterAvailabilityPort {
    fn check_required_distribution_slots(
        &self,
        _required_slot_refs: MethodAssetAdapterSlotRefSet,
    ) -> MethodAssetAdapterAvailabilitySummary {
        *self.calls.lock().expect("availability call lock poisoned") += 1;
        self.summary
            .lock()
            .expect("availability summary lock poisoned")
            .clone()
    }
}

/// Configured exact target-summary fake.
pub struct InMemoryMethodAssetCollaborationTargetRegistryPort {
    summary: Mutex<MethodAssetCollaborationTargetSummary>,
    calls: Mutex<u64>,
}

impl InMemoryMethodAssetCollaborationTargetRegistryPort {
    pub fn set_summary(&self, summary: MethodAssetCollaborationTargetSummary) {
        *self.summary.lock().expect("target summary lock poisoned") = summary;
    }

    pub fn call_count(&self) -> u64 {
        *self.calls.lock().expect("target call lock poisoned")
    }
}

impl MethodAssetCollaborationTargetRegistryPort
    for InMemoryMethodAssetCollaborationTargetRegistryPort
{
    fn resolve_distribution_targets(
        &self,
        _scope_ref: MethodAssetTargetRegistryScopeRef,
        _publisher_binding_ref: MethodAssetPublisherBindingStateRef,
        _handoff_binding_ref: Option<MethodAssetHandoffBindingStateRef>,
    ) -> MethodAssetCollaborationTargetSummary {
        *self.calls.lock().expect("target call lock poisoned") += 1;
        self.summary
            .lock()
            .expect("target summary lock poisoned")
            .clone()
    }
}

/// Body-free publisher fake. Optional formal reason/diagnostic config selects failure.
pub struct InMemoryMethodAssetEventCandidatePublisherPort {
    failure: Mutex<Option<(MethodLibrarySafeMarker, MethodAssetInfraSafeDiagnosticRef)>>,
    calls: Mutex<u64>,
}

impl InMemoryMethodAssetEventCandidatePublisherPort {
    pub fn set_failure(
        &self,
        failure: Option<(MethodLibrarySafeMarker, MethodAssetInfraSafeDiagnosticRef)>,
    ) {
        *self
            .failure
            .lock()
            .expect("publisher failure lock poisoned") = failure;
    }

    pub fn call_count(&self) -> u64 {
        *self.calls.lock().expect("publisher call lock poisoned")
    }
}

impl MethodAssetEventCandidatePublisherPort for InMemoryMethodAssetEventCandidatePublisherPort {
    fn publish_event_candidate(
        &self,
        input: MethodAssetEventCandidatePublicationInput,
    ) -> MethodAssetPublicationOutcome {
        *self.calls.lock().expect("publisher call lock poisoned") += 1;
        let target_ref_set = match &input.target_summary {
            MethodAssetCollaborationTargetSummary::Enabled { target_ref_set, .. }
            | MethodAssetCollaborationTargetSummary::Disabled { target_ref_set, .. }
            | MethodAssetCollaborationTargetSummary::Blocked { target_ref_set, .. }
            | MethodAssetCollaborationTargetSummary::Unavailable { target_ref_set, .. } => {
                target_ref_set.clone()
            }
        };
        let candidate_ref = input.candidate.assembly_ref;
        if let Some((reason_ref, diagnostic_ref)) = self
            .failure
            .lock()
            .expect("publisher failure lock poisoned")
            .clone()
        {
            MethodAssetPublicationOutcome::Failed {
                publication_ref: input.publication_ref,
                candidate_ref,
                target_ref_set,
                boundary_marker_ref: input.boundary_marker_ref,
                reason_ref,
                diagnostic_ref,
            }
        } else {
            MethodAssetPublicationOutcome::Published {
                publication_ref: input.publication_ref,
                candidate_ref,
                target_ref_set,
                boundary_marker_ref: input.boundary_marker_ref,
            }
        }
    }
}

/// Body-free handoff fake. Optional formal reason/diagnostic config selects failure.
pub struct InMemoryMethodAssetCollaborationHandoffPort {
    failure: Mutex<Option<(MethodLibrarySafeMarker, MethodAssetInfraSafeDiagnosticRef)>>,
    calls: Mutex<u64>,
}

impl InMemoryMethodAssetCollaborationHandoffPort {
    pub fn set_failure(
        &self,
        failure: Option<(MethodLibrarySafeMarker, MethodAssetInfraSafeDiagnosticRef)>,
    ) {
        *self.failure.lock().expect("handoff failure lock poisoned") = failure;
    }

    pub fn call_count(&self) -> u64 {
        *self.calls.lock().expect("handoff call lock poisoned")
    }
}

impl MethodAssetCollaborationHandoffPort for InMemoryMethodAssetCollaborationHandoffPort {
    fn prepare_distribution_handoff(
        &self,
        input: MethodAssetDistributionHandoffInput,
    ) -> MethodAssetHandoffOutcome {
        *self.calls.lock().expect("handoff call lock poisoned") += 1;
        if let Some((reason_ref, diagnostic_ref)) = self
            .failure
            .lock()
            .expect("handoff failure lock poisoned")
            .clone()
        {
            MethodAssetHandoffOutcome::Failed {
                handoff_ref: input.handoff_ref,
                target_ref: input.target_ref,
                boundary_marker_ref: input.boundary_marker_ref,
                reason_ref,
                diagnostic_ref,
            }
        } else {
            MethodAssetHandoffOutcome::Prepared {
                handoff_ref: input.handoff_ref,
                target_ref: input.target_ref,
                boundary_marker_ref: input.boundary_marker_ref,
                hint_ref: input.follow_up_hint_ref,
            }
        }
    }
}

/// In-memory support factory owning every new current-boundary opaque identity.
pub struct InMemoryMethodAssetDistributionHandoffSupportRefFactory {
    nonce: u64,
}

impl Default for InMemoryMethodAssetDistributionHandoffSupportRefFactory {
    fn default() -> Self {
        Self { nonce: 0 }
    }
}

impl InMemoryMethodAssetDistributionHandoffSupportRefFactory {
    fn next_opaque(&mut self, prefix: &str, canonical_input: &str) -> String {
        self.nonce = self.nonce.wrapping_add(1);
        stable_hash(&format!("{prefix}|{}|{canonical_input}", self.nonce))
    }
}

impl MethodAssetDistributionHandoffSupportRefFactory
    for InMemoryMethodAssetDistributionHandoffSupportRefFactory
{
    fn distribution_handoff_dispatch_ref(&self) -> MethodAssetApplicationDispatchRef {
        MethodAssetApplicationDispatchRef::new("distribution-handoff-command-service")
    }

    fn new_api_entry_context_ref(&mut self) -> MethodAssetApiEntryContextRef {
        MethodAssetApiEntryContextRef::new(format!(
            "api-entry:{}",
            self.next_opaque("api-entry", "distribution-handoff")
        ))
    }

    fn build_distribution_handoff_replay_envelope(
        &mut self,
        input: MethodAssetDistributionHandoffReplayEnvelopeFactoryInput,
    ) -> Result<MethodAssetDistributionHandoffReplayEnvelope, MethodAssetReplayEnvelopeBuildError>
    {
        if input.application_dispatch_ref != self.distribution_handoff_dispatch_ref() {
            return Err(
                MethodAssetReplayEnvelopeBuildError::UnsupportedDispatchTarget {
                    reason_ref: self.new_safe_reject_reason_ref(),
                },
            );
        }
        if !selector_matches_source(input.selector, &input.command_source) {
            return Err(
                MethodAssetReplayEnvelopeBuildError::SourceSelectorMismatch {
                    reason_ref: self.new_safe_reject_reason_ref(),
                },
            );
        }
        let Some(idempotency_key) = input
            .command_shell
            .metadata
            .request
            .idempotency_key
            .as_ref()
        else {
            return Err(MethodAssetReplayEnvelopeBuildError::MissingIdempotencyKey {
                reason_ref: self.new_safe_reject_reason_ref(),
            });
        };
        let operation_context_ref = MethodAssetOperationContextRef::new(format!(
            "operation-context:{}",
            self.next_opaque(
                "operation-context",
                &format!(
                    "{:?}|{}|{}|{}",
                    input.command_shell.capability_kind,
                    canonical_typed_ref(&input.command_shell.boundary_ref),
                    input.api_entry_context_ref.as_public_ref(),
                    input.application_dispatch_ref.as_public_ref(),
                )
            )
        ));
        let idempotency_key_ref = MethodAssetIdempotencyKeyRef::new(format!(
            "idempotency-key:{}",
            stable_hash(idempotency_key.as_str())
        ));
        let operation_digest_ref = MethodAssetOperationDigestRef::new(format!(
            "operation-digest:{}",
            stable_hash(&canonical_operation_digest(&input))
        ));
        let dedup_scope_ref = MethodAssetDedupScopeRef::new(format!(
            "dedup-scope:{}",
            stable_hash(&canonical_dedup_scope(
                input.selector,
                &input.command_source
            ))
        ));
        Ok(MethodAssetDistributionHandoffReplayEnvelope {
            operation_context_ref,
            idempotency_key_ref,
            operation_digest_ref,
            dedup_scope_ref,
        })
    }

    fn new_stored_operation_result_ref(&mut self) -> MethodAssetStoredOperationResultRef {
        MethodAssetStoredOperationResultRef::new(format!(
            "stored-result:{}",
            self.next_opaque("stored-result", "distribution-handoff")
        ))
    }

    fn new_accepted_operation_summary_ref(&mut self) -> MethodAssetAcceptedOperationSummaryRef {
        MethodAssetAcceptedOperationSummaryRef::new(format!(
            "accepted-summary:{}",
            self.next_opaque("accepted-summary", "distribution-handoff")
        ))
    }

    fn new_safe_reject_reason_ref(&mut self) -> MethodAssetSafeRejectReasonRef {
        MethodAssetSafeRejectReasonRef::new(format!(
            "reject-reason:{}",
            self.next_opaque("reject-reason", "distribution-handoff")
        ))
    }

    fn new_safe_ignore_reason_ref(&mut self) -> MethodAssetSafeIgnoreReasonRef {
        MethodAssetSafeIgnoreReasonRef::new(format!(
            "ignore-reason:{}",
            self.next_opaque("ignore-reason", "distribution-handoff")
        ))
    }

    fn new_effect_summary_ref(&mut self) -> MethodAssetEffectSummaryRef {
        MethodAssetEffectSummaryRef::new(format!(
            "effect-summary:{}",
            self.next_opaque("effect-summary", "distribution-handoff")
        ))
    }

    fn new_replay_marker_ref(&mut self) -> MethodAssetReplayMarkerRef {
        MethodAssetReplayMarkerRef::new(format!(
            "replay-marker:{}",
            self.next_opaque("replay-marker", "distribution-handoff")
        ))
    }

    fn new_distribution_ref(
        &mut self,
        relation_ref: MethodAssetRelationRef,
        distribution_context_ref: DistributionContextRef,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> MethodAssetDistributionRef {
        MethodAssetDistributionRef::new(format!(
            "distribution:{}",
            self.next_opaque(
                "distribution",
                &format!(
                    "{}|{}|{}|{}|{}",
                    relation_ref.as_public_ref(),
                    distribution_context_ref.as_public_ref(),
                    operation_context_ref.as_public_ref(),
                    operation_digest_ref.as_public_ref(),
                    dedup_scope_ref.as_public_ref(),
                )
            )
        ))
    }

    fn new_event_candidate_assembly_ref(
        &mut self,
        distribution_ref: MethodAssetDistributionRef,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> MethodAssetEventCandidateAssemblyRef {
        MethodAssetEventCandidateAssemblyRef::new(format!(
            "event-candidate:{}",
            self.next_opaque(
                "event-candidate",
                &format!(
                    "{}|{}|{}|{}",
                    distribution_ref.as_public_ref(),
                    operation_context_ref.as_public_ref(),
                    operation_digest_ref.as_public_ref(),
                    dedup_scope_ref.as_public_ref(),
                )
            )
        ))
    }

    fn new_publication_outcome_ref(
        &mut self,
        candidate_ref: MethodAssetEventCandidateAssemblyRef,
        target_ref_set: MethodAssetHandoffTargetRefSet,
    ) -> MethodAssetPublicationOutcomeRef {
        MethodAssetPublicationOutcomeRef::new(format!(
            "publication-outcome:{}",
            self.next_opaque(
                "publication-outcome",
                &format!(
                    "{}|{}",
                    candidate_ref.as_public_ref(),
                    canonical_target_set(&target_ref_set)
                )
            )
        ))
    }

    fn new_handoff_marker_ref(
        &mut self,
        candidate_ref: MethodAssetEventCandidateAssemblyRef,
        target_ref: MethodAssetHandoffTargetRef,
    ) -> MethodAssetHandoffMarkerRef {
        MethodAssetHandoffMarkerRef::new(format!(
            "handoff-marker:{}",
            self.next_opaque(
                "handoff-marker",
                &format!(
                    "{}|{}",
                    candidate_ref.as_public_ref(),
                    target_ref.as_public_ref()
                )
            )
        ))
    }
}

/// Complete in-memory distribution/handoff service runtime.
pub struct InMemoryMethodAssetDistributionHandoffRuntime {
    state: Arc<Mutex<InMemoryDistributionHandoffState>>,
    relation_repository: Arc<InMemoryMethodAssetRelationRepository>,
    distribution_repository: Arc<InMemoryMethodAssetDistributionRepository>,
    candidate_repository: Arc<InMemoryMethodAssetEventCandidateAssemblyRepository>,
    publication_repository: Arc<InMemoryMethodAssetPublicationOutcomeRepository>,
    handoff_repository: Arc<InMemoryMethodAssetHandoffMarkerRepository>,
    stored_result_repository: Arc<InMemoryDistributionHandoffStoredOperationResultRepository>,
    builder: Arc<InMemoryDistributionReadMaterialBuilderPort>,
    adapter_availability: Arc<InMemoryMethodAssetAdapterAvailabilityPort>,
    target_registry: Arc<InMemoryMethodAssetCollaborationTargetRegistryPort>,
    publisher: Arc<InMemoryMethodAssetEventCandidatePublisherPort>,
    handoff: Arc<InMemoryMethodAssetCollaborationHandoffPort>,
    support_ref_factory: Arc<Mutex<Box<dyn MethodAssetDistributionHandoffSupportRefFactory>>>,
    unit_of_work: Arc<InMemoryDistributionHandoffUnitOfWorkFactory>,
    facade: Arc<dyn MethodAssetDistributionHandoffCommandFacade>,
}

impl InMemoryMethodAssetDistributionHandoffRuntime {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(InMemoryDistributionHandoffState::default()));
        let relation_repository = Arc::new(InMemoryMethodAssetRelationRepository {
            state: Arc::clone(&state),
        });
        let distribution_repository = Arc::new(InMemoryMethodAssetDistributionRepository {
            state: Arc::clone(&state),
        });
        let candidate_repository = Arc::new(InMemoryMethodAssetEventCandidateAssemblyRepository {
            state: Arc::clone(&state),
        });
        let publication_repository = Arc::new(InMemoryMethodAssetPublicationOutcomeRepository {
            state: Arc::clone(&state),
        });
        let handoff_repository = Arc::new(InMemoryMethodAssetHandoffMarkerRepository {
            state: Arc::clone(&state),
        });
        let stored_result_repository =
            Arc::new(InMemoryDistributionHandoffStoredOperationResultRepository {
                state: Arc::clone(&state),
            });
        let builder = Arc::new(InMemoryDistributionReadMaterialBuilderPort::new());
        let availability_resolver =
            Arc::new(InMemoryMethodAssetConsumptionAvailabilityResolverPort);
        let degraded_mapper = Arc::new(InMemoryMethodAssetDegradedDecisionMapperPort);
        let adapter_availability = Arc::new(InMemoryMethodAssetAdapterAvailabilityPort {
            summary: Mutex::new(MethodAssetAdapterAvailabilitySummary::Available {
                availability_state_ref: MethodAssetAdapterAvailabilityStateRef::new(
                    "fake-adapter-available",
                ),
                marker_ref: repository_marker("adapter-available"),
            }),
            calls: Mutex::new(0),
        });
        let target_registry = Arc::new(InMemoryMethodAssetCollaborationTargetRegistryPort {
            summary: Mutex::new(MethodAssetCollaborationTargetSummary::Enabled {
                target_ref_set: MethodAssetHandoffTargetRefSet::from_refs([
                    MethodAssetHandoffTargetRef::new("fake-handoff-target"),
                ]),
                summary_marker_ref: repository_marker("target-enabled"),
            }),
            calls: Mutex::new(0),
        });
        let publisher = Arc::new(InMemoryMethodAssetEventCandidatePublisherPort {
            failure: Mutex::new(None),
            calls: Mutex::new(0),
        });
        let handoff = Arc::new(InMemoryMethodAssetCollaborationHandoffPort {
            failure: Mutex::new(None),
            calls: Mutex::new(0),
        });
        let support_ref_factory: Arc<
            Mutex<Box<dyn MethodAssetDistributionHandoffSupportRefFactory>>,
        > = Arc::new(Mutex::new(Box::new(
            InMemoryMethodAssetDistributionHandoffSupportRefFactory::default(),
        )));
        let unit_of_work = Arc::new(InMemoryDistributionHandoffUnitOfWorkFactory {
            state: Arc::clone(&state),
        });
        let facade: Arc<dyn MethodAssetDistributionHandoffCommandFacade> =
            Arc::new(DefaultMethodAssetDistributionHandoffCommandFacade::new(
                relation_repository.clone(),
                distribution_repository.clone(),
                builder.clone(),
                availability_resolver,
                degraded_mapper,
                adapter_availability.clone(),
                target_registry.clone(),
                publisher.clone(),
                handoff.clone(),
                candidate_repository.clone(),
                publication_repository.clone(),
                handoff_repository.clone(),
                stored_result_repository.clone(),
                unit_of_work.clone(),
                support_ref_factory.clone(),
            ));
        Self {
            state,
            relation_repository,
            distribution_repository,
            candidate_repository,
            publication_repository,
            handoff_repository,
            stored_result_repository,
            builder,
            adapter_availability,
            target_registry,
            publisher,
            handoff,
            support_ref_factory,
            unit_of_work,
            facade,
        }
    }

    pub fn facade(&self) -> Arc<dyn MethodAssetDistributionHandoffCommandFacade> {
        Arc::clone(&self.facade)
    }

    pub fn support_ref_factory(
        &self,
    ) -> Arc<Mutex<Box<dyn MethodAssetDistributionHandoffSupportRefFactory>>> {
        Arc::clone(&self.support_ref_factory)
    }

    pub fn relation_repository(&self) -> Arc<InMemoryMethodAssetRelationRepository> {
        Arc::clone(&self.relation_repository)
    }

    pub fn distribution_repository(&self) -> Arc<InMemoryMethodAssetDistributionRepository> {
        Arc::clone(&self.distribution_repository)
    }

    pub fn candidate_repository(&self) -> Arc<InMemoryMethodAssetEventCandidateAssemblyRepository> {
        Arc::clone(&self.candidate_repository)
    }

    pub fn publication_repository(&self) -> Arc<InMemoryMethodAssetPublicationOutcomeRepository> {
        Arc::clone(&self.publication_repository)
    }

    pub fn handoff_repository(&self) -> Arc<InMemoryMethodAssetHandoffMarkerRepository> {
        Arc::clone(&self.handoff_repository)
    }

    pub fn stored_result_repository(
        &self,
    ) -> Arc<InMemoryDistributionHandoffStoredOperationResultRepository> {
        Arc::clone(&self.stored_result_repository)
    }

    pub fn builder(&self) -> Arc<InMemoryDistributionReadMaterialBuilderPort> {
        Arc::clone(&self.builder)
    }

    pub fn adapter_availability(&self) -> Arc<InMemoryMethodAssetAdapterAvailabilityPort> {
        Arc::clone(&self.adapter_availability)
    }

    pub fn target_registry(&self) -> Arc<InMemoryMethodAssetCollaborationTargetRegistryPort> {
        Arc::clone(&self.target_registry)
    }

    pub fn publisher(&self) -> Arc<InMemoryMethodAssetEventCandidatePublisherPort> {
        Arc::clone(&self.publisher)
    }

    pub fn handoff(&self) -> Arc<InMemoryMethodAssetCollaborationHandoffPort> {
        Arc::clone(&self.handoff)
    }

    pub fn unit_of_work(&self) -> Arc<InMemoryDistributionHandoffUnitOfWorkFactory> {
        Arc::clone(&self.unit_of_work)
    }

    pub fn seed_relation_anchor(&self, anchor: MethodAssetRelationReadAnchor) {
        self.state
            .lock()
            .expect("in-memory state lock poisoned")
            .relations
            .insert(
                anchor.relation_ref.as_public_ref().to_owned(),
                Versioned {
                    value: anchor,
                    version: MethodAssetRepositoryVersion(1),
                },
            );
    }

    pub fn simulate_commit_unknown_once(&self) {
        self.state
            .lock()
            .expect("in-memory state lock poisoned")
            .commit_unknown_once = true;
    }

    /// Makes the next publication-outcome save return the formal unavailable error.
    pub fn fail_next_publication_outcome_save(&self) {
        self.state
            .lock()
            .expect("in-memory state lock poisoned")
            .publication_outcome_storage_unavailable_once = true;
    }

    /// Makes the next handoff-marker save return the formal unavailable error.
    pub fn fail_next_handoff_outcome_save(&self) {
        self.state
            .lock()
            .expect("in-memory state lock poisoned")
            .handoff_outcome_storage_unavailable_once = true;
    }

    pub fn publication_outcome_count(&self) -> usize {
        self.state
            .lock()
            .expect("in-memory state lock poisoned")
            .publication_outcomes
            .len()
    }

    pub fn handoff_outcome_count(&self) -> usize {
        self.state
            .lock()
            .expect("in-memory state lock poisoned")
            .handoff_outcomes
            .len()
    }
}

impl Default for InMemoryMethodAssetDistributionHandoffRuntime {
    fn default() -> Self {
        Self::new()
    }
}
