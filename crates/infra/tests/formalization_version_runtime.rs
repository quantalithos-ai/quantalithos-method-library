use method_library_application::UnitOfWork;
use method_library_application::ports::{
    FormalMethodAssetVersionRepository, FormalizationStateRepository,
    MethodAssetCatalogEntryRepository, MethodAssetDefinitionRepository,
    MethodAssetStoredOperationResultRepository,
};
use method_library_contracts::fixtures::sample_command_shell;
use method_library_contracts::metadata::IdempotencyKey;
use method_library_contracts::{
    CatalogScopeRef, ExternalSourceSummaryRef, FormalMethodAssetVersionRef,
    FormalizationBasisSafeSummary, FormalizationBasisSummaryRef, FormalizationBasisSummaryRefSet,
    MethodAssetCatalogClassification, MethodAssetDefinitionKind, MethodAssetDefinitionRef,
    MethodAssetDefinitionSummary, MethodAssetIdentityKey, MethodLibraryCapabilityKind,
    MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};
use method_library_domain::{
    FormalizationBasisSummary, MethodAssetCatalogEntry, MethodAssetDefinition,
};
use method_library_infra::InMemoryMethodAssetFormalizationVersionRuntime;

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

fn seed_definition_and_catalog(
    runtime: &InMemoryMethodAssetFormalizationVersionRuntime,
    label: &str,
) -> (
    MethodAssetIdentityKey,
    MethodAssetDefinitionRef,
    method_library_contracts::MethodAssetCatalogEntryRef,
    CatalogScopeRef,
) {
    let definition_kind = MethodAssetDefinitionKind::ProcessTemplate;
    let scope_ref = CatalogScopeRef::new(format!("scope:{label}"));
    let identity_key = MethodAssetIdentityKey::new(
        definition_kind,
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            format!("identity-namespace:{label}"),
        ),
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::RelatedMethodAssetRef,
            format!("identity-anchor:{label}"),
        ),
        scope_ref.clone(),
    );
    let definition_ref = MethodAssetDefinitionRef::new(format!("definition:{label}"));
    let catalog_entry_ref =
        method_library_contracts::MethodAssetCatalogEntryRef::new(format!("catalog:{label}"));
    let definition = MethodAssetDefinition::create(
        definition_ref.clone(),
        identity_key.clone(),
        definition_summary(definition_kind, label),
    );
    let catalog_entry = MethodAssetCatalogEntry::create_for_definition(
        catalog_entry_ref.clone(),
        definition_ref.clone(),
        scope_ref.clone(),
        MethodAssetCatalogClassification::new(
            definition_kind,
            scope_ref.clone(),
            no_body_marker(&format!("classification:{label}")),
        ),
        method_library_contracts::MethodAssetApplicabilitySummary::new(
            scope_ref.clone(),
            no_body_marker(&format!("applicability:{label}")),
            [typed_ref(
                MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
                format!("context:{label}"),
            )],
        ),
    )
    .expect("catalog entry should be valid");

    let mut uow = runtime.unit_of_work().begin_command_uow();
    runtime
        .definition_repository()
        .save_definition(definition, None, uow.as_mut())
        .expect("definition save should succeed");
    runtime
        .catalog_repository()
        .save_catalog_entry(catalog_entry, None, uow.as_mut())
        .expect("catalog save should succeed");
    uow.commit().expect("seed commit should succeed");

    (identity_key, definition_ref, catalog_entry_ref, scope_ref)
}

fn seed_basis_summary(
    runtime: &InMemoryMethodAssetFormalizationVersionRuntime,
    definition_ref: &MethodAssetDefinitionRef,
    catalog_entry_ref: &method_library_contracts::MethodAssetCatalogEntryRef,
    label: &str,
) -> FormalizationBasisSummaryRef {
    let basis_summary_ref = FormalizationBasisSummaryRef::new(format!("basis:{label}"));
    runtime.seed_basis_summary(FormalizationBasisSummary::from_external_summary(
        basis_summary_ref.clone(),
        definition_ref.clone(),
        Some(catalog_entry_ref.clone()),
        ExternalSourceSummaryRef::new(format!("external-summary:{label}")),
        FormalizationBasisSafeSummary::new(
            no_body_marker(&format!("basis-summary:{label}")),
            Some(ExternalSourceSummaryRef::new(format!(
                "external-summary:{label}"
            ))),
            None,
            None,
        ),
    ));
    basis_summary_ref
}

fn load_state(
    runtime: &InMemoryMethodAssetFormalizationVersionRuntime,
    definition_ref: &MethodAssetDefinitionRef,
    catalog_entry_ref: &method_library_contracts::MethodAssetCatalogEntryRef,
) -> method_library_application::Versioned<method_library_domain::FormalizationState> {
    runtime
        .formalization_state_repository()
        .find_formalization_state_by_definition_catalog(
            definition_ref.clone(),
            catalog_entry_ref.clone(),
        )
        .expect("state lookup should succeed")
        .expect("state should exist")
}

