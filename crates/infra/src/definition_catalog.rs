//! In-memory fake/runtime support for the `commit-03-b` accepted-service slice.

use core::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use method_library_application::UnitOfWork;
use method_library_application::ports::{
    ExternalSourceSummaryValidationPort, MethodAssetCatalogEntryRepository,
    MethodAssetDefinitionRepository, MethodAssetStoredOperationResultRepository,
};
use method_library_application::{
    DefaultMethodAssetDefinitionCatalogCommandFacade, MethodAssetCommitObservation,
    MethodAssetDefinitionCatalogCommandFacade, MethodAssetDefinitionCatalogCommandSelector,
    MethodAssetDefinitionCatalogCommandSource, MethodAssetDefinitionCatalogReplayEnvelope,
    MethodAssetDefinitionCatalogReplayEnvelopeFactoryInput,
    MethodAssetDefinitionCatalogSupportRefFactory, MethodAssetExpectedVersion,
    MethodAssetRepositoryError, MethodAssetRepositoryVersion, MethodAssetStoredOperationResult,
    Versioned, VersionedRef,
};
use method_library_contracts::{
    CatalogScopeRef, ExternalSourceSummaryRefSet, MethodAssetAcceptedOperationSummaryRef,
    MethodAssetApiEntryContextRef, MethodAssetApplicabilitySummary,
    MethodAssetApplicationDispatchRef, MethodAssetCatalogClassification,
    MethodAssetCatalogEntryRef, MethodAssetDedupScopeRef, MethodAssetDefinitionRef,
    MethodAssetEffectSummaryRef, MethodAssetIdempotencyKeyRef, MethodAssetIdentityKey,
    MethodAssetOperationContextRef, MethodAssetOperationDigestRef, MethodAssetReplayMarkerRef,
    MethodAssetSafeIgnoreReasonRef, MethodAssetSafeRejectReasonRef,
    MethodAssetStoredOperationResultRef, MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef,
    MethodLibraryTypedBoundaryRefKind,
};
use method_library_domain::{MethodAssetCatalogEntry, MethodAssetDefinition};

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

fn canonical_identity_key(identity_key: &MethodAssetIdentityKey) -> String {
    format!(
        "{:?}|{}|{}|{}",
        identity_key.definition_kind,
        canonical_typed_boundary_ref(&identity_key.identity_namespace_ref),
        canonical_typed_boundary_ref(&identity_key.identity_anchor_ref),
        identity_key.applicability_scope_ref.as_public_ref(),
    )
}

fn canonical_definition_summary(
    summary: &method_library_contracts::MethodAssetDefinitionSummary,
) -> String {
    format!(
        "{}|{:?}|{}|{}|{}",
        canonical_typed_boundary_ref(&summary.summary_ref),
        summary.definition_kind,
        canonical_typed_boundary_ref(&summary.safe_title_ref),
        summary
            .safe_description_ref
            .as_ref()
            .map(canonical_typed_boundary_ref)
            .unwrap_or_else(|| "-".to_owned()),
        canonical_safe_marker(&summary.summary_marker_ref),
    )
}

fn canonical_external_summary_ref_set(refs: &ExternalSourceSummaryRefSet) -> String {
    refs.refs
        .iter()
        .map(|item| canonical_typed_boundary_ref(item.as_typed_ref()))
        .collect::<Vec<_>>()
        .join(",")
}

fn canonical_catalog_entry_ref_set(
    refs: &method_library_contracts::MethodAssetCatalogEntryRefSet,
) -> String {
    refs.refs
        .iter()
        .map(|item| canonical_typed_boundary_ref(item.as_typed_ref()))
        .collect::<Vec<_>>()
        .join(",")
}

fn canonical_catalog_classification(classification: &MethodAssetCatalogClassification) -> String {
    format!(
        "{:?}|{}|{}",
        classification.definition_kind,
        classification.catalog_scope_ref.as_public_ref(),
        canonical_safe_marker(&classification.classification_marker_ref),
    )
}

