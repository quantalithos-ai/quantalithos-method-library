//! In-memory fake/runtime support for the `commit-04-b` formalization/version slice.

use core::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use method_library_application::ports::{
    FormalMethodAssetVersionRepository, FormalizationBasisResolverPort,
    FormalizationBasisSummaryRepository, FormalizationStateRepository,
    MethodAssetCatalogEntryRepository, MethodAssetDefinitionRepository,
    MethodAssetPolicyDiagnosticBuilderPort, MethodAssetStoredOperationResultRepository,
};
use method_library_application::{
    DefaultMethodAssetFormalizationVersionCommandFacade, FormalizationBasisResolution,
    FormalizationBasisResolutionInput, FormalizationEligibilityDiagnostic,
    MethodAssetCommitObservation, MethodAssetExpectedVersion,
    MethodAssetFormalizationVersionCommandFacade, MethodAssetFormalizationVersionCommandSelector,
    MethodAssetFormalizationVersionCommandSource, MethodAssetFormalizationVersionReplayEnvelope,
    MethodAssetFormalizationVersionReplayEnvelopeFactoryInput,
    MethodAssetFormalizationVersionSupportRefFactory, MethodAssetRepositoryError,
    MethodAssetRepositoryVersion, MethodAssetStoredOperationResult, UnitOfWork, Versioned,
    VersionedRef,
};
use method_library_contracts::{
    FormalMethodAssetVersionRef, FormalVersionBoundarySummary, FormalizationBasisSummaryRef,
    FormalizationBasisSummaryRefSet, FormalizationEligibilityRuleRef, FormalizationStateKind,
    FormalizationStateRef, GovernanceBasisRef, MethodAssetAcceptedOperationSummaryRef,
    MethodAssetApiEntryContextRef, MethodAssetApplicationDispatchRef, MethodAssetCatalogEntryRef,
    MethodAssetDedupScopeRef, MethodAssetDefinitionRef, MethodAssetEffectSummaryRef,
    MethodAssetIdempotencyKeyRef, MethodAssetIdentityKey, MethodAssetOperationContextRef,
    MethodAssetOperationDigestRef, MethodAssetReplayMarkerRef, MethodAssetSafeIgnoreReasonRef,
    MethodAssetSafeRejectReasonRef, MethodAssetStoredOperationResultRef, MethodLibrarySafeMarker,
    MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};
use method_library_domain::{
    FormalMethodAssetVersion, FormalizationBasisSummary, FormalizationState,
    MethodAssetCatalogEntry, MethodAssetDefinition,
};

fn stable_hash(input: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn canonical_ref(kind: MethodLibraryTypedBoundaryRefKind, public_ref: &str) -> String {
    format!("{kind:?}:{public_ref}")
}

fn canonical_typed_boundary_ref(boundary_ref: &MethodLibraryTypedBoundaryRef) -> String {
    canonical_ref(boundary_ref.kind(), boundary_ref.as_public_ref())
}

fn canonical_safe_marker(marker: &MethodLibrarySafeMarker) -> String {
    format!(
        "{:?}:{}",
        marker.marker_kind(),
        canonical_typed_boundary_ref(&marker.source_ref)
    )
}

fn canonical_basis_summary_ref_set(refs: &FormalizationBasisSummaryRefSet) -> String {
    refs.refs
        .iter()
        .map(|item| canonical_typed_boundary_ref(item.as_typed_ref()))
        .collect::<Vec<_>>()
        .join(",")
}

fn canonical_version_boundary_summary(summary: &FormalVersionBoundarySummary) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        canonical_safe_marker(&summary.boundary_marker_ref),
        summary.definition_ref.as_public_ref(),
        summary.catalog_entry_ref.as_public_ref(),
        summary.formalization_state_ref.as_public_ref(),
        canonical_basis_summary_ref_set(&summary.basis_summary_refs),
    )
}

fn canonical_command_source(source: &MethodAssetFormalizationVersionCommandSource) -> String {
    match source {
        MethodAssetFormalizationVersionCommandSource::EvaluateFormalizationEligibility(source) => {
            format!(
                "evaluate|{}|{}|{}|{}",
                source.definition_ref.as_public_ref(),
                source.catalog_entry_ref.as_public_ref(),
                canonical_basis_summary_ref_set(&source.basis_summary_refs),
                source.eligibility_rule_ref.as_public_ref(),
            )
        }
        MethodAssetFormalizationVersionCommandSource::InitiateFormalization(source) => format!(
            "initiate|{}|{}|{}|{}",
            source.definition_ref.as_public_ref(),
            source.catalog_entry_ref.as_public_ref(),
            canonical_safe_marker(&source.trigger_marker_ref),
            canonical_basis_summary_ref_set(&source.basis_summary_refs),
        ),
        MethodAssetFormalizationVersionCommandSource::EstablishFormalVersion(source) => format!(
            "establish|{}|{}|{}|{}",
            source.formalization_state_ref.as_public_ref(),
            source.definition_ref.as_public_ref(),
            source.catalog_entry_ref.as_public_ref(),
            canonical_version_boundary_summary(&source.version_boundary_summary),
        ),
        MethodAssetFormalizationVersionCommandSource::RecordFormalVersionSemanticChange(source) => {
            format!(
                "record_semantic_change|{}|{}|{}|{}",
                source.formal_version_ref.as_public_ref(),
                canonical_safe_marker(&source.semantic_change_marker_ref),
                canonical_basis_summary_ref_set(&source.basis_summary_refs),
                source
                    .governance_basis_ref
                    .as_ref()
                    .map(|value| value.as_public_ref().to_owned())
                    .unwrap_or_else(|| "-".to_owned()),
            )
        }
        MethodAssetFormalizationVersionCommandSource::SupersedeFormalVersion(source) => format!(
            "supersede|{}|{}|{}",
            source.previous_formal_version_ref.as_public_ref(),
            source.next_formal_version_ref.as_public_ref(),
            canonical_safe_marker(&source.supersession_marker_ref),
        ),
        MethodAssetFormalizationVersionCommandSource::RetireFormalVersion(source) => format!(
            "retire|{}|{}",
            source.formal_version_ref.as_public_ref(),
            canonical_safe_marker(&source.retirement_marker_ref),
        ),
    }
}

