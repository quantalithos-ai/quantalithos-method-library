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
    MethodAssetFormalizationVersionCommandDispatchInput,
    MethodAssetFormalizationVersionCommandFacade, MethodAssetFormalizationVersionCommandSelector,
    MethodAssetFormalizationVersionCommandSource, MethodAssetFormalizationVersionReplayEnvelope,
    MethodAssetFormalizationVersionReplayEnvelopeFactoryInput,
    MethodAssetFormalizationVersionSupportRefFactory, MethodAssetRepositoryError,
    MethodAssetRepositoryVersion, MethodAssetStoredOperationResult, UnitOfWork, Versioned,
};
use method_library_contracts::fixtures::sample_command_shell;
use method_library_contracts::metadata::IdempotencyKey;
use method_library_contracts::{
    ExternalSourceSummaryRef, FormalMethodAssetVersionRef, FormalizationBasisSafeSummary,
    FormalizationBasisSummaryRef, FormalizationBasisSummaryRefSet, FormalizationEligibilityRuleRef,
    FormalizationStateKind, FormalizationStateReasonSummary, FormalizationStateRef,
    MethodAssetAcceptedOperationSummaryRef, MethodAssetApiEntryContextRef,
    MethodAssetApplicationDispatchRef, MethodAssetCatalogClassification,
    MethodAssetCatalogEntryRef, MethodAssetDedupScopeRef, MethodAssetDefinitionKind,
    MethodAssetDefinitionRef, MethodAssetDefinitionSummary, MethodAssetEffectSummaryRef,
    MethodAssetIdempotencyKeyRef, MethodAssetIdentityKey, MethodAssetOperationContextRef,
    MethodAssetOperationDigestRef, MethodAssetReplayMarkerRef, MethodAssetSafeIgnoreReasonRef,
    MethodAssetSafeRejectReasonRef, MethodAssetStoredOperationResultRef,
    MethodLibraryCapabilityKind, MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef,
    MethodLibraryTypedBoundaryRefKind,
};
use method_library_domain::{
    FormalMethodAssetVersion, FormalizationBasisSummary, FormalizationState,
    MethodAssetCatalogEntry, MethodAssetDefinition,
};

macro_rules! assert_object_safe {
    ($($trait_name:ty),+ $(,)?) => {
        $(
            let _: Option<&$trait_name> = None;
        )+
    };
}

fn typed_ref(
    kind: MethodLibraryTypedBoundaryRefKind,
    value: impl Into<String>,
) -> MethodLibraryTypedBoundaryRef {
    MethodLibraryTypedBoundaryRef::from_verified_source(kind, value)
}

fn no_body_marker(label: &str) -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::no_body(typed_ref(
        MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
        format!("marker:{label}"),
    ))
}

fn definition_summary(
    definition_kind: MethodAssetDefinitionKind,
    label: &str,
) -> MethodAssetDefinitionSummary {
    MethodAssetDefinitionSummary::new(
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
            format!("definition-summary:{label}"),
        ),
        definition_kind,
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            format!("definition-title:{label}"),
        ),
        None,
        no_body_marker(&format!("definition-summary:{label}")),
    )
}

fn command_shell(
    idempotency_key: &str,
    boundary_kind: MethodLibraryTypedBoundaryRefKind,
) -> method_library_contracts::MethodLibraryCommandShell {
    let mut shell = sample_command_shell();
    shell.capability_kind = MethodLibraryCapabilityKind::FormalizationVersion;
    shell.boundary_ref = typed_ref(boundary_kind, format!("boundary:{idempotency_key}"));
    shell.metadata.request.idempotency_key = Some(IdempotencyKey::new(idempotency_key));
    shell
}

#[test]
fn formalization_version_surfaces_are_object_safe() {
    assert_object_safe!(
        dyn MethodAssetDefinitionRepository,
        dyn MethodAssetCatalogEntryRepository,
        dyn FormalizationStateRepository,
        dyn FormalMethodAssetVersionRepository,
        dyn FormalizationBasisSummaryRepository,
        dyn FormalizationBasisResolverPort,
        dyn MethodAssetPolicyDiagnosticBuilderPort,
        dyn MethodAssetStoredOperationResultRepository,
        dyn UnitOfWork,
        dyn MethodAssetFormalizationVersionCommandFacade,
        dyn MethodAssetFormalizationVersionSupportRefFactory,
    );
}