fn canonical_applicability_summary(summary: &MethodAssetApplicabilitySummary) -> String {
    let context_refs = summary
        .applicable_context_refs
        .iter()
        .map(canonical_typed_boundary_ref)
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{}|{}|{}",
        summary.applicability_scope_ref.as_public_ref(),
        canonical_safe_marker(&summary.applicability_marker_ref),
        context_refs,
    )
}

fn canonical_command_source(source: &MethodAssetDefinitionCatalogCommandSource) -> String {
    match source {
        MethodAssetDefinitionCatalogCommandSource::EstablishDefinition(source) => format!(
            "establish|{:?}|{}|{}|{}|{}",
            source.definition_kind,
            canonical_identity_key(&source.identity_key),
            canonical_definition_summary(&source.definition_summary),
            canonical_external_summary_ref_set(&source.source_summary_refs),
            canonical_catalog_entry_ref_set(&source.preaccepted_catalog_entry_refs),
        ),
        MethodAssetDefinitionCatalogCommandSource::AdjustDefinition(source) => format!(
            "adjust|{}|{}|{}",
            source.definition_ref.as_public_ref(),
            canonical_definition_summary(&source.replacement_definition_summary),
            canonical_external_summary_ref_set(&source.replacement_source_summary_refs),
        ),
        MethodAssetDefinitionCatalogCommandSource::RetireDefinition(source) => format!(
            "retire_definition|{}|{}",
            source.definition_ref.as_public_ref(),
            canonical_safe_marker(&source.retirement_marker_ref),
        ),
        MethodAssetDefinitionCatalogCommandSource::RegisterCatalogEntry(source) => format!(
            "register_catalog|{}|{}|{}|{}",
            source.definition_ref.as_public_ref(),
            source.catalog_scope_ref.as_public_ref(),
            canonical_catalog_classification(&source.catalog_classification),
            canonical_applicability_summary(&source.applicability_summary),
        ),
        MethodAssetDefinitionCatalogCommandSource::ReclassifyCatalogEntry(source) => format!(
            "reclassify_catalog|{}|{}|{}",
            source.catalog_entry_ref.as_public_ref(),
            canonical_catalog_classification(&source.new_catalog_classification),
            canonical_applicability_summary(&source.new_applicability_summary),
        ),
        MethodAssetDefinitionCatalogCommandSource::RetireCatalogEntry(source) => format!(
            "retire_catalog|{}|{}",
            source.catalog_entry_ref.as_public_ref(),
            canonical_safe_marker(&source.retirement_marker_ref),
        ),
    }
}

fn canonical_selector(selector: MethodAssetDefinitionCatalogCommandSelector) -> &'static str {
    match selector {
        MethodAssetDefinitionCatalogCommandSelector::EstablishDefinition => "establish_definition",
        MethodAssetDefinitionCatalogCommandSelector::AdjustDefinition => "adjust_definition",
        MethodAssetDefinitionCatalogCommandSelector::RetireDefinition => "retire_definition",
        MethodAssetDefinitionCatalogCommandSelector::RegisterCatalogEntry => {
            "register_catalog_entry"
        }
        MethodAssetDefinitionCatalogCommandSelector::ReclassifyCatalogEntry => {
            "reclassify_catalog_entry"
        }
        MethodAssetDefinitionCatalogCommandSelector::RetireCatalogEntry => "retire_catalog_entry",
    }
}