fn canonical_selector(selector: MethodAssetFormalizationVersionCommandSelector) -> &'static str {
    match selector {
        MethodAssetFormalizationVersionCommandSelector::EvaluateFormalizationEligibility => {
            "evaluate_formalization_eligibility"
        }
        MethodAssetFormalizationVersionCommandSelector::InitiateFormalization => {
            "initiate_formalization"
        }
        MethodAssetFormalizationVersionCommandSelector::EstablishFormalVersion => {
            "establish_formal_version"
        }
        MethodAssetFormalizationVersionCommandSelector::RecordFormalVersionSemanticChange => {
            "record_formal_version_semantic_change"
        }
        MethodAssetFormalizationVersionCommandSelector::SupersedeFormalVersion => {
            "supersede_formal_version"
        }
        MethodAssetFormalizationVersionCommandSelector::RetireFormalVersion => {
            "retire_formal_version"
        }
    }
}

fn selector_matches_source(
    selector: MethodAssetFormalizationVersionCommandSelector,
    source: &MethodAssetFormalizationVersionCommandSource,
) -> bool {
    matches!(
        (selector, source),
        (
            MethodAssetFormalizationVersionCommandSelector::EvaluateFormalizationEligibility,
            MethodAssetFormalizationVersionCommandSource::EvaluateFormalizationEligibility(_),
        ) | (
            MethodAssetFormalizationVersionCommandSelector::InitiateFormalization,
            MethodAssetFormalizationVersionCommandSource::InitiateFormalization(_),
        ) | (
            MethodAssetFormalizationVersionCommandSelector::EstablishFormalVersion,
            MethodAssetFormalizationVersionCommandSource::EstablishFormalVersion(_),
        ) | (
            MethodAssetFormalizationVersionCommandSelector::RecordFormalVersionSemanticChange,
            MethodAssetFormalizationVersionCommandSource::RecordFormalVersionSemanticChange(_),
        ) | (
            MethodAssetFormalizationVersionCommandSelector::SupersedeFormalVersion,
            MethodAssetFormalizationVersionCommandSource::SupersedeFormalVersion(_),
        ) | (
            MethodAssetFormalizationVersionCommandSelector::RetireFormalVersion,
            MethodAssetFormalizationVersionCommandSource::RetireFormalVersion(_),
        )
    )
}

fn canonical_dedup_scope(
    selector: MethodAssetFormalizationVersionCommandSelector,
    source: &MethodAssetFormalizationVersionCommandSource,
) -> String {
    match (selector, source) {
        (
            MethodAssetFormalizationVersionCommandSelector::EvaluateFormalizationEligibility,
            MethodAssetFormalizationVersionCommandSource::EvaluateFormalizationEligibility(source),
        ) => format!(
            "formalization_version|{}|definition|{}|catalog|{}",
            canonical_selector(selector),
            source.definition_ref.as_public_ref(),
            source.catalog_entry_ref.as_public_ref(),
        ),
        (
            MethodAssetFormalizationVersionCommandSelector::InitiateFormalization,
            MethodAssetFormalizationVersionCommandSource::InitiateFormalization(source),
        ) => format!(
            "formalization_version|{}|definition|{}|catalog|{}",
            canonical_selector(selector),
            source.definition_ref.as_public_ref(),
            source.catalog_entry_ref.as_public_ref(),
        ),
        (
            MethodAssetFormalizationVersionCommandSelector::EstablishFormalVersion,
            MethodAssetFormalizationVersionCommandSource::EstablishFormalVersion(source),
        ) => format!(
            "formalization_version|{}|state|{}",
            canonical_selector(selector),
            source.formalization_state_ref.as_public_ref(),
        ),
        (
            MethodAssetFormalizationVersionCommandSelector::RecordFormalVersionSemanticChange,
            MethodAssetFormalizationVersionCommandSource::RecordFormalVersionSemanticChange(source),
        ) => format!(
            "formalization_version|{}|version|{}",
            canonical_selector(selector),
            source.formal_version_ref.as_public_ref(),
        ),
        (
            MethodAssetFormalizationVersionCommandSelector::RetireFormalVersion,
            MethodAssetFormalizationVersionCommandSource::RetireFormalVersion(source),
        ) => format!(
            "formalization_version|{}|version|{}",
            canonical_selector(selector),
            source.formal_version_ref.as_public_ref(),
        ),
        (
            MethodAssetFormalizationVersionCommandSelector::SupersedeFormalVersion,
            MethodAssetFormalizationVersionCommandSource::SupersedeFormalVersion(source),
        ) => format!(
            "formalization_version|{}|previous|{}|next|{}",
            canonical_selector(selector),
            source.previous_formal_version_ref.as_public_ref(),
            source.next_formal_version_ref.as_public_ref(),
        ),
        _ => "formalization_version|mismatch".to_owned(),
    }
}

fn canonical_operation_digest(
    input: &MethodAssetFormalizationVersionReplayEnvelopeFactoryInput,
) -> String {
    let typed_refs = input
        .command_shell
        .typed_refs
        .iter()
        .map(canonical_typed_boundary_ref)
        .collect::<Vec<_>>()
        .join(",");
    let safe_markers = input
        .command_shell
        .safe_markers
        .iter()
        .map(canonical_safe_marker)
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{:?}|{:?}|{}|{}|{}|{}",
        input.command_shell.capability_kind,
        input.command_shell.boundary_ref.kind(),
        canonical_selector(input.selector),
        canonical_command_source(&input.command_source),
        typed_refs,
        safe_markers,
    )
}

fn canonical_identity_key(identity_key: &MethodAssetIdentityKey) -> String {
    format!(
        "{:?}|{}|{}|{}",
        identity_key.definition_kind,
        canonical_typed_boundary_ref(&identity_key.identity_namespace_ref),
        canonical_typed_boundary_ref(&identity_key.identity_anchor_ref),
        identity_key.applicability_scope_ref.as_public_ref(),
    )
}

fn repository_marker(label: &str) -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::boundary(MethodLibraryTypedBoundaryRef::from_verified_source(
        MethodLibraryTypedBoundaryRefKind::MethodAssetApplicationDispatch,
        format!("method-library-infra:{label}"),
    ))
}

fn state_key(
    definition_ref: &MethodAssetDefinitionRef,
    catalog_entry_ref: &MethodAssetCatalogEntryRef,
) -> String {
    format!(
        "{}|{}",
        definition_ref.as_public_ref(),
        catalog_entry_ref.as_public_ref()
    )
}

#[derive(Clone)]
struct StoredResultLookupEntry {
    stored_result_ref: MethodAssetStoredOperationResultRef,
    operation_digest_ref: MethodAssetOperationDigestRef,
}

