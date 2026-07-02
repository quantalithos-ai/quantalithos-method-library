use method_library_application::ports::{
    MethodAssetCatalogEntryRepository, MethodAssetDefinitionRepository,
};
use method_library_application::{
    AdjustDefinitionCommandSource, EstablishDefinitionCommandSource,
    MethodAssetDefinitionCatalogCommandDispatchInput, MethodAssetDefinitionCatalogCommandSource,
    ReclassifyCatalogEntryCommandSource, RegisterCatalogEntryCommandSource,
    RetireCatalogEntryCommandSource, RetireDefinitionCommandSource, UnitOfWork,
};
use method_library_contracts::fixtures::sample_command_shell;
use method_library_contracts::metadata::IdempotencyKey;
use method_library_contracts::{
    CatalogScopeRef, ExternalSourceSummaryRef, ExternalSourceSummaryRefSet,
    MethodAssetApplicabilitySummary, MethodAssetCatalogClassification,
    MethodAssetCatalogEntryRefSet, MethodAssetCatalogEntryStatus, MethodAssetDefinitionKind,
    MethodAssetDefinitionSummary, MethodAssetIdentityKey, MethodLibrarySafeMarker,
    MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};
use method_library_domain::MethodAssetDefinitionLifecycle;
use method_library_infra::InMemoryMethodAssetDefinitionCatalogRuntime;

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
    shell.boundary_ref = typed_ref(boundary_kind, format!("boundary:{idempotency_key}"));
    shell.metadata.request.idempotency_key = Some(IdempotencyKey::new(idempotency_key));
    shell
}

fn catalog_scope_ref(label: &str) -> CatalogScopeRef {
    CatalogScopeRef::new(format!("catalog-scope:{label}"))
}

fn identity_key(
    definition_kind: MethodAssetDefinitionKind,
    scope_ref: &CatalogScopeRef,
    label: &str,
) -> MethodAssetIdentityKey {
    MethodAssetIdentityKey::new(
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
    )
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
        Some(typed_ref(
            MethodLibraryTypedBoundaryRefKind::TraceSubjectRef,
            format!("definition-description:{label}"),
        )),
        no_body_marker(&format!("definition-summary:{label}")),
    )
}

fn source_summary_refs(label: &str) -> ExternalSourceSummaryRefSet {
    ExternalSourceSummaryRefSet::from_refs([ExternalSourceSummaryRef::new(format!(
        "external-summary:{label}"
    ))])
}

fn catalog_classification(
    definition_kind: MethodAssetDefinitionKind,
    scope_ref: &CatalogScopeRef,
    label: &str,
) -> MethodAssetCatalogClassification {
    MethodAssetCatalogClassification::new(
        definition_kind,
        scope_ref.clone(),
        no_body_marker(&format!("catalog-classification:{label}")),
    )
}

fn applicability_summary(
    scope_ref: &CatalogScopeRef,
    label: &str,
) -> MethodAssetApplicabilitySummary {
    MethodAssetApplicabilitySummary::new(
        scope_ref.clone(),
        no_body_marker(&format!("applicability-summary:{label}")),
        [typed_ref(
            MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
            format!("applicability-context:{label}"),
        )],
    )
}

fn load_definition(
    runtime: &InMemoryMethodAssetDefinitionCatalogRuntime,
    identity_key: &MethodAssetIdentityKey,
) -> method_library_application::Versioned<method_library_domain::MethodAssetDefinition> {
    runtime
        .definition_repository()
        .find_definition_by_identity_key(identity_key.clone())
        .expect("definition lookup should succeed")
        .expect("definition should exist")
}

fn load_catalog_entry(
    runtime: &InMemoryMethodAssetDefinitionCatalogRuntime,
    definition_ref: &method_library_contracts::MethodAssetDefinitionRef,
    scope_ref: &CatalogScopeRef,
) -> method_library_application::Versioned<method_library_domain::MethodAssetCatalogEntry> {
    runtime
        .catalog_repository()
        .find_catalog_entry_by_definition_scope(definition_ref.clone(), scope_ref.clone())
        .expect("catalog lookup should succeed")
        .expect("catalog entry should exist")
}