fn selector_matches_source(
    selector: MethodAssetDefinitionCatalogCommandSelector,
    source: &MethodAssetDefinitionCatalogCommandSource,
) -> bool {
    matches!(
        (selector, source),
        (
            MethodAssetDefinitionCatalogCommandSelector::EstablishDefinition,
            MethodAssetDefinitionCatalogCommandSource::EstablishDefinition(_),
        ) | (
            MethodAssetDefinitionCatalogCommandSelector::AdjustDefinition,
            MethodAssetDefinitionCatalogCommandSource::AdjustDefinition(_),
        ) | (
            MethodAssetDefinitionCatalogCommandSelector::RetireDefinition,
            MethodAssetDefinitionCatalogCommandSource::RetireDefinition(_),
        ) | (
            MethodAssetDefinitionCatalogCommandSelector::RegisterCatalogEntry,
            MethodAssetDefinitionCatalogCommandSource::RegisterCatalogEntry(_),
        ) | (
            MethodAssetDefinitionCatalogCommandSelector::ReclassifyCatalogEntry,
            MethodAssetDefinitionCatalogCommandSource::ReclassifyCatalogEntry(_),
        ) | (
            MethodAssetDefinitionCatalogCommandSelector::RetireCatalogEntry,
            MethodAssetDefinitionCatalogCommandSource::RetireCatalogEntry(_),
        )
    )
}

fn canonical_dedup_scope(
    selector: MethodAssetDefinitionCatalogCommandSelector,
    source: &MethodAssetDefinitionCatalogCommandSource,
) -> String {
    match (selector, source) {
        (
            MethodAssetDefinitionCatalogCommandSelector::EstablishDefinition,
            MethodAssetDefinitionCatalogCommandSource::EstablishDefinition(source),
        ) => format!(
            "definition_catalog|{}|identity|{}",
            canonical_selector(selector),
            canonical_identity_key(&source.identity_key),
        ),
        (
            MethodAssetDefinitionCatalogCommandSelector::AdjustDefinition,
            MethodAssetDefinitionCatalogCommandSource::AdjustDefinition(source),
        ) => format!(
            "definition_catalog|{}|definition|{}",
            canonical_selector(selector),
            source.definition_ref.as_public_ref(),
        ),
        (
            MethodAssetDefinitionCatalogCommandSelector::RetireDefinition,
            MethodAssetDefinitionCatalogCommandSource::RetireDefinition(source),
        ) => format!(
            "definition_catalog|{}|definition|{}",
            canonical_selector(selector),
            source.definition_ref.as_public_ref(),
        ),
        (
            MethodAssetDefinitionCatalogCommandSelector::RegisterCatalogEntry,
            MethodAssetDefinitionCatalogCommandSource::RegisterCatalogEntry(source),
        ) => format!(
            "definition_catalog|{}|definition|{}|scope|{}",
            canonical_selector(selector),
            source.definition_ref.as_public_ref(),
            source.catalog_scope_ref.as_public_ref(),
        ),
        (
            MethodAssetDefinitionCatalogCommandSelector::ReclassifyCatalogEntry,
            MethodAssetDefinitionCatalogCommandSource::ReclassifyCatalogEntry(source),
        ) => format!(
            "definition_catalog|{}|catalog_entry|{}",
            canonical_selector(selector),
            source.catalog_entry_ref.as_public_ref(),
        ),
        (
            MethodAssetDefinitionCatalogCommandSelector::RetireCatalogEntry,
            MethodAssetDefinitionCatalogCommandSource::RetireCatalogEntry(source),
        ) => format!(
            "definition_catalog|{}|catalog_entry|{}",
            canonical_selector(selector),
            source.catalog_entry_ref.as_public_ref(),
        ),
        _ => "definition_catalog|mismatch".to_owned(),
    }
}