#[derive(Default)]
struct InMemoryFormalizationVersionState {
    definitions: HashMap<String, Versioned<MethodAssetDefinition>>,
    definition_identity_index: HashMap<String, MethodAssetDefinitionRef>,
    catalog_entries: HashMap<String, Versioned<MethodAssetCatalogEntry>>,
    catalog_scope_index: HashMap<String, MethodAssetCatalogEntryRef>,
    formalization_states: HashMap<String, Versioned<FormalizationState>>,
    formalization_state_index: HashMap<String, FormalizationStateRef>,
    formal_method_versions: HashMap<String, Versioned<FormalMethodAssetVersion>>,
    current_formal_version_index: HashMap<String, FormalMethodAssetVersionRef>,
    basis_summaries: HashMap<String, Versioned<FormalizationBasisSummary>>,
    stored_results: HashMap<String, MethodAssetStoredOperationResult>,
    stored_result_lookup: HashMap<String, StoredResultLookupEntry>,
    commit_unknown_once: bool,
}

enum StagedOperation {
    SaveDefinition {
        definition: MethodAssetDefinition,
        version: MethodAssetRepositoryVersion,
    },
    SaveCatalogEntry {
        catalog_entry: MethodAssetCatalogEntry,
        version: MethodAssetRepositoryVersion,
    },
    SaveFormalizationState {
        formalization_state: FormalizationState,
        version: MethodAssetRepositoryVersion,
    },
    SaveFormalMethodVersion {
        formal_method_version: FormalMethodAssetVersion,
        version: MethodAssetRepositoryVersion,
    },
    SaveStoredResult {
        idempotency_key_ref: MethodAssetIdempotencyKeyRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        stored_result: MethodAssetStoredOperationResult,
    },
}

struct InMemoryCommandUnitOfWork {
    state: Arc<Mutex<InMemoryFormalizationVersionState>>,
    staged_operations: Vec<StagedOperation>,
    active: bool,
}

fn ensure_in_memory_uow(
    uow: &mut dyn method_library_application::CommandUnitOfWork,
) -> Result<&mut InMemoryCommandUnitOfWork, MethodAssetRepositoryError> {
    let any = uow as &mut dyn Any;
    any.downcast_mut::<InMemoryCommandUnitOfWork>()
        .ok_or_else(|| MethodAssetRepositoryError::TransactionNotActive {
            failure_marker_ref: repository_marker("wrong-uow-type"),
        })
}

