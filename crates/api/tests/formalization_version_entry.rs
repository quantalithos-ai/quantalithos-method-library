use method_library_api::MethodAssetFormalizationVersionApiCommandHandlerEntry;
use method_library_application::UnitOfWork;
use method_library_application::ports::{
    MethodAssetCatalogEntryRepository, MethodAssetDefinitionRepository,
};
use method_library_contracts::fixtures::sample_command_shell;
use method_library_contracts::metadata::IdempotencyKey;
use method_library_contracts::{
    CatalogScopeRef, ExternalSourceSummaryRef, FormalizationBasisSafeSummary,
    FormalizationBasisSummaryRef, FormalizationBasisSummaryRefSet,
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

#[test]
fn api_entry_creates_context_and_delegates_to_formalization_facade() {
    let runtime = InMemoryMethodAssetFormalizationVersionRuntime::new();
    let api_entry = MethodAssetFormalizationVersionApiCommandHandlerEntry::new(
        runtime.facade(),
        runtime.support_ref_factory(),
    );

    let definition_kind = MethodAssetDefinitionKind::ProcessTemplate;
    let scope_ref = CatalogScopeRef::new("api-formalization-scope");
    let definition_ref = MethodAssetDefinitionRef::new("api-definition");
    let catalog_entry_ref =
        method_library_contracts::MethodAssetCatalogEntryRef::new("api-catalog");
    let identity_key = MethodAssetIdentityKey::new(
        definition_kind,
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            "api-namespace",
        ),
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::RelatedMethodAssetRef,
            "api-anchor",
        ),
        scope_ref.clone(),
    );
    let definition = MethodAssetDefinition::create(
        definition_ref.clone(),
        identity_key,
        MethodAssetDefinitionSummary::new(
            typed_ref(
                MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
                "api-definition-summary",
            ),
            definition_kind,
            typed_ref(
                MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
                "api-definition-title",
            ),
            None,
            no_body_marker("api-definition-summary"),
        ),
    );
    let catalog_entry = MethodAssetCatalogEntry::create_for_definition(
        catalog_entry_ref.clone(),
        definition_ref.clone(),
        scope_ref.clone(),
        MethodAssetCatalogClassification::new(
            definition_kind,
            scope_ref.clone(),
            no_body_marker("api-classification"),
        ),
        method_library_contracts::MethodAssetApplicabilitySummary::new(
            scope_ref,
            no_body_marker("api-applicability"),
            [typed_ref(
                MethodLibraryTypedBoundaryRefKind::ConsumptionContextRef,
                "api-context",
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

    runtime.seed_basis_summary(FormalizationBasisSummary::from_external_summary(
        FormalizationBasisSummaryRef::new("api-basis"),
        definition_ref,
        Some(catalog_entry_ref.clone()),
        ExternalSourceSummaryRef::new("api-external-summary"),
        FormalizationBasisSafeSummary::new(
            no_body_marker("api-basis-summary"),
            Some(ExternalSourceSummaryRef::new("api-external-summary")),
            None,
            None,
        ),
    ));

    let mut command_shell = sample_command_shell();
    command_shell.capability_kind = MethodLibraryCapabilityKind::FormalizationVersion;
    command_shell.boundary_ref = typed_ref(
        MethodLibraryTypedBoundaryRefKind::MethodAssetFormalizationEligibilityEvaluateIntent,
        "api-evaluate-boundary",
    );
    command_shell.metadata.request.idempotency_key = Some(IdempotencyKey::new("api-evaluate"));

    let output = api_entry.handle_formalization_version_command(
        command_shell,
        method_library_application::MethodAssetFormalizationVersionCommandSource::EvaluateFormalizationEligibility(
            method_library_application::EvaluateFormalizationEligibilityCommandSource {
                definition_ref: MethodAssetDefinitionRef::new("api-definition"),
                catalog_entry_ref,
                basis_summary_refs: FormalizationBasisSummaryRefSet::from_refs([
                    FormalizationBasisSummaryRef::new("api-basis"),
                ]),
                eligibility_rule_ref: method_library_contracts::FormalizationEligibilityRuleRef::new(
                    "api-eligibility-rule",
                ),
            },
        ),
    );

    assert_eq!(
        output.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );
    assert!(output.accepted_summary_ref.is_some());
}