#[test]
fn formalization_version_selectors_and_commit_observations_are_closed() {
    let selectors = [
        MethodAssetFormalizationVersionCommandSelector::EvaluateFormalizationEligibility,
        MethodAssetFormalizationVersionCommandSelector::InitiateFormalization,
        MethodAssetFormalizationVersionCommandSelector::EstablishFormalVersion,
        MethodAssetFormalizationVersionCommandSelector::RecordFormalVersionSemanticChange,
        MethodAssetFormalizationVersionCommandSelector::SupersedeFormalVersion,
        MethodAssetFormalizationVersionCommandSelector::RetireFormalVersion,
    ];
    assert_eq!(selectors.len(), 6);

    let observations = [
        MethodAssetCommitObservation::Committed,
        MethodAssetCommitObservation::CommitUnknown {
            unknown_marker_ref: no_body_marker("commit-unknown"),
        },
    ];
    assert_eq!(observations.len(), 2);
}

struct NoopUnitOfWork;

impl method_library_application::CommandUnitOfWork for NoopUnitOfWork {
    fn commit(&mut self) -> Result<MethodAssetCommitObservation, ()> {
        Ok(MethodAssetCommitObservation::Committed)
    }

    fn rollback(&mut self) -> Result<(), ()> {
        Ok(())
    }
}

struct StubUnitOfWork;

impl UnitOfWork for StubUnitOfWork {
    fn begin_command_uow(&self) -> Box<dyn method_library_application::CommandUnitOfWork> {
        Box::new(NoopUnitOfWork)
    }
}

struct StubDefinitionRepository {
    value: Versioned<MethodAssetDefinition>,
}

impl MethodAssetDefinitionRepository for StubDefinitionRepository {
    fn get_definition_with_version(
        &self,
        definition_ref: MethodAssetDefinitionRef,
    ) -> Result<Option<Versioned<MethodAssetDefinition>>, MethodAssetRepositoryError> {
        if self.value.value.definition_ref == definition_ref {
            Ok(Some(self.value.clone()))
        } else {
            Ok(None)
        }
    }

    fn find_definition_by_identity_key(
        &self,
        _identity_key: MethodAssetIdentityKey,
    ) -> Result<Option<Versioned<MethodAssetDefinition>>, MethodAssetRepositoryError> {
        Ok(None)
    }

    fn save_definition(
        &self,
        _definition: MethodAssetDefinition,
        _expected_version: Option<MethodAssetExpectedVersion>,
        _uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<
        method_library_application::VersionedRef<MethodAssetDefinitionRef>,
        MethodAssetRepositoryError,
    > {
        unreachable!("definition save is not part of the conflict test")
    }
}

struct StubCatalogRepository {
    value: Versioned<MethodAssetCatalogEntry>,
}

impl MethodAssetCatalogEntryRepository for StubCatalogRepository {
    fn get_catalog_entry_with_version(
        &self,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
    ) -> Result<Option<Versioned<MethodAssetCatalogEntry>>, MethodAssetRepositoryError> {
        if self.value.value.catalog_entry_ref == catalog_entry_ref {
            Ok(Some(self.value.clone()))
        } else {
            Ok(None)
        }
    }

    fn find_catalog_entry_by_definition_scope(
        &self,
        _definition_ref: MethodAssetDefinitionRef,
        _catalog_scope_ref: method_library_contracts::CatalogScopeRef,
    ) -> Result<Option<Versioned<MethodAssetCatalogEntry>>, MethodAssetRepositoryError> {
        Ok(None)
    }

    fn save_catalog_entry(
        &self,
        _catalog_entry: MethodAssetCatalogEntry,
        _expected_version: Option<MethodAssetExpectedVersion>,
        _uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<
        method_library_application::VersionedRef<MethodAssetCatalogEntryRef>,
        MethodAssetRepositoryError,
    > {
        unreachable!("catalog save is not part of the conflict test")
    }
}

struct StubFormalizationStateRepository {
    value: Versioned<FormalizationState>,
}

impl FormalizationStateRepository for StubFormalizationStateRepository {
    fn get_formalization_state_with_version(
        &self,
        _formalization_state_ref: FormalizationStateRef,
    ) -> Result<Option<Versioned<FormalizationState>>, MethodAssetRepositoryError> {
        Ok(None)
    }

    fn find_formalization_state_by_definition_catalog(
        &self,
        _definition_ref: MethodAssetDefinitionRef,
        _catalog_entry_ref: MethodAssetCatalogEntryRef,
    ) -> Result<Option<Versioned<FormalizationState>>, MethodAssetRepositoryError> {
        Ok(Some(self.value.clone()))
    }