fn load_version(
    runtime: &InMemoryMethodAssetFormalizationVersionRuntime,
    formal_version_ref: &FormalMethodAssetVersionRef,
) -> method_library_application::Versioned<method_library_domain::FormalMethodAssetVersion> {
    runtime
        .formal_method_asset_version_repository()
        .get_formal_method_asset_version_with_version(formal_version_ref.clone())
        .expect("formal version lookup should succeed")
        .expect("formal version should exist")
}

#[test]
fn evaluate_replays_duplicate_without_rerunning_state_mutation() {
    let runtime = InMemoryMethodAssetFormalizationVersionRuntime::new();
    let facade = runtime.facade();
    let support_ref_factory = runtime.support_ref_factory();
    let (_identity_key, definition_ref, catalog_entry_ref, _scope_ref) =
        seed_definition_and_catalog(&runtime, "evaluate");
    let basis_summary_ref =
        seed_basis_summary(&runtime, &definition_ref, &catalog_entry_ref, "evaluate");

    let source = method_library_application::MethodAssetFormalizationVersionCommandSource::EvaluateFormalizationEligibility(
        method_library_application::EvaluateFormalizationEligibilityCommandSource {
            definition_ref: definition_ref.clone(),
            catalog_entry_ref: catalog_entry_ref.clone(),
            basis_summary_refs: FormalizationBasisSummaryRefSet::from_refs([
                basis_summary_ref,
            ]),
            eligibility_rule_ref: method_library_contracts::FormalizationEligibilityRuleRef::new(
                "eligibility-rule:evaluate",
            ),
        },
    );
    let shell = command_shell(
        "evaluate-eligibility",
        MethodLibraryTypedBoundaryRefKind::MethodAssetFormalizationEligibilityEvaluateIntent,
    );

    let first = facade.dispatch_formalization_version_command(
        method_library_application::MethodAssetFormalizationVersionCommandDispatchInput {
            command_shell: shell.clone(),
            command_source: source.clone(),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.formalization_version_dispatch_ref()
            },
        },
    );
    assert_eq!(
        first.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );

    let duplicate = facade.dispatch_formalization_version_command(
        method_library_application::MethodAssetFormalizationVersionCommandDispatchInput {
            command_shell: shell,
            command_source: source,
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.formalization_version_dispatch_ref()
            },
        },
    );
    assert_eq!(duplicate.stored_result_ref, first.stored_result_ref);

    let saved_state = load_state(&runtime, &definition_ref, &catalog_entry_ref);
    assert_eq!(saved_state.version.0, 1);
    assert_eq!(
        saved_state.value.state_kind,
        method_library_contracts::FormalizationStateKind::Eligible
    );
}

#[test]
fn duplicate_lookup_integrity_failure_returns_conflict_surface() {
    let runtime = InMemoryMethodAssetFormalizationVersionRuntime::new();
    let facade = runtime.facade();
    let support_ref_factory = runtime.support_ref_factory();
    let (_identity_key, definition_ref, catalog_entry_ref, _scope_ref) =
        seed_definition_and_catalog(&runtime, "integrity");
    let basis_summary_ref =
        seed_basis_summary(&runtime, &definition_ref, &catalog_entry_ref, "integrity");

    let source = method_library_application::MethodAssetFormalizationVersionCommandSource::EvaluateFormalizationEligibility(
        method_library_application::EvaluateFormalizationEligibilityCommandSource {
            definition_ref: definition_ref.clone(),
            catalog_entry_ref: catalog_entry_ref.clone(),
            basis_summary_refs: FormalizationBasisSummaryRefSet::from_refs([
                basis_summary_ref,
            ]),
            eligibility_rule_ref: method_library_contracts::FormalizationEligibilityRuleRef::new(
                "eligibility-rule:integrity",
            ),
        },
    );
    let shell = command_shell(
        "evaluate-integrity",
        MethodLibraryTypedBoundaryRefKind::MethodAssetFormalizationEligibilityEvaluateIntent,
    );

    let first = facade.dispatch_formalization_version_command(
        method_library_application::MethodAssetFormalizationVersionCommandDispatchInput {
            command_shell: shell.clone(),
            command_source: source.clone(),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.formalization_version_dispatch_ref()
            },
        },
    );
    assert_eq!(
        first.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );

    runtime.remove_stored_result(&first.stored_result_ref);

    let conflict = facade.dispatch_formalization_version_command(
        method_library_application::MethodAssetFormalizationVersionCommandDispatchInput {
            command_shell: shell,
            command_source: source,
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.formalization_version_dispatch_ref()
            },
        },
    );
    assert_eq!(
        conflict.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Conflict
    );
    assert!(conflict.rejected_reason_ref.is_some());
}