impl method_library_application::CommandUnitOfWork for InMemoryCommandUnitOfWork {
    fn commit(&mut self) -> Result<MethodAssetCommitObservation, ()> {
        if !self.active {
            return Err(());
        }

        let mut state = self.state.lock().expect("in-memory state lock poisoned");
        for operation in self.staged_operations.drain(..) {
            match operation {
                StagedOperation::SaveDefinition {
                    definition,
                    version,
                } => {
                    state.definition_identity_index.insert(
                        canonical_identity_key(&definition.identity_key),
                        definition.definition_ref.clone(),
                    );
                    state.definitions.insert(
                        definition.definition_ref.as_public_ref().to_owned(),
                        Versioned {
                            value: definition,
                            version,
                        },
                    );
                }
                StagedOperation::SaveCatalogEntry {
                    catalog_entry,
                    version,
                } => {
                    let scope_key = format!(
                        "{}|{}",
                        catalog_entry.definition_ref.as_public_ref(),
                        catalog_entry.catalog_scope_ref.as_public_ref(),
                    );
                    state
                        .catalog_scope_index
                        .insert(scope_key, catalog_entry.catalog_entry_ref.clone());
                    state.catalog_entries.insert(
                        catalog_entry.catalog_entry_ref.as_public_ref().to_owned(),
                        Versioned {
                            value: catalog_entry,
                            version,
                        },
                    );
                }
                StagedOperation::SaveFormalizationState {
                    formalization_state,
                    version,
                } => {
                    state.formalization_state_index.insert(
                        state_key(
                            &formalization_state.definition_ref,
                            &formalization_state.catalog_entry_ref,
                        ),
                        formalization_state.formalization_state_ref.clone(),
                    );
                    state.formalization_states.insert(
                        formalization_state
                            .formalization_state_ref
                            .as_public_ref()
                            .to_owned(),
                        Versioned {
                            value: formalization_state,
                            version,
                        },
                    );
                }
                StagedOperation::SaveFormalMethodVersion {
                    formal_method_version,
                    version,
                } => {
                    let current_key = formal_method_version
                        .formalization_state_ref
                        .as_public_ref()
                        .to_owned();
                    if formal_method_version
                        .is_current_for(&formal_method_version.formalization_state_ref)
                    {
                        state.current_formal_version_index.insert(
                            current_key,
                            formal_method_version.formal_version_ref.clone(),
                        );
                    } else if state
                        .current_formal_version_index
                        .get(&current_key)
                        .is_some_and(|value| value == &formal_method_version.formal_version_ref)
                    {
                        state.current_formal_version_index.remove(&current_key);
                    }
                    state.formal_method_versions.insert(
                        formal_method_version
                            .formal_version_ref
                            .as_public_ref()
                            .to_owned(),
                        Versioned {
                            value: formal_method_version,
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
                        dedup_scope_ref.as_public_ref(),
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

        self.staged_operations.clear();
        self.active = false;
        Ok(())
    }
}

/// Unit-of-work factory for the in-memory fake runtime.
pub struct InMemoryFormalizationVersionUnitOfWorkFactory {
    state: Arc<Mutex<InMemoryFormalizationVersionState>>,
}

impl InMemoryFormalizationVersionUnitOfWorkFactory {
    fn new(state: Arc<Mutex<InMemoryFormalizationVersionState>>) -> Self {
        Self { state }
    }
}

impl UnitOfWork for InMemoryFormalizationVersionUnitOfWorkFactory {
    fn begin_command_uow(&self) -> Box<dyn method_library_application::CommandUnitOfWork> {
        Box::new(InMemoryCommandUnitOfWork {
            state: Arc::clone(&self.state),
            staged_operations: Vec::new(),
            active: true,
        })
    }
}

/// In-memory definition repository with versioned truth semantics.
pub struct InMemoryMethodAssetDefinitionRepository {
    state: Arc<Mutex<InMemoryFormalizationVersionState>>,
}

impl InMemoryMethodAssetDefinitionRepository {
    fn new(state: Arc<Mutex<InMemoryFormalizationVersionState>>) -> Self {
        Self { state }
    }
}

impl MethodAssetDefinitionRepository for InMemoryMethodAssetDefinitionRepository {
    fn get_definition_with_version(
        &self,
        definition_ref: MethodAssetDefinitionRef,
    ) -> Result<Option<Versioned<MethodAssetDefinition>>, MethodAssetRepositoryError> {
        let state = self.state.lock().expect("in-memory state lock poisoned");
        Ok(state
            .definitions
            .get(definition_ref.as_public_ref())
            .cloned())
    }

    fn find_definition_by_identity_key(
        &self,
        identity_key: MethodAssetIdentityKey,
    ) -> Result<Option<Versioned<MethodAssetDefinition>>, MethodAssetRepositoryError> {
        let state = self.state.lock().expect("in-memory state lock poisoned");
        let identity_key = canonical_identity_key(&identity_key);
        let Some(definition_ref) = state.definition_identity_index.get(&identity_key) else {
            return Ok(None);
        };

        Ok(state
            .definitions
            .get(definition_ref.as_public_ref())
            .cloned())
    }

    fn save_definition(
        &self,
        definition: MethodAssetDefinition,
        expected_version: Option<MethodAssetExpectedVersion>,
        uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<VersionedRef<MethodAssetDefinitionRef>, MethodAssetRepositoryError> {
        let uow = ensure_in_memory_uow(uow)?;
        if !uow.active {
            return Err(MethodAssetRepositoryError::TransactionNotActive {
                failure_marker_ref: repository_marker("definition-save-inactive"),
            });
        }

        let current = {
            let state = self.state.lock().expect("in-memory state lock poisoned");
            state
                .definitions
                .get(definition.definition_ref.as_public_ref())
                .cloned()
        };

        let version = match (current, expected_version) {
            (None, None) => {
                let state = self.state.lock().expect("in-memory state lock poisoned");
                let identity_key = canonical_identity_key(&definition.identity_key);
                if state.definition_identity_index.contains_key(&identity_key) {
                    return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                        conflict_marker_ref: repository_marker("definition-duplicate-identity"),
                    });
                }

                MethodAssetRepositoryVersion(1)
            }
            (Some(current), Some(expected_version)) if current.version == expected_version.0 => {
                MethodAssetRepositoryVersion(current.version.0 + 1)
            }
            (Some(current), Some(expected_version)) => {
                return Err(MethodAssetRepositoryError::VersionConflict {
                    expected_version: Some(expected_version),
                    actual_version: current.version,
                    conflict_marker_ref: repository_marker("definition-version-conflict"),
                });
            }
            _ => {
                return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                    conflict_marker_ref: repository_marker("definition-create-conflict"),
                });
            }
        };

        let definition_ref = definition.definition_ref.clone();
        uow.staged_operations.push(StagedOperation::SaveDefinition {
            definition,
            version,
        });

        Ok(VersionedRef {
            value_ref: definition_ref,
            version,
        })
    }
}

/// In-memory catalog repository with versioned truth semantics.
pub struct InMemoryMethodAssetCatalogEntryRepository {
    state: Arc<Mutex<InMemoryFormalizationVersionState>>,
}

impl InMemoryMethodAssetCatalogEntryRepository {
    fn new(state: Arc<Mutex<InMemoryFormalizationVersionState>>) -> Self {
        Self { state }
    }
}

impl MethodAssetCatalogEntryRepository for InMemoryMethodAssetCatalogEntryRepository {
    fn get_catalog_entry_with_version(
        &self,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
    ) -> Result<Option<Versioned<MethodAssetCatalogEntry>>, MethodAssetRepositoryError> {
        let state = self.state.lock().expect("in-memory state lock poisoned");
        Ok(state
            .catalog_entries
            .get(catalog_entry_ref.as_public_ref())
            .cloned())
    }

    fn find_catalog_entry_by_definition_scope(
        &self,
        definition_ref: MethodAssetDefinitionRef,
        catalog_scope_ref: method_library_contracts::CatalogScopeRef,
    ) -> Result<Option<Versioned<MethodAssetCatalogEntry>>, MethodAssetRepositoryError> {
        let state = self.state.lock().expect("in-memory state lock poisoned");
        let scope_key = format!(
            "{}|{}",
            definition_ref.as_public_ref(),
            catalog_scope_ref.as_public_ref()
        );
        let Some(catalog_entry_ref) = state.catalog_scope_index.get(&scope_key) else {
            return Ok(None);
        };

        Ok(state
            .catalog_entries
            .get(catalog_entry_ref.as_public_ref())
            .cloned())
    }

    fn save_catalog_entry(
        &self,
        catalog_entry: MethodAssetCatalogEntry,
        expected_version: Option<MethodAssetExpectedVersion>,
        uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<VersionedRef<MethodAssetCatalogEntryRef>, MethodAssetRepositoryError> {
        let uow = ensure_in_memory_uow(uow)?;
        if !uow.active {
            return Err(MethodAssetRepositoryError::TransactionNotActive {
                failure_marker_ref: repository_marker("catalog-save-inactive"),
            });
        }

        let current = {
            let state = self.state.lock().expect("in-memory state lock poisoned");
            state
                .catalog_entries
                .get(catalog_entry.catalog_entry_ref.as_public_ref())
                .cloned()
        };

        let version = match (current, expected_version) {
            (None, None) => {
                let state = self.state.lock().expect("in-memory state lock poisoned");
                let scope_key = format!(
                    "{}|{}",
                    catalog_entry.definition_ref.as_public_ref(),
                    catalog_entry.catalog_scope_ref.as_public_ref()
                );
                if state.catalog_scope_index.contains_key(&scope_key) {
                    return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                        conflict_marker_ref: repository_marker("catalog-duplicate-scope"),
                    });
                }

                MethodAssetRepositoryVersion(1)
            }
            (Some(current), Some(expected_version)) if current.version == expected_version.0 => {
                MethodAssetRepositoryVersion(current.version.0 + 1)
            }
            (Some(current), Some(expected_version)) => {
                return Err(MethodAssetRepositoryError::VersionConflict {
                    expected_version: Some(expected_version),
                    actual_version: current.version,
                    conflict_marker_ref: repository_marker("catalog-version-conflict"),
                });
            }
            _ => {
                return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                    conflict_marker_ref: repository_marker("catalog-create-conflict"),
                });
            }
        };

        let catalog_entry_ref = catalog_entry.catalog_entry_ref.clone();
        uow.staged_operations
            .push(StagedOperation::SaveCatalogEntry {
                catalog_entry,
                version,
            });

        Ok(VersionedRef {
            value_ref: catalog_entry_ref,
            version,
        })
    }
}

/// In-memory formalization-state repository with versioned truth semantics.
pub struct InMemoryFormalizationStateRepository {
    state: Arc<Mutex<InMemoryFormalizationVersionState>>,
}

impl InMemoryFormalizationStateRepository {
    fn new(state: Arc<Mutex<InMemoryFormalizationVersionState>>) -> Self {
        Self { state }
    }
}

impl FormalizationStateRepository for InMemoryFormalizationStateRepository {
    fn get_formalization_state_with_version(
        &self,
        formalization_state_ref: FormalizationStateRef,
    ) -> Result<Option<Versioned<FormalizationState>>, MethodAssetRepositoryError> {
        let state = self.state.lock().expect("in-memory state lock poisoned");
        Ok(state
            .formalization_states
            .get(formalization_state_ref.as_public_ref())
            .cloned())
    }