    fn save_formalization_state(
        &self,
        _formalization_state: FormalizationState,
        expected_version: Option<MethodAssetExpectedVersion>,
        _uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<
        method_library_application::VersionedRef<FormalizationStateRef>,
        MethodAssetRepositoryError,
    > {
        Err(MethodAssetRepositoryError::VersionConflict {
            expected_version,
            actual_version: MethodAssetRepositoryVersion(9),
            conflict_marker_ref: no_body_marker("formalization-state-conflict"),
        })
    }
}

struct StubFormalMethodAssetVersionRepository;

impl FormalMethodAssetVersionRepository for StubFormalMethodAssetVersionRepository {
    fn get_formal_method_asset_version_with_version(
        &self,
        _formal_version_ref: FormalMethodAssetVersionRef,
    ) -> Result<Option<Versioned<FormalMethodAssetVersion>>, MethodAssetRepositoryError> {
        Ok(None)
    }

    fn find_current_formal_method_asset_version(
        &self,
        _formalization_state_ref: FormalizationStateRef,
    ) -> Result<Option<Versioned<FormalMethodAssetVersion>>, MethodAssetRepositoryError> {
        Ok(None)
    }

    fn save_formal_method_asset_version(
        &self,
        _formal_version: FormalMethodAssetVersion,
        _expected_version: Option<MethodAssetExpectedVersion>,
        _uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<
        method_library_application::VersionedRef<FormalMethodAssetVersionRef>,
        MethodAssetRepositoryError,
    > {
        unreachable!("formal version save is not part of the conflict test")
    }
}

struct StubBasisSummaryRepository {
    value: Versioned<FormalizationBasisSummary>,
}

impl FormalizationBasisSummaryRepository for StubBasisSummaryRepository {
    fn get_formalization_basis_summary_with_version(
        &self,
        basis_summary_ref: FormalizationBasisSummaryRef,
    ) -> Result<Option<Versioned<FormalizationBasisSummary>>, MethodAssetRepositoryError> {
        if self.value.value.basis_summary_ref == basis_summary_ref {
            Ok(Some(self.value.clone()))
        } else {
            Ok(None)
        }
    }
}

struct StubBasisResolver;

impl FormalizationBasisResolverPort for StubBasisResolver {
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

struct StubPolicyDiagnosticBuilder;

impl MethodAssetPolicyDiagnosticBuilderPort for StubPolicyDiagnosticBuilder {
    fn build_formalization_eligibility_diagnostic(
        &self,
        _definition: &MethodAssetDefinition,
        _catalog_entry: &MethodAssetCatalogEntry,
        basis_resolution: &FormalizationBasisResolution,
        eligibility_rule_ref: FormalizationEligibilityRuleRef,
    ) -> Result<FormalizationEligibilityDiagnostic, MethodAssetRepositoryError> {
        Ok(FormalizationEligibilityDiagnostic {
            target_state_kind: FormalizationStateKind::Eligible,
            reason_summary: FormalizationStateReasonSummary::new(
                MethodLibrarySafeMarker::boundary(eligibility_rule_ref.clone().into()),
                basis_resolution.accepted_basis_summary_refs.clone(),
                None,
            ),
        })
    }

    fn build_formal_version_change_diagnostic(
        &self,
        _formal_version: &FormalMethodAssetVersion,
        _basis_summary_refs: &FormalizationBasisSummaryRefSet,
        _governance_basis_ref: Option<method_library_contracts::GovernanceBasisRef>,
        semantic_change_marker_ref: MethodLibrarySafeMarker,
    ) -> Result<method_library_application::FormalVersionChangeDiagnostic, MethodAssetRepositoryError>
    {
        Ok(method_library_application::FormalVersionChangeDiagnostic {
            accepted_change_marker_ref: semantic_change_marker_ref,
            blocking_reason_ref: None,
        })
    }
}

struct StubStoredResultRepository;

impl MethodAssetStoredOperationResultRepository for StubStoredResultRepository {
    fn find_command_result_by_idempotency(
        &self,
        _idempotency_key_ref: MethodAssetIdempotencyKeyRef,
        _dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> Result<Option<MethodAssetStoredOperationResult>, MethodAssetRepositoryError> {
        Ok(None)
    }

    fn get_stored_operation_result(
        &self,
        _stored_result_ref: MethodAssetStoredOperationResultRef,
    ) -> Result<Option<MethodAssetStoredOperationResult>, MethodAssetRepositoryError> {
        Ok(None)
    }

    fn save_command_result_for_idempotency(
        &self,
        _idempotency_key_ref: MethodAssetIdempotencyKeyRef,
        _dedup_scope_ref: MethodAssetDedupScopeRef,
        _operation_digest_ref: MethodAssetOperationDigestRef,
        _stored_result: MethodAssetStoredOperationResult,
        _uow: &mut dyn method_library_application::CommandUnitOfWork,
    ) -> Result<MethodAssetStoredOperationResultRef, MethodAssetRepositoryError> {
        unreachable!("version conflict should stay ephemeral")
    }
}

#[derive(Default)]
struct StubSupportRefFactory {
    nonce: u64,
}

impl StubSupportRefFactory {
    fn next(&mut self, prefix: &str) -> String {
        self.nonce += 1;
        format!("{prefix}:{}", self.nonce)
    }
}

impl MethodAssetFormalizationVersionSupportRefFactory for StubSupportRefFactory {
    fn formalization_version_dispatch_ref(&self) -> MethodAssetApplicationDispatchRef {
        MethodAssetApplicationDispatchRef::new("formalization-version-command-service")
    }