#[test]
fn retire_formal_version_reads_back_after_commit_unknown_without_future_owner_precheck() {
    let runtime = InMemoryMethodAssetFormalizationVersionRuntime::new();
    let facade = runtime.facade();
    let support_ref_factory = runtime.support_ref_factory();
    let (_identity_key, definition_ref, catalog_entry_ref, _scope_ref) =
        seed_definition_and_catalog(&runtime, "retire");
    let basis_summary_ref =
        seed_basis_summary(&runtime, &definition_ref, &catalog_entry_ref, "retire");

    let evaluate_output = facade.dispatch_formalization_version_command(
        method_library_application::MethodAssetFormalizationVersionCommandDispatchInput {
            command_shell: command_shell(
                "retire-evaluate",
                MethodLibraryTypedBoundaryRefKind::MethodAssetFormalizationEligibilityEvaluateIntent,
            ),
            command_source: method_library_application::MethodAssetFormalizationVersionCommandSource::EvaluateFormalizationEligibility(
                method_library_application::EvaluateFormalizationEligibilityCommandSource {
                    definition_ref: definition_ref.clone(),
                    catalog_entry_ref: catalog_entry_ref.clone(),
                    basis_summary_refs: FormalizationBasisSummaryRefSet::from_refs([
                        basis_summary_ref.clone(),
                    ]),
                    eligibility_rule_ref: method_library_contracts::FormalizationEligibilityRuleRef::new(
                        "eligibility-rule:retire",
                    ),
                },
            ),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.formalization_version_dispatch_ref()
            },
        },
    );
    assert_eq!(
        evaluate_output.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );

    let saved_state = load_state(&runtime, &definition_ref, &catalog_entry_ref);
    let version_boundary_summary = method_library_contracts::FormalVersionBoundarySummary::new(
        no_body_marker("formal-version-boundary"),
        definition_ref.clone(),
        catalog_entry_ref.clone(),
        saved_state.value.formalization_state_ref.clone(),
        FormalizationBasisSummaryRefSet::from_refs([basis_summary_ref]),
    );
    let establish_output = facade.dispatch_formalization_version_command(
        method_library_application::MethodAssetFormalizationVersionCommandDispatchInput {
            command_shell: command_shell(
                "retire-establish",
                MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersionEstablishIntent,
            ),
            command_source: method_library_application::MethodAssetFormalizationVersionCommandSource::EstablishFormalVersion(
                method_library_application::EstablishFormalMethodAssetVersionCommandSource {
                    formalization_state_ref: saved_state.value.formalization_state_ref.clone(),
                    definition_ref: definition_ref.clone(),
                    catalog_entry_ref: catalog_entry_ref.clone(),
                    version_boundary_summary,
                },
            ),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.formalization_version_dispatch_ref()
            },
        },
    );
    assert_eq!(
        establish_output.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );

    let current_version = runtime
        .formal_method_asset_version_repository()
        .find_current_formal_method_asset_version(saved_state.value.formalization_state_ref.clone())
        .expect("current version lookup should succeed")
        .expect("formal version should exist");

    runtime.simulate_commit_unknown_once();

    let retire_output = facade.dispatch_formalization_version_command(
        method_library_application::MethodAssetFormalizationVersionCommandDispatchInput {
            command_shell: command_shell(
                "retire-version",
                MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersionRetireIntent,
            ),
            command_source: method_library_application::MethodAssetFormalizationVersionCommandSource::RetireFormalVersion(
                method_library_application::RetireFormalMethodAssetVersionCommandSource {
                    formal_version_ref: current_version.value.formal_version_ref.clone(),
                    retirement_marker_ref: no_body_marker("formal-version-retire"),
                },
            ),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.formalization_version_dispatch_ref()
            },
        },
    );
    assert_eq!(
        retire_output.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );

    let retired_version = load_version(&runtime, &current_version.value.formal_version_ref);
    assert_eq!(
        retired_version.value.version_state,
        method_library_contracts::FormalMethodAssetVersionState::Retired
    );
    assert!(
        runtime
            .stored_result_repository()
            .get_stored_operation_result(retire_output.stored_result_ref.clone())
            .expect("stored result lookup should succeed")
            .is_some()
    );

    let rejected_again = facade.dispatch_formalization_version_command(
        method_library_application::MethodAssetFormalizationVersionCommandDispatchInput {
            command_shell: command_shell(
                "retire-version-again",
                MethodLibraryTypedBoundaryRefKind::FormalMethodAssetVersionRetireIntent,
            ),
            command_source: method_library_application::MethodAssetFormalizationVersionCommandSource::RetireFormalVersion(
                method_library_application::RetireFormalMethodAssetVersionCommandSource {
                    formal_version_ref: current_version.value.formal_version_ref.clone(),
                    retirement_marker_ref: no_body_marker("formal-version-retire-again"),
                },
            ),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.formalization_version_dispatch_ref()
            },
        },
    );
    assert_eq!(
        rejected_again.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Rejected
    );
}