fn canonical_operation_digest(
    input: &MethodAssetDefinitionCatalogReplayEnvelopeFactoryInput,
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

fn repository_marker(label: &str) -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::boundary(MethodLibraryTypedBoundaryRef::from_verified_source(
        MethodLibraryTypedBoundaryRefKind::MethodAssetApplicationDispatch,
        format!("method-library-infra:{label}"),
    ))
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

#[derive(Clone)]
struct StoredResultLookupEntry {
    stored_result_ref: MethodAssetStoredOperationResultRef,
    operation_digest_ref: MethodAssetOperationDigestRef,
}

#[derive(Default)]
struct InMemoryDefinitionCatalogState {
    definitions: HashMap<String, Versioned<MethodAssetDefinition>>,
    definition_identity_index: HashMap<String, MethodAssetDefinitionRef>,
    catalog_entries: HashMap<String, Versioned<MethodAssetCatalogEntry>>,
    catalog_scope_index: HashMap<String, MethodAssetCatalogEntryRef>,
    stored_results: HashMap<String, MethodAssetStoredOperationResult>,
    stored_result_lookup: HashMap<String, StoredResultLookupEntry>,
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
    SaveStoredResult {
        idempotency_key_ref: MethodAssetIdempotencyKeyRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        stored_result: MethodAssetStoredOperationResult,
    },
}

struct InMemoryCommandUnitOfWork {
    state: Arc<Mutex<InMemoryDefinitionCatalogState>>,
    staged_operations: Vec<StagedOperation>,
    active: bool,
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
        Ok(MethodAssetCommitObservation::Committed)
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
pub struct InMemoryUnitOfWorkFactory {
    state: Arc<Mutex<InMemoryDefinitionCatalogState>>,
}

impl InMemoryUnitOfWorkFactory {
    fn new(state: Arc<Mutex<InMemoryDefinitionCatalogState>>) -> Self {
        Self { state }
    }
}

impl UnitOfWork for InMemoryUnitOfWorkFactory {
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
    state: Arc<Mutex<InMemoryDefinitionCatalogState>>,
}

impl InMemoryMethodAssetDefinitionRepository {
    fn new(state: Arc<Mutex<InMemoryDefinitionCatalogState>>) -> Self {
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
    state: Arc<Mutex<InMemoryDefinitionCatalogState>>,
}

impl InMemoryMethodAssetCatalogEntryRepository {
    fn new(state: Arc<Mutex<InMemoryDefinitionCatalogState>>) -> Self {
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
        catalog_scope_ref: CatalogScopeRef,
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

/// In-memory stored-result repository for duplicate replay.
pub struct InMemoryMethodAssetStoredOperationResultRepository {
    state: Arc<Mutex<InMemoryDefinitionCatalogState>>,
}

impl InMemoryMethodAssetStoredOperationResultRepository {
    fn new(state: Arc<Mutex<InMemoryDefinitionCatalogState>>) -> Self {
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

/// Named-ref validation carve-out for external summary refs.
pub struct InMemoryExternalSourceSummaryValidationPort;

impl ExternalSourceSummaryValidationPort for InMemoryExternalSourceSummaryValidationPort {
    fn validate_named_refs(
        &self,
        refs: &ExternalSourceSummaryRefSet,
    ) -> Result<(), MethodAssetRepositoryError> {
        if refs.refs.iter().all(|item| {
            item.as_typed_ref().kind()
                == method_library_contracts::ExternalSourceSummaryRef::expected_kind()
        }) {
            Ok(())
        } else {
            Err(MethodAssetRepositoryError::StorageUnavailable {
                unavailable_marker_ref: repository_marker("external-summary-wrong-kind"),
            })
        }
    }
}

/// In-memory support-ref factory that owns replay refs and truth refs.
pub struct InMemoryMethodAssetDefinitionCatalogSupportRefFactory {
    nonce: u64,
}

impl Default for InMemoryMethodAssetDefinitionCatalogSupportRefFactory {
    fn default() -> Self {
        Self { nonce: 0 }
    }
}

impl InMemoryMethodAssetDefinitionCatalogSupportRefFactory {
    fn next_opaque(&mut self, prefix: &str, canonical_input: &str) -> String {
        self.nonce = self.nonce.wrapping_add(1);
        stable_hash(&format!("{prefix}|{}|{canonical_input}", self.nonce))
    }
}

impl MethodAssetDefinitionCatalogSupportRefFactory
    for InMemoryMethodAssetDefinitionCatalogSupportRefFactory
{
    fn definition_catalog_dispatch_ref(&self) -> MethodAssetApplicationDispatchRef {
        MethodAssetApplicationDispatchRef::new("definition-catalog-command-service")
    }

    fn new_api_entry_context_ref(&mut self) -> MethodAssetApiEntryContextRef {
        MethodAssetApiEntryContextRef::new(format!(
            "api-entry:{}",
            self.next_opaque("api-entry", "definition-catalog")
        ))
    }

    fn build_definition_catalog_replay_envelope(
        &mut self,
        input: MethodAssetDefinitionCatalogReplayEnvelopeFactoryInput,
    ) -> Result<
        MethodAssetDefinitionCatalogReplayEnvelope,
        method_library_application::MethodAssetReplayEnvelopeBuildError,
    > {
        if input.application_dispatch_ref != self.definition_catalog_dispatch_ref() {
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

        Ok(MethodAssetDefinitionCatalogReplayEnvelope {
            operation_context_ref,
            idempotency_key_ref,
            operation_digest_ref,
            dedup_scope_ref,
        })
    }

    fn new_stored_operation_result_ref(&mut self) -> MethodAssetStoredOperationResultRef {
        MethodAssetStoredOperationResultRef::new(format!(
            "stored-result:{}",
            self.next_opaque("stored-result", "command")
        ))
    }

    fn new_accepted_operation_summary_ref(&mut self) -> MethodAssetAcceptedOperationSummaryRef {
        MethodAssetAcceptedOperationSummaryRef::new(format!(
            "accepted-summary:{}",
            self.next_opaque("accepted-summary", "command")
        ))
    }

    fn new_safe_reject_reason_ref(&mut self) -> MethodAssetSafeRejectReasonRef {
        MethodAssetSafeRejectReasonRef::new(format!(
            "reject-reason:{}",
            self.next_opaque("reject-reason", "command")
        ))
    }

    fn new_safe_ignore_reason_ref(&mut self) -> MethodAssetSafeIgnoreReasonRef {
        MethodAssetSafeIgnoreReasonRef::new(format!(
            "ignore-reason:{}",
            self.next_opaque("ignore-reason", "command")
        ))
    }

    fn new_effect_summary_ref(&mut self) -> MethodAssetEffectSummaryRef {
        MethodAssetEffectSummaryRef::new(format!(
            "effect-summary:{}",
            self.next_opaque("effect-summary", "command")
        ))
    }

    fn new_replay_marker_ref(&mut self) -> MethodAssetReplayMarkerRef {
        MethodAssetReplayMarkerRef::new(format!(
            "replay-marker:{}",
            self.next_opaque("replay-marker", "command")
        ))
    }

    fn new_definition_ref(
        &mut self,
        identity_key: MethodAssetIdentityKey,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> MethodAssetDefinitionRef {
        MethodAssetDefinitionRef::new(format!(
            "definition:{}",
            stable_hash(&format!(
                "{}|{}|{}|{}",
                canonical_identity_key(&identity_key),
                operation_context_ref.as_public_ref(),
                operation_digest_ref.as_public_ref(),
                dedup_scope_ref.as_public_ref(),
            ))
        ))
    }

    fn new_catalog_entry_ref(
        &mut self,
        definition_ref: MethodAssetDefinitionRef,
        catalog_scope_ref: CatalogScopeRef,
        catalog_classification: MethodAssetCatalogClassification,
        applicability_summary: MethodAssetApplicabilitySummary,
        operation_context_ref: MethodAssetOperationContextRef,
        operation_digest_ref: MethodAssetOperationDigestRef,
        dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> MethodAssetCatalogEntryRef {
        MethodAssetCatalogEntryRef::new(format!(
            "catalog-entry:{}",
            stable_hash(&format!(
                "{}|{}|{}|{}|{}|{}|{}",
                definition_ref.as_public_ref(),
                catalog_scope_ref.as_public_ref(),
                canonical_catalog_classification(&catalog_classification),
                canonical_applicability_summary(&applicability_summary),
                operation_context_ref.as_public_ref(),
                operation_digest_ref.as_public_ref(),
                dedup_scope_ref.as_public_ref(),
            ))
        ))
    }
}

/// Bundled in-memory runtime used by current-boundary tests and the minimal API entry.
pub struct InMemoryMethodAssetDefinitionCatalogRuntime {
    definition_repository: Arc<InMemoryMethodAssetDefinitionRepository>,
    catalog_repository: Arc<InMemoryMethodAssetCatalogEntryRepository>,
    stored_result_repository: Arc<InMemoryMethodAssetStoredOperationResultRepository>,
    external_summary_validation: Arc<InMemoryExternalSourceSummaryValidationPort>,
    support_ref_factory: Arc<Mutex<Box<dyn MethodAssetDefinitionCatalogSupportRefFactory>>>,
    unit_of_work: Arc<InMemoryUnitOfWorkFactory>,
    facade: Arc<dyn MethodAssetDefinitionCatalogCommandFacade>,
}

impl InMemoryMethodAssetDefinitionCatalogRuntime {
    /// Creates a fresh in-memory runtime with fake-parity repositories and support factory.
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(InMemoryDefinitionCatalogState::default()));
        let definition_repository = Arc::new(InMemoryMethodAssetDefinitionRepository::new(
            Arc::clone(&state),
        ));
        let catalog_repository = Arc::new(InMemoryMethodAssetCatalogEntryRepository::new(
            Arc::clone(&state),
        ));
        let stored_result_repository = Arc::new(
            InMemoryMethodAssetStoredOperationResultRepository::new(Arc::clone(&state)),
        );
        let external_summary_validation = Arc::new(InMemoryExternalSourceSummaryValidationPort);
        let support_ref_factory: Arc<
            Mutex<Box<dyn MethodAssetDefinitionCatalogSupportRefFactory>>,
        > = Arc::new(Mutex::new(Box::new(
            InMemoryMethodAssetDefinitionCatalogSupportRefFactory::default(),
        )));
        let unit_of_work = Arc::new(InMemoryUnitOfWorkFactory::new(state));
        let facade: Arc<dyn MethodAssetDefinitionCatalogCommandFacade> =
            Arc::new(DefaultMethodAssetDefinitionCatalogCommandFacade::new(
                definition_repository.clone(),
                catalog_repository.clone(),
                stored_result_repository.clone(),
                external_summary_validation.clone(),
                unit_of_work.clone(),
                support_ref_factory.clone(),
            ));

        Self {
            definition_repository,
            catalog_repository,
            stored_result_repository,
            external_summary_validation,
            support_ref_factory,
            unit_of_work,
            facade,
        }
    }

    /// Returns the current-boundary command facade.
    pub fn facade(&self) -> Arc<dyn MethodAssetDefinitionCatalogCommandFacade> {
        Arc::clone(&self.facade)
    }

    /// Returns the shared support-ref factory handle.
    pub fn support_ref_factory(
        &self,
    ) -> Arc<Mutex<Box<dyn MethodAssetDefinitionCatalogSupportRefFactory>>> {
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

    /// Returns the stored-result repository for verification in tests.
    pub fn stored_result_repository(
        &self,
    ) -> Arc<InMemoryMethodAssetStoredOperationResultRepository> {
        Arc::clone(&self.stored_result_repository)
    }

    /// Returns the unit-of-work factory for direct staging tests.
    pub fn unit_of_work(&self) -> Arc<InMemoryUnitOfWorkFactory> {
        Arc::clone(&self.unit_of_work)
    }

    /// Returns the external-summary validation port for completeness.
    pub fn external_summary_validation(&self) -> Arc<InMemoryExternalSourceSummaryValidationPort> {
        Arc::clone(&self.external_summary_validation)
    }
}