#[test]
fn definition_lifecycle_persists_and_duplicate_replay_does_not_rerun_mutation() {
    let runtime = InMemoryMethodAssetDefinitionCatalogRuntime::new();
    let facade = runtime.facade();
    let support_ref_factory = runtime.support_ref_factory();

    let definition_kind = MethodAssetDefinitionKind::ProcessTemplate;
    let scope_ref = catalog_scope_ref("definition");
    let identity_key = identity_key(definition_kind, &scope_ref, "definition");
    let source = MethodAssetDefinitionCatalogCommandSource::EstablishDefinition(
        EstablishDefinitionCommandSource {
            definition_kind,
            identity_key: identity_key.clone(),
            definition_summary: definition_summary(definition_kind, "v1"),
            source_summary_refs: source_summary_refs("v1"),
            preaccepted_catalog_entry_refs: MethodAssetCatalogEntryRefSet::new(),
        },
    );
    let shell = command_shell(
        "definition-establish",
        MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionEstablishIntent,
    );
    let first = facade.dispatch_definition_catalog_command(
        MethodAssetDefinitionCatalogCommandDispatchInput {
            command_shell: shell.clone(),
            command_source: source.clone(),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.definition_catalog_dispatch_ref()
            },
        },
    );
    assert_eq!(
        first.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );

    let duplicate = facade.dispatch_definition_catalog_command(
        MethodAssetDefinitionCatalogCommandDispatchInput {
            command_shell: shell,
            command_source: source,
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.definition_catalog_dispatch_ref()
            },
        },
    );
    assert_eq!(duplicate.stored_result_ref, first.stored_result_ref);

    let established = load_definition(&runtime, &identity_key);
    assert_eq!(established.version.0, 1);
    assert_eq!(
        established.value.definition_lifecycle,
        MethodAssetDefinitionLifecycle::Active
    );

    let adjusted = facade.dispatch_definition_catalog_command(
        MethodAssetDefinitionCatalogCommandDispatchInput {
            command_shell: command_shell(
                "definition-adjust",
                MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionAdjustIntent,
            ),
            command_source: MethodAssetDefinitionCatalogCommandSource::AdjustDefinition(
                AdjustDefinitionCommandSource {
                    definition_ref: established.value.definition_ref.clone(),
                    replacement_definition_summary: definition_summary(definition_kind, "v2"),
                    replacement_source_summary_refs: source_summary_refs("v2"),
                },
            ),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.definition_catalog_dispatch_ref()
            },
        },
    );
    assert_eq!(
        adjusted.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );

    let adjusted_definition = load_definition(&runtime, &identity_key);
    assert_eq!(adjusted_definition.version.0, 2);
    assert_eq!(
        adjusted_definition.value.definition_lifecycle,
        MethodAssetDefinitionLifecycle::Active
    );

    let retired = facade.dispatch_definition_catalog_command(
        MethodAssetDefinitionCatalogCommandDispatchInput {
            command_shell: command_shell(
                "definition-retire",
                MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionRetireIntent,
            ),
            command_source: MethodAssetDefinitionCatalogCommandSource::RetireDefinition(
                RetireDefinitionCommandSource {
                    definition_ref: adjusted_definition.value.definition_ref.clone(),
                    retirement_marker_ref: no_body_marker("definition-retire"),
                },
            ),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.definition_catalog_dispatch_ref()
            },
        },
    );
    assert_eq!(
        retired.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );

    let retired_definition = load_definition(&runtime, &identity_key);
    assert_eq!(retired_definition.version.0, 3);
    assert_eq!(
        retired_definition.value.definition_lifecycle,
        MethodAssetDefinitionLifecycle::Retired
    );

    let rejected = facade.dispatch_definition_catalog_command(
        MethodAssetDefinitionCatalogCommandDispatchInput {
            command_shell: command_shell(
                "definition-adjust-rejected",
                MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionAdjustIntent,
            ),
            command_source: MethodAssetDefinitionCatalogCommandSource::AdjustDefinition(
                AdjustDefinitionCommandSource {
                    definition_ref: retired_definition.value.definition_ref.clone(),
                    replacement_definition_summary: definition_summary(definition_kind, "v3"),
                    replacement_source_summary_refs: source_summary_refs("v3"),
                },
            ),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.definition_catalog_dispatch_ref()
            },
        },
    );
    assert_eq!(
        rejected.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Rejected
    );

    let replayed_rejection = facade.dispatch_definition_catalog_command(
        MethodAssetDefinitionCatalogCommandDispatchInput {
            command_shell: command_shell(
                "definition-adjust-rejected",
                MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionAdjustIntent,
            ),
            command_source: MethodAssetDefinitionCatalogCommandSource::AdjustDefinition(
                AdjustDefinitionCommandSource {
                    definition_ref: retired_definition.value.definition_ref.clone(),
                    replacement_definition_summary: definition_summary(definition_kind, "v3"),
                    replacement_source_summary_refs: source_summary_refs("v3"),
                },
            ),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.definition_catalog_dispatch_ref()
            },
        },
    );
    assert_eq!(
        replayed_rejection.stored_result_ref,
        rejected.stored_result_ref
    );
}

