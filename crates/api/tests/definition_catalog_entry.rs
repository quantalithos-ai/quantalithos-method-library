use method_library_api::MethodAssetApiCommandHandlerEntry;
use method_library_application::{
    EstablishDefinitionCommandSource, MethodAssetDefinitionCatalogCommandSource,
};
use method_library_contracts::fixtures::sample_command_shell;
use method_library_contracts::metadata::IdempotencyKey;
use method_library_contracts::{
    CatalogScopeRef, ExternalSourceSummaryRefSet, MethodAssetCatalogEntryRefSet,
    MethodAssetDefinitionKind, MethodAssetDefinitionSummary, MethodAssetIdentityKey,
    MethodLibrarySafeMarker, MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind,
};
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

#[test]
fn api_entry_creates_context_and_delegates_to_facade() {
    let runtime = InMemoryMethodAssetDefinitionCatalogRuntime::new();
    let api_entry =
        MethodAssetApiCommandHandlerEntry::new(runtime.facade(), runtime.support_ref_factory());

    let scope_ref = CatalogScopeRef::new("api-scope");
    let identity_key = MethodAssetIdentityKey::new(
        MethodAssetDefinitionKind::SpemMethodContent,
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
            "api-identity-namespace",
        ),
        typed_ref(
            MethodLibraryTypedBoundaryRefKind::RelatedMethodAssetRef,
            "api-identity-anchor",
        ),
        scope_ref,
    );

    let mut command_shell = sample_command_shell();
    command_shell.boundary_ref = typed_ref(
        MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionEstablishIntent,
        "api-establish-boundary",
    );
    command_shell.metadata.request.idempotency_key = Some(IdempotencyKey::new("api-establish"));

    let output = api_entry.handle_definition_catalog_command(
        command_shell,
        MethodAssetDefinitionCatalogCommandSource::EstablishDefinition(
            EstablishDefinitionCommandSource {
                definition_kind: MethodAssetDefinitionKind::SpemMethodContent,
                identity_key,
                definition_summary: MethodAssetDefinitionSummary::new(
                    typed_ref(
                        MethodLibraryTypedBoundaryRefKind::MethodAssetDefinition,
                        "api-definition-summary",
                    ),
                    MethodAssetDefinitionKind::SpemMethodContent,
                    typed_ref(
                        MethodLibraryTypedBoundaryRefKind::GovernanceBasisRef,
                        "api-definition-title",
                    ),
                    None,
                    no_body_marker("api-definition-summary"),
                ),
                source_summary_refs: ExternalSourceSummaryRefSet::new(),
                preaccepted_catalog_entry_refs: MethodAssetCatalogEntryRefSet::new(),
            },
        ),
    );

    assert_eq!(
        output.result_kind,
        method_library_application::MethodAssetStoredOperationResultKind::Accepted
    );
    assert!(output.accepted_summary_ref.is_some());
}