    fn find_formalization_state_by_definition_catalog(
        &self,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
    ) -> Result<Option<Versioned<FormalizationState>>, MethodAssetRepositoryError> {
        let state = self.state.lock().expect("in-memory state lock poisoned");
        let Some(formalization_state_ref) = state
            .formalization_state_index
            .get(&state_key(&definition_ref, &catalog_entry_ref))
        else {
            return Ok(None);
        };

        Ok(state
            .formalization_states
            .get(formalization_state_ref.as_public_ref())
            .cloned())
    }

    fn save_formalization_state(
        &self,
        formalization_state: FormalizationState,
        expected_version: Option<MethodAssetExpectedVersion>,
        uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<VersionedRef<FormalizationStateRef>, MethodAssetRepositoryError> {
        let uow = ensure_in_memory_uow(uow)?;
        if !uow.active {
            return Err(MethodAssetRepositoryError::TransactionNotActive {
                failure_marker_ref: repository_marker("formalization-state-save-inactive"),
            });
        }

        let current = {
            let state = self.state.lock().expect("in-memory state lock poisoned");
            state
                .formalization_states
                .get(formalization_state.formalization_state_ref.as_public_ref())
                .cloned()
        };

        let version = match (current, expected_version) {
            (None, None) => {
                let state = self.state.lock().expect("in-memory state lock poisoned");
                let lookup_key = state_key(
                    &formalization_state.definition_ref,
                    &formalization_state.catalog_entry_ref,
                );
                if state.formalization_state_index.contains_key(&lookup_key) {
                    return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                        conflict_marker_ref: repository_marker(
                            "formalization-state-duplicate-owner",
                        ),
                    });
                }

                MethodAssetRepositoryVersion(1)
            }
            (Some(current), Some(expected_version)) if current.version == expected_version.0 => {
                MethodAssetRepositoryVersion(current.version.0 + 1)
            }
            (Some(current), Some(expected_version)) => {
                return Err(MethodAssetRepositoryError::VersionConflict {
                    expected_version: Some(expected_version),
                    actual_version: current.version,
                    conflict_marker_ref: repository_marker("formalization-state-version-conflict"),
                });
            }
            _ => {
                return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                    conflict_marker_ref: repository_marker("formalization-state-create-conflict"),
                });
            }
        };

        let formalization_state_ref = formalization_state.formalization_state_ref.clone();
        uow.staged_operations
            .push(StagedOperation::SaveFormalizationState {
                formalization_state,
                version,
            });

        Ok(VersionedRef {
            value_ref: formalization_state_ref,
            version,
        })
    }
}

/// In-memory formal method-version repository with versioned truth semantics.
pub struct InMemoryFormalMethodAssetVersionRepository {
    state: Arc<Mutex<InMemoryFormalizationVersionState>>,
}

impl InMemoryFormalMethodAssetVersionRepository {
    fn new(state: Arc<Mutex<InMemoryFormalizationVersionState>>) -> Self {
        Self { state }
    }
}

impl FormalMethodAssetVersionRepository for InMemoryFormalMethodAssetVersionRepository {
    fn get_formal_method_asset_version_with_version(
        &self,
        formal_version_ref: FormalMethodAssetVersionRef,
    ) -> Result<Option<Versioned<FormalMethodAssetVersion>>, MethodAssetRepositoryError> {
        let state = self.state.lock().expect("in-memory state lock poisoned");
        Ok(state
            .formal_method_versions
            .get(formal_version_ref.as_public_ref())
            .cloned())
    }

    fn find_current_formal_method_asset_version(
        &self,
        formalization_state_ref: FormalizationStateRef,
    ) -> Result<Option<Versioned<FormalMethodAssetVersion>>, MethodAssetRepositoryError> {
        let state = self.state.lock().expect("in-memory state lock poisoned");
        let Some(formal_version_ref) = state
            .current_formal_version_index
            .get(formalization_state_ref.as_public_ref())
        else {
            return Ok(None);
        };

        Ok(state
            .formal_method_versions
            .get(formal_version_ref.as_public_ref())
            .cloned())
    }

    fn save_formal_method_asset_version(
        &self,
        formal_version: FormalMethodAssetVersion,
        expected_version: Option<MethodAssetExpectedVersion>,
        uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<VersionedRef<FormalMethodAssetVersionRef>, MethodAssetRepositoryError> {
        let uow = ensure_in_memory_uow(uow)?;
        if !uow.active {
            return Err(MethodAssetRepositoryError::TransactionNotActive {
                failure_marker_ref: repository_marker("formal-version-save-inactive"),
            });
        }

        let current = {
            let state = self.state.lock().expect("in-memory state lock poisoned");
            state
                .formal_method_versions
                .get(formal_version.formal_version_ref.as_public_ref())
                .cloned()
        };

        let version = match (current, expected_version) {
            (None, None) => {
                let state = self.state.lock().expect("in-memory state lock poisoned");
                if formal_version.is_current_for(&formal_version.formalization_state_ref) {
                    if state
                        .current_formal_version_index
                        .contains_key(formal_version.formalization_state_ref.as_public_ref())
                    {
                        return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                            conflict_marker_ref: repository_marker(
                                "formal-version-duplicate-current",
                            ),
                        });
                    }
                }

                MethodAssetRepositoryVersion(1)
            }
            (Some(current), Some(expected_version)) if current.version == expected_version.0 => {
                let state = self.state.lock().expect("in-memory state lock poisoned");
                if formal_version.is_current_for(&formal_version.formalization_state_ref) {
                    if let Some(existing_ref) = state
                        .current_formal_version_index
                        .get(formal_version.formalization_state_ref.as_public_ref())
                    {
                        if existing_ref != &formal_version.formal_version_ref {
                            return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                                conflict_marker_ref: repository_marker(
                                    "formal-version-current-conflict",
                                ),
                            });
                        }
                    }
                }

                MethodAssetRepositoryVersion(current.version.0 + 1)
            }
            (Some(current), Some(expected_version)) => {
                return Err(MethodAssetRepositoryError::VersionConflict {
                    expected_version: Some(expected_version),
                    actual_version: current.version,
                    conflict_marker_ref: repository_marker("formal-version-version-conflict"),
                });
            }
            _ => {
                return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                    conflict_marker_ref: repository_marker("formal-version-create-conflict"),
                });
            }
        };

        let formal_version_ref = formal_version.formal_version_ref.clone();
        uow.staged_operations
            .push(StagedOperation::SaveFormalMethodVersion {
                formal_method_version: formal_version,
                version,
            });

        Ok(VersionedRef {
            value_ref: formal_version_ref,
            version,
        })
    }
}