#[test]
fn catalog_status_persists_visible_to_retired_and_replays_duplicates() {
    let runtime = InMemoryMethodAssetDefinitionCatalogRuntime::new();
    let facade = runtime.facade();
    let support_ref_factory = runtime.support_ref_factory();

    let definition_kind = MethodAssetDefinitionKind::AiObjective;
    let scope_ref = catalog_scope_ref("catalog");
    let identity_key = identity_key(definition_kind, &scope_ref, "catalog");
    let establish = facade.dispatch_definition_catalog_command(
        MethodAssetDefinitionCatalogCommandDispatchInput {
            command_shell: command_shell(
                "catalog-definition-establish",
                MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionEstablishIntent,
            ),
            command_source: MethodAssetDefinitionCatalogCommandSource::EstablishDefinition(
                EstablishDefinitionCommandSource {
                    definition_kind,
                    identity_key: identity_key.clone(),
                    definition_summary: definition_summary(definition_kind, "catalog-definition"),
                    source_summary_refs: source_summary_refs("catalog-definition"),
                    preaccepted_catalog_entry_refs: MethodAssetCatalogEntryRefSet::new(),
                },
            ),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.definition_catalog_dispatch_ref()
            },
        },
    );
    assert_eq!(
        establish.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );

    let definition = load_definition(&runtime, &identity_key);
    let register_scope = catalog_scope_ref("register");
    let register_source = MethodAssetDefinitionCatalogCommandSource::RegisterCatalogEntry(
        RegisterCatalogEntryCommandSource {
            definition_ref: definition.value.definition_ref.clone(),
            catalog_scope_ref: register_scope.clone(),
            catalog_classification: catalog_classification(
                definition_kind,
                &register_scope,
                "register-v1",
            ),
            applicability_summary: applicability_summary(&register_scope, "register-v1"),
        },
    );
    let register_shell = command_shell(
        "catalog-register",
        MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntryRegisterIntent,
    );
    let registered = facade.dispatch_definition_catalog_command(
        MethodAssetDefinitionCatalogCommandDispatchInput {
            command_shell: register_shell.clone(),
            command_source: register_source.clone(),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.definition_catalog_dispatch_ref()
            },
        },
    );
    assert_eq!(
        registered.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );

    let register_duplicate = facade.dispatch_definition_catalog_command(
        MethodAssetDefinitionCatalogCommandDispatchInput {
            command_shell: register_shell,
            command_source: register_source,
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.definition_catalog_dispatch_ref()
            },
        },
    );
    assert_eq!(
        register_duplicate.stored_result_ref,
        registered.stored_result_ref
    );

    let catalog_entry =
        load_catalog_entry(&runtime, &definition.value.definition_ref, &register_scope);
    assert_eq!(catalog_entry.version.0, 1);
    assert_eq!(
        catalog_entry.value.catalog_status,
        MethodAssetCatalogEntryStatus::Visible
    );

    let reclassified = facade.dispatch_definition_catalog_command(
        MethodAssetDefinitionCatalogCommandDispatchInput {
            command_shell: command_shell(
                "catalog-reclassify",
                MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntryReclassifyIntent,
            ),
            command_source: MethodAssetDefinitionCatalogCommandSource::ReclassifyCatalogEntry(
                ReclassifyCatalogEntryCommandSource {
                    catalog_entry_ref: catalog_entry.value.catalog_entry_ref.clone(),
                    new_catalog_classification: catalog_classification(
                        definition_kind,
                        &register_scope,
                        "register-v2",
                    ),
                    new_applicability_summary: applicability_summary(
                        &register_scope,
                        "register-v2",
                    ),
                },
            ),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.definition_catalog_dispatch_ref()
            },
        },
    );
    assert_eq!(
        reclassified.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );

    let reclassified_entry =
        load_catalog_entry(&runtime, &definition.value.definition_ref, &register_scope);
    assert_eq!(reclassified_entry.version.0, 2);
    assert_eq!(
        reclassified_entry.value.catalog_status,
        MethodAssetCatalogEntryStatus::Visible
    );
    assert_eq!(
        reclassified_entry
            .value
            .applicability_summary
            .applicable_context_refs
            .first()
            .expect("context ref should exist")
            .as_public_ref(),
        "applicability-context:register-v2"
    );

    let retired = facade.dispatch_definition_catalog_command(
        MethodAssetDefinitionCatalogCommandDispatchInput {
            command_shell: command_shell(
                "catalog-retire",
                MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntryRetireIntent,
            ),
            command_source: MethodAssetDefinitionCatalogCommandSource::RetireCatalogEntry(
                RetireCatalogEntryCommandSource {
                    catalog_entry_ref: reclassified_entry.value.catalog_entry_ref.clone(),
                    retirement_marker_ref: no_body_marker("catalog-retire"),
                },
            ),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.definition_catalog_dispatch_ref()
            },
        },
    );
    assert_eq!(
        retired.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );

    let retired_entry =
        load_catalog_entry(&runtime, &definition.value.definition_ref, &register_scope);
    assert_eq!(retired_entry.version.0, 3);
    assert_eq!(
        retired_entry.value.catalog_status,
        MethodAssetCatalogEntryStatus::Retired
    );

    let rejected_reclassify = facade.dispatch_definition_catalog_command(
        MethodAssetDefinitionCatalogCommandDispatchInput {
            command_shell: command_shell(
                "catalog-reclassify-rejected",
                MethodLibraryTypedBoundaryRefKind::MethodAssetCatalogEntryReclassifyIntent,
            ),
            command_source: MethodAssetDefinitionCatalogCommandSource::ReclassifyCatalogEntry(
                ReclassifyCatalogEntryCommandSource {
                    catalog_entry_ref: retired_entry.value.catalog_entry_ref.clone(),
                    new_catalog_classification: catalog_classification(
                        definition_kind,
                        &register_scope,
                        "register-v3",
                    ),
                    new_applicability_summary: applicability_summary(
                        &register_scope,
                        "register-v3",
                    ),
                },
            ),
            api_entry_context_ref: {
                let mut factory = support_ref_factory.lock().expect("factory lock");
                factory.new_api_entry_context_ref()
            },
            application_dispatch_ref: {
                let factory = support_ref_factory.lock().expect("factory lock");
                factory.definition_catalog_dispatch_ref()
            },
        },
    );
    assert_eq!(
        rejected_reclassify.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Rejected
    );
}