    fn new_api_entry_context_ref(&mut self) -> MethodAssetApiEntryContextRef {
        MethodAssetApiEntryContextRef::new(self.next("api-entry"))
    }

    fn build_formalization_version_replay_envelope(
        &mut self,
        _input: MethodAssetFormalizationVersionReplayEnvelopeFactoryInput,
    ) -> Result<
        MethodAssetFormalizationVersionReplayEnvelope,
        method_library_application::MethodAssetReplayEnvelopeBuildError,
    > {
        Ok(MethodAssetFormalizationVersionReplayEnvelope {
            operation_context_ref: MethodAssetOperationContextRef::new("operation-context:1"),
            idempotency_key_ref: MethodAssetIdempotencyKeyRef::new("idempotency:1"),
            operation_digest_ref: MethodAssetOperationDigestRef::new("operation-digest:1"),
            dedup_scope_ref: MethodAssetDedupScopeRef::new("dedup-scope:1"),
        })
    }

    fn new_stored_operation_result_ref(&mut self) -> MethodAssetStoredOperationResultRef {
        MethodAssetStoredOperationResultRef::new(self.next("stored-result"))
    }

    fn new_accepted_operation_summary_ref(&mut self) -> MethodAssetAcceptedOperationSummaryRef {
        MethodAssetAcceptedOperationSummaryRef::new(self.next("accepted-summary"))
    }

    fn new_safe_reject_reason_ref(&mut self) -> MethodAssetSafeRejectReasonRef {
        MethodAssetSafeRejectReasonRef::new(self.next("reject-reason"))
    }

    fn new_safe_ignore_reason_ref(&mut self) -> MethodAssetSafeIgnoreReasonRef {
        MethodAssetSafeIgnoreReasonRef::new(self.next("ignore-reason"))
    }

    fn new_effect_summary_ref(&mut self) -> MethodAssetEffectSummaryRef {
        MethodAssetEffectSummaryRef::new(self.next("effect-summary"))
    }

    fn new_replay_marker_ref(&mut self) -> MethodAssetReplayMarkerRef {
        MethodAssetReplayMarkerRef::new(self.next("replay-marker"))
    }