/// In-memory read-only basis-summary repository for the current boundary.
pub struct InMemoryFormalizationBasisSummaryRepository {
    state: Arc<Mutex<InMemoryFormalizationVersionState>>,
}

impl InMemoryFormalizationBasisSummaryRepository {
    fn new(state: Arc<Mutex<InMemoryFormalizationVersionState>>) -> Self {
        Self { state }
    }
}

impl FormalizationBasisSummaryRepository for InMemoryFormalizationBasisSummaryRepository {
    fn get_formalization_basis_summary_with_version(
        &self,
        basis_summary_ref: FormalizationBasisSummaryRef,
    ) -> Result<Option<Versioned<FormalizationBasisSummary>>, MethodAssetRepositoryError> {
        let state = self.state.lock().expect("in-memory state lock poisoned");
        Ok(state
            .basis_summaries
            .get(basis_summary_ref.as_public_ref())
            .cloned())
    }
}

/// In-memory stored-result repository for duplicate replay.
pub struct InMemoryMethodAssetStoredOperationResultRepository {
    state: Arc<Mutex<InMemoryFormalizationVersionState>>,
}

impl InMemoryMethodAssetStoredOperationResultRepository {
    fn new(state: Arc<Mutex<InMemoryFormalizationVersionState>>) -> Self {
        Self { state }
    }
}

impl MethodAssetStoredOperationResultRepository
    for InMemoryMethodAssetStoredOperationResultRepository
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
        let Some(stored_result) = state
            .stored_results
            .get(entry.stored_result_ref.as_public_ref())
        else {
            return Err(MethodAssetRepositoryError::StoredResultIntegrityViolation {
                stored_result_ref: Some(entry.stored_result_ref.clone()),
                violation_marker_ref: repository_marker("stored-result-missing"),
            });
        };

        if stored_result.operation_digest_ref != entry.operation_digest_ref {
            return Err(MethodAssetRepositoryError::StoredResultIntegrityViolation {
                stored_result_ref: Some(entry.stored_result_ref.clone()),
                violation_marker_ref: repository_marker("stored-result-digest-mismatch"),
            });
        }

        Ok(Some(stored_result.clone()))
    }

    fn get_stored_operation_result(
        &self,
        stored_result_ref: MethodAssetStoredOperationResultRef,
    ) -> Result<Option<MethodAssetStoredOperationResult>, MethodAssetRepositoryError> {
        let state = self.state.lock().expect("in-memory state lock poisoned");
        Ok(state
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
        let uow = ensure_in_memory_uow(uow)?;
        if !uow.active {
            return Err(MethodAssetRepositoryError::TransactionNotActive {
                failure_marker_ref: repository_marker("stored-result-save-inactive"),
            });
        }

        let state = self.state.lock().expect("in-memory state lock poisoned");
        let lookup_key = format!(
            "{}|{}",
            idempotency_key_ref.as_public_ref(),
            dedup_scope_ref.as_public_ref()
        );
        if state.stored_result_lookup.contains_key(&lookup_key) {
            return Err(MethodAssetRepositoryError::DuplicateKeyConflict {
                conflict_marker_ref: repository_marker("stored-result-duplicate-key"),
            });
        }
        drop(state);

        let stored_result_ref = stored_result.stored_result_ref.clone();
        uow.staged_operations
            .push(StagedOperation::SaveStoredResult {
                idempotency_key_ref,
                dedup_scope_ref,
                operation_digest_ref,
                stored_result,
            });

        Ok(stored_result_ref)
    }
}

/// In-memory basis resolver with exact body-free output.
pub struct InMemoryFormalizationBasisResolverPort;

impl FormalizationBasisResolverPort for InMemoryFormalizationBasisResolverPort {
    fn resolve_formalization_basis(
        &self,
        input: FormalizationBasisResolutionInput,
    ) -> Result<FormalizationBasisResolution, MethodAssetRepositoryError> {
        Ok(FormalizationBasisResolution {
            accepted_basis_summary_refs: input.basis_summary_refs,
            pending_marker_ref: None,
            rejection_reason_ref: None,
        })
    }
}

/// In-memory policy diagnostic builder with body-free parity.
pub struct InMemoryMethodAssetPolicyDiagnosticBuilderPort;

impl MethodAssetPolicyDiagnosticBuilderPort for InMemoryMethodAssetPolicyDiagnosticBuilderPort {
    fn build_formalization_eligibility_diagnostic(
        &self,
        definition: &MethodAssetDefinition,
        catalog_entry: &MethodAssetCatalogEntry,
        basis_resolution: &FormalizationBasisResolution,
        eligibility_rule_ref: FormalizationEligibilityRuleRef,
    ) -> Result<FormalizationEligibilityDiagnostic, MethodAssetRepositoryError> {
        let reason_summary = method_library_contracts::FormalizationStateReasonSummary::new(
            MethodLibrarySafeMarker::boundary(eligibility_rule_ref.clone().into()),
            basis_resolution.accepted_basis_summary_refs.clone(),
            basis_resolution.rejection_reason_ref.clone(),
        );

        let target_state_kind = if definition.definition_lifecycle
            != method_library_domain::MethodAssetDefinitionLifecycle::Active
            || catalog_entry.catalog_status
                != method_library_contracts::MethodAssetCatalogEntryStatus::Visible
        {
            FormalizationStateKind::Ineligible
        } else if basis_resolution.accepted_basis_summary_refs.refs.is_empty() {
            FormalizationStateKind::AssessmentPending
        } else if basis_resolution.rejection_reason_ref.is_some() {
            FormalizationStateKind::Ineligible
        } else {
            FormalizationStateKind::Eligible
        };

        Ok(FormalizationEligibilityDiagnostic {
            target_state_kind,
            reason_summary,
        })
    }

    fn build_formal_version_change_diagnostic(
        &self,
        formal_version: &FormalMethodAssetVersion,
        basis_summary_refs: &FormalizationBasisSummaryRefSet,
        _governance_basis_ref: Option<GovernanceBasisRef>,
        semantic_change_marker_ref: MethodLibrarySafeMarker,
    ) -> Result<method_library_application::FormalVersionChangeDiagnostic, MethodAssetRepositoryError>
    {
        let blocking_reason_ref = if formal_version.version_state
            != method_library_contracts::FormalMethodAssetVersionState::Active
            || basis_summary_refs.refs.is_empty()
            || !semantic_change_marker_ref.is_public_safe()
        {
            Some(MethodAssetSafeRejectReasonRef::new(format!(
                "formal-version-change-blocked:{}",
                stable_hash(formal_version.formal_version_ref.as_public_ref())
            )))
        } else {
            None
        };

        Ok(method_library_application::FormalVersionChangeDiagnostic {
            accepted_change_marker_ref: semantic_change_marker_ref,
            blocking_reason_ref,
        })
    }
}