#[test]
fn rollback_hides_directly_staged_definition_write() {
    let runtime = InMemoryMethodAssetDefinitionCatalogRuntime::new();
    let support_ref_factory = runtime.support_ref_factory();

    let scope_ref = catalog_scope_ref("rollback");
    let identity_key = identity_key(
        MethodAssetDefinitionKind::LifecycleModel,
        &scope_ref,
        "rollback",
    );
    let command_shell = command_shell(
        "rollback-establish",
        MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionEstablishIntent,
    );
    let command_source = MethodAssetDefinitionCatalogCommandSource::EstablishDefinition(
        EstablishDefinitionCommandSource {
            definition_kind: MethodAssetDefinitionKind::LifecycleModel,
            identity_key: identity_key.clone(),
            definition_summary: definition_summary(
                MethodAssetDefinitionKind::LifecycleModel,
                "rollback",
            ),
            source_summary_refs: source_summary_refs("rollback"),
            preaccepted_catalog_entry_refs: MethodAssetCatalogEntryRefSet::new(),
        },
    );

    let replay_envelope = {
        let mut factory = support_ref_factory.lock().expect("factory lock");
        let api_entry_context_ref = factory.new_api_entry_context_ref();
        let application_dispatch_ref = factory.definition_catalog_dispatch_ref();
        factory
            .build_definition_catalog_replay_envelope(
                method_library_application::MethodAssetDefinitionCatalogReplayEnvelopeFactoryInput {
                    command_shell,
                    command_source: command_source.clone(),
                    selector:
                        method_library_application::MethodAssetDefinitionCatalogCommandSelector::EstablishDefinition,
                    api_entry_context_ref,
                    application_dispatch_ref,
                },
            )
            .expect("replay envelope should build")
    };

    let definition_ref = {
        let mut factory = support_ref_factory.lock().expect("factory lock");
        factory.new_definition_ref(
            identity_key.clone(),
            replay_envelope.operation_context_ref.clone(),
            replay_envelope.operation_digest_ref.clone(),
            replay_envelope.dedup_scope_ref.clone(),
        )
    };

    let definition = method_library_domain::MethodAssetDefinition::create(
        definition_ref,
        identity_key.clone(),
        definition_summary(MethodAssetDefinitionKind::LifecycleModel, "rollback"),
    );

    let mut uow = runtime.unit_of_work().begin_command_uow();
    runtime
        .definition_repository()
        .save_definition(definition, None, uow.as_mut())
        .expect("definition save should stage successfully");
    uow.rollback().expect("rollback should succeed");

    assert!(
        runtime
            .definition_repository()
            .find_definition_by_identity_key(identity_key)
            .expect("lookup should succeed")
            .is_none()
    );
}