    fn new_formalization_state_ref(
        &mut self,
        _definition_ref: MethodAssetDefinitionRef,
        _catalog_entry_ref: MethodAssetCatalogEntryRef,
        _operation_context_ref: MethodAssetOperationContextRef,
        _operation_digest_ref: MethodAssetOperationDigestRef,
        _dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> FormalizationStateRef {
        FormalizationStateRef::new(self.next("formalization-state"))
    }

    fn new_formal_method_asset_version_ref(
        &mut self,
        _formalization_state_ref: FormalizationStateRef,
        _definition_ref: MethodAssetDefinitionRef,
        _catalog_entry_ref: MethodAssetCatalogEntryRef,
        _version_boundary_summary: method_library_contracts::FormalVersionBoundarySummary,
        _operation_context_ref: MethodAssetOperationContextRef,
        _operation_digest_ref: MethodAssetOperationDigestRef,
        _dedup_scope_ref: MethodAssetDedupScopeRef,
    ) -> FormalMethodAssetVersionRef {
        FormalMethodAssetVersionRef::new(self.next("formal-version"))
    }
}

#[test]
fn evaluate_flow_maps_save_version_conflict_to_rejected_surface() {
    let definition_kind = MethodAssetDefinitionKind::ProcessTemplate;
    let definition_ref = MethodAssetDefinitionRef::new("definition:1");
    let catalog_entry_ref = MethodAssetCatalogEntryRef::new("catalog:1");
    let state_ref = FormalizationStateRef::new("formalization-state:1");
    let basis_summary_ref = FormalizationBasisSummaryRef::new("basis:1");
    let definition = MethodAssetDefinition::create(
        definition_ref.clone(),
        MethodAssetIdentityKey::new(
            definition_kind,
            typed_ref(
                MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
                "namespace:1",
            ),
            typed_ref(
                MethodLibraryTypedBoundaryRefKind::RelatedMethodAssetRef,
                "anchor:1",
            ),
            method_library_contracts::CatalogScopeRef::new("scope:1"),
        ),
        definition_summary(definition_kind, "v1"),
    );
    let catalog_entry = MethodAssetCatalogEntry::create_for_definition(
        catalog_entry_ref.clone(),
        definition_ref.clone(),
        method_library_contracts::CatalogScopeRef::new("scope:1"),
        MethodAssetCatalogClassification::new(
            definition_kind,
            method_library_contracts::CatalogScopeRef::new("scope:1"),
            no_body_marker("classification"),
        ),
        method_library_contracts::MethodAssetApplicabilitySummary::new(
            method_library_contracts::CatalogScopeRef::new("scope:1"),
            no_body_marker("applicability"),
            [typed_ref(
                MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
                "context:1",
            )],
        ),
    )
    .expect("catalog entry should be valid");
    let current_state = FormalizationState::pending_for_definition(
        state_ref,
        definition_ref.clone(),
        catalog_entry_ref.clone(),
        FormalizationStateReasonSummary::new(
            no_body_marker("reason"),
            FormalizationBasisSummaryRefSet::from_refs([basis_summary_ref.clone()]),
            None,
        ),
    );
    let basis_summary = FormalizationBasisSummary::from_external_summary(
        basis_summary_ref.clone(),
        definition_ref.clone(),
        Some(catalog_entry_ref.clone()),
        ExternalSourceSummaryRef::new("external-summary:1"),
        FormalizationBasisSafeSummary::new(
            no_body_marker("basis-summary"),
            Some(ExternalSourceSummaryRef::new("external-summary:1")),
            None,
            None,
        ),
    );

    let facade = DefaultMethodAssetFormalizationVersionCommandFacade::new(
        Arc::new(StubDefinitionRepository {
            value: Versioned {
                value: definition,
                version: MethodAssetRepositoryVersion(1),
            },
        }),
        Arc::new(StubCatalogRepository {
            value: Versioned {
                value: catalog_entry,
                version: MethodAssetRepositoryVersion(1),
            },
        }),
        Arc::new(StubFormalizationStateRepository {
            value: Versioned {
                value: current_state,
                version: MethodAssetRepositoryVersion(2),
            },
        }),
        Arc::new(StubFormalMethodAssetVersionRepository),
        Arc::new(StubBasisSummaryRepository {
            value: Versioned {
                value: basis_summary,
                version: MethodAssetRepositoryVersion(1),
            },
        }),
        Arc::new(StubBasisResolver),
        Arc::new(StubPolicyDiagnosticBuilder),
        Arc::new(StubStoredResultRepository),
        Arc::new(StubUnitOfWork),
        Arc::new(Mutex::new(Box::new(StubSupportRefFactory::default()))),
    );

    let output = facade.dispatch_formalization_version_command(
        MethodAssetFormalizationVersionCommandDispatchInput {
            command_shell: command_shell(
                "evaluate-conflict",
                MethodLibraryTypedBoundaryRefKind::MethodAssetFormalizationEligibilityEvaluateIntent,
            ),
            command_source: MethodAssetFormalizationVersionCommandSource::EvaluateFormalizationEligibility(
                method_library_application::EvaluateFormalizationEligibilityCommandSource {
                    definition_ref,
                    catalog_entry_ref,
                    basis_summary_refs: FormalizationBasisSummaryRefSet::from_refs([
                        basis_summary_ref,
                    ]),
                    eligibility_rule_ref: FormalizationEligibilityRuleRef::new(
                        "eligibility-rule:1",
                    ),
                },
            ),
            api_entry_context_ref: MethodAssetApiEntryContextRef::new("api-entry:seed"),
            application_dispatch_ref: MethodAssetApplicationDispatchRef::new(
                "formalization-version-command-service",
            ),
        },
    );

    assert_eq!(
        output.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Rejected
    );
    assert!(output.accepted_summary_ref.is_none());
    assert!(output.rejected_reason_ref.is_some());
}