/// In-memory support-ref factory that owns replay refs and truth refs.
pub struct InMemoryMethodAssetFormalizationVersionSupportRefFactory {
    nonce: u64,
}

impl Default for InMemoryMethodAssetFormalizationVersionSupportRefFactory {
    fn default() -> Self {
        Self { nonce: 0 }
    }
}

impl InMemoryMethodAssetFormalizationVersionSupportRefFactory {
    fn next_opaque(&mut self, prefix: &str, canonical_input: &str) -> String {
        self.nonce = self.nonce.wrapping_add(1);
        stable_hash(&format!("{prefix}|{}|{canonical_input}", self.nonce))
    }
}

impl MethodAssetFormalizationVersionSupportRefFactory
    for InMemoryMethodAssetFormalizationVersionSupportRefFactory
{
    fn formalization_version_dispatch_ref(&self) -> MethodAssetApplicationDispatchRef {
        MethodAssetApplicationDispatchRef::new("formalization-version-command-service")
    }

    fn new_api_entry_context_ref(&mut self) -> MethodAssetApiEntryContextRef {
        MethodAssetApiEntryContextRef::new(format!(
            "api-entry:{}",
            self.next_opaque("api-entry", "formalization-version")
        ))
    }

    fn build_formalization_version_replay_envelope(
        &mut self,
        input: MethodAssetFormalizationVersionReplayEnvelopeFactoryInput,
    ) -> Result<
        MethodAssetFormalizationVersionReplayEnvelope,
        method_library_application::MethodAssetReplayEnvelopeBuildError,
    > {
        if input.application_dispatch_ref != self.formalization_version_dispatch_ref() {
            return Err(
                method_library_application::MethodAssetReplayEnvelopeBuildError::UnsupportedDispatchTarget {
                    reason_ref: self.new_safe_reject_reason_ref(),
                },
            );
        }
        if !selector_matches_source(input.selector, &input.command_source) {
            return Err(
                method_library_application::MethodAssetReplayEnvelopeBuildError::SourceSelectorMismatch {
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
            return Err(
                method_library_application::MethodAssetReplayEnvelopeBuildError::MissingIdempotencyKey {
                    reason_ref: self.new_safe_reject_reason_ref(),
                },
            );
        };

        let operation_context_ref = MethodAssetOperationContextRef::new(format!(
            "operation-context:{}",
            self.next_opaque(
                "operation-context",
                &format!(
                    "{:?}|{}|{}|{}",
                    input.command_shell.capability_kind,
                    canonical_typed_boundary_ref(&input.command_shell.boundary_ref),
                    input.api_entry_context_ref.as_public_ref(),
                    input.application_dispatch_ref.as_public_ref(),
                ),
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

        Ok(MethodAssetFormalizationVersionReplayEnvelope {
            operation_context_ref,
            idempotency_key_ref,
            operation_digest_ref,
            dedup_scope_ref,
        })
    }

    fn new_stored_operation_result_ref(&mut self) -> MethodAssetStoredOperationResultRef {
        MethodAssetStoredOperationResultRef::new(format!(
            "stored-result:{}",
            self.next_opaque("stored-result", "formalization-version")
        ))
    }

    fn new_accepted_operation_summary_ref(&mut self) -> MethodAssetAcceptedOperationSummaryRef {
        MethodAssetAcceptedOperationSummaryRef::new(format!(
            "accepted-summary:{}",
            self.next_opaque("accepted-summary", "formalization-version")
        ))
    }

    fn new_safe_reject_reason_ref(&mut self) -> MethodAssetSafeRejectReasonRef {
        MethodAssetSafeRejectReasonRef::new(format!(
            "reject-reason:{}",
            self.next_opaque("reject-reason", "formalization-version")
        ))
    }

    fn new_safe_ignore_reason_ref(&mut self) -> MethodAssetSafeIgnoreReasonRef {
        MethodAssetSafeIgnoreReasonRef::new(format!(
            "ignore-reason:{}",
            self.next_opaque("ignore-reason", "formalization-version")
        ))
    }

    fn new_effect_summary_ref(&mut self) -> MethodAssetEffectSummaryRef {
        MethodAssetEffectSummaryRef::new(format!(
            "effect-summary:{}",
            self.next_opaque("effect-summary", "formalization-version")
        ))
    }

    fn new_replay_marker_ref(&mut self) -> MethodAssetReplayMarkerRef {
        MethodAssetReplayMarkerRef::new(format!(
            "replay-marker:{}",
            self.next_opaque("replay-marker", "formalization-version")
        ))
    }

    fn new_formalization_state_ref(
        &mut self,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> FormalizationStateRef {
        FormalizationStateRef::new(format!(
            "formalization-state:{}",
            stable_hash(&format!(
                "{}|{}|{}|{}|{}",
                definition_ref.as_public_ref(),
                catalog_entry_ref.as_public_ref(),
                operation_context_ref.as_public_ref(),
                operation_digest_ref.as_public_ref(),
                dedup_scope_ref.as_public_ref(),
            ))
        ))
    }

    fn new_formal_method_asset_version_ref(
        &mut self,
        formalization_state_ref: FormalizationStateRef,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
        version_boundary_summary: FormalVersionBoundarySummary,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> FormalMethodAssetVersionRef {
        FormalMethodAssetVersionRef::new(format!(
            "formal-version:{}",
            stable_hash(&format!(
                "{}|{}|{}|{}|{}|{}|{}",
                formalization_state_ref.as_public_ref(),
                definition_ref.as_public_ref(),
                catalog_entry_ref.as_public_ref(),
                canonical_version_boundary_summary(&version_boundary_summary),
                operation_context_ref.as_public_ref(),
                operation_digest_ref.as_public_ref(),
                dedup_scope_ref.as_public_ref(),
            ))
        ))
    }
}

/// Bundled in-memory runtime used by current-boundary tests and the minimal API entry.
pub struct InMemoryMethodAssetFormalizationVersionRuntime {
    state: Arc<Mutex<InMemoryFormalizationVersionState>>,
    definition_repository: Arc<InMemoryMethodAssetDefinitionRepository>,
    catalog_repository: Arc<InMemoryMethodAssetCatalogEntryRepository>,
    formalization_state_repository: Arc<InMemoryFormalizationStateRepository>,
    formal_method_asset_version_repository: Arc<InMemoryFormalMethodAssetVersionRepository>,
    formalization_basis_summary_repository: Arc<InMemoryFormalizationBasisSummaryRepository>,
    stored_result_repository: Arc<InMemoryMethodAssetStoredOperationResultRepository>,
    support_ref_factory: Arc<Mutex<Box<dyn MethodAssetFormalizationVersionSupportRefFactory>>>,
    unit_of_work: Arc<InMemoryFormalizationVersionUnitOfWorkFactory>,
    facade: Arc<dyn MethodAssetFormalizationVersionCommandFacade>,
}

impl InMemoryMethodAssetFormalizationVersionRuntime {
    /// Creates a fresh in-memory runtime with fake-parity repositories and support factory.
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(InMemoryFormalizationVersionState::default()));
        let definition_repository = Arc::new(InMemoryMethodAssetDefinitionRepository::new(
            Arc::clone(&state),
        ));
        let catalog_repository = Arc::new(InMemoryMethodAssetCatalogEntryRepository::new(
            Arc::clone(&state),
        ));
        let formalization_state_repository = Arc::new(InMemoryFormalizationStateRepository::new(
            Arc::clone(&state),
        ));
        let formal_method_asset_version_repository = Arc::new(
            InMemoryFormalMethodAssetVersionRepository::new(Arc::clone(&state)),
        );
        let formalization_basis_summary_repository = Arc::new(
            InMemoryFormalizationBasisSummaryRepository::new(Arc::clone(&state)),
        );
        let formalization_basis_resolver = Arc::new(InMemoryFormalizationBasisResolverPort);
        let policy_diagnostic_builder = Arc::new(InMemoryMethodAssetPolicyDiagnosticBuilderPort);
        let stored_result_repository = Arc::new(
            InMemoryMethodAssetStoredOperationResultRepository::new(Arc::clone(&state)),
        );
        let support_ref_factory: Arc<
            Mutex<Box<dyn MethodAssetFormalizationVersionSupportRefFactory>>,
        > = Arc::new(Mutex::new(Box::new(
            InMemoryMethodAssetFormalizationVersionSupportRefFactory::default(),
        )));
        let unit_of_work = Arc::new(InMemoryFormalizationVersionUnitOfWorkFactory::new(
            Arc::clone(&state),
        ));
        let facade: Arc<dyn MethodAssetFormalizationVersionCommandFacade> =
            Arc::new(DefaultMethodAssetFormalizationVersionCommandFacade::new(
                definition_repository.clone(),
                catalog_repository.clone(),
                formalization_state_repository.clone(),
                formal_method_asset_version_repository.clone(),
                formalization_basis_summary_repository.clone(),
                formalization_basis_resolver.clone(),
                policy_diagnostic_builder.clone(),
                stored_result_repository.clone(),
                unit_of_work.clone(),
                support_ref_factory.clone(),
            ));

        Self {
            state,
            definition_repository,
            catalog_repository,
            formalization_state_repository,
            formal_method_asset_version_repository,
            formalization_basis_summary_repository,
            stored_result_repository,
            support_ref_factory,
            unit_of_work,
            facade,
        }
    }

    /// Returns the current-boundary command facade.
    pub fn facade(&self) -> Arc<dyn MethodAssetFormalizationVersionCommandFacade> {
        Arc::clone(&self.facade)
    }

    /// Returns the shared support-ref factory handle.
    pub fn support_ref_factory(
        &self,
    ) -> Arc<Mutex<Box<dyn MethodAssetFormalizationVersionSupportRefFactory>>> {
        Arc::clone(&self.support_ref_factory)
    }

    /// Returns the definition repository for verification in tests.
    pub fn definition_repository(&self) -> Arc<InMemoryMethodAssetDefinitionRepository> {
        Arc::clone(&self.definition_repository)
    }

    /// Returns the catalog repository for verification in tests.
    pub fn catalog_repository(&self) -> Arc<InMemoryMethodAssetCatalogEntryRepository> {
        Arc::clone(&self.catalog_repository)
    }

    /// Returns the formalization-state repository for verification in tests.
    pub fn formalization_state_repository(&self) -> Arc<InMemoryFormalizationStateRepository> {
        Arc::clone(&self.formalization_state_repository)
    }

    /// Returns the formal method-version repository for verification in tests.
    pub fn formal_method_asset_version_repository(
        &self,
    ) -> Arc<InMemoryFormalMethodAssetVersionRepository> {
        Arc::clone(&self.formal_method_asset_version_repository)
    }

    /// Returns the basis-summary repository for verification in tests.
    pub fn formalization_basis_summary_repository(
        &self,
    ) -> Arc<InMemoryFormalizationBasisSummaryRepository> {
        Arc::clone(&self.formalization_basis_summary_repository)
    }

    /// Returns the stored-result repository for verification in tests.
    pub fn stored_result_repository(
        &self,
    ) -> Arc<InMemoryMethodAssetStoredOperationResultRepository> {
        Arc::clone(&self.stored_result_repository)
    }

    /// Returns the unit-of-work factory for direct staging tests.
    pub fn unit_of_work(&self) -> Arc<InMemoryFormalizationVersionUnitOfWorkFactory> {
        Arc::clone(&self.unit_of_work)
    }

    /// Seeds a basis summary for targeted tests.
    pub fn seed_basis_summary(&self, basis_summary: FormalizationBasisSummary) {
        let mut state = self.state.lock().expect("in-memory state lock poisoned");
        state.basis_summaries.insert(
            basis_summary.basis_summary_ref.as_public_ref().to_owned(),
            Versioned {
                value: basis_summary,
                version: MethodAssetRepositoryVersion(1),
            },
        );
    }

    /// Causes the next commit to return `CommitUnknown` after applying staged writes.
    pub fn simulate_commit_unknown_once(&self) {
        let mut state = self.state.lock().expect("in-memory state lock poisoned");
        state.commit_unknown_once = true;
    }

    /// Removes a stored result while keeping any idempotency lookup entry intact.
    pub fn remove_stored_result(&self, stored_result_ref: &MethodAssetStoredOperationResultRef) {
        let mut state = self.state.lock().expect("in-memory state lock poisoned");
        state
            .stored_results
            .remove(stored_result_ref.as_public_ref());
    }

    /// Corrupts the lookup digest for the stored result tied to the provided ref.
    pub fn corrupt_stored_result_lookup_digest(
        &self,
        stored_result_ref: &MethodAssetStoredOperationResultRef,
    ) {
        let mut state = self.state.lock().expect("in-memory state lock poisoned");
        let Some((_, entry)) = state
            .stored_result_lookup
            .iter_mut()
            .find(|(_, entry)| &entry.stored_result_ref == stored_result_ref)
        else {
            return;
        };
        entry.operation_digest_ref =
            MethodAssetOperationDigestRef::new("operation-digest:corrupted");
    }
}
