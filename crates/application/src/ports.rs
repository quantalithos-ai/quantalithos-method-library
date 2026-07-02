//! Exact repository ports for the definition/catalog accepted-service slice.

use method_library_contracts::{
    CatalogScopeRef, FormalMethodAssetVersionRef, FormalizationBasisSummaryRef,
    FormalizationBasisSummaryRefSet, FormalizationEligibilityRejectionRef,
    FormalizationEligibilityRuleRef, FormalizationStateKind, FormalizationStateReasonSummary,
    FormalizationStateRef, GovernanceBasisRef, MethodAssetCatalogEntryRef,
    MethodAssetDefinitionRef, MethodAssetIdentityKey, MethodAssetSafeRejectReasonRef,
    MethodLibrarySafeMarker,
};
use method_library_domain::{
    FormalMethodAssetVersion, FormalizationBasisSummary, FormalizationState,
    MethodAssetCatalogEntry, MethodAssetDefinition,
};

use crate::definition_catalog::{
    MethodAssetExpectedVersion, MethodAssetRepositoryError, MethodAssetStoredOperationResult,
    Versioned, VersionedRef,
};
use crate::unit_of_work::CommandUnitOfWork;

/// Definition truth repository for the current boundary.
pub trait MethodAssetDefinitionRepository: Send + Sync {
    /// Loads a definition by stable ref together with its current repository version.
    fn get_definition_with_version(
        &self,
        definition_ref: MethodAssetDefinitionRef,
    ) -> Result<Option<Versioned<MethodAssetDefinition>>, MethodAssetRepositoryError>;

    /// Resolves a definition by stable identity key.
    fn find_definition_by_identity_key(
        &self,
        identity_key: MethodAssetIdentityKey,
    ) -> Result<Option<Versioned<MethodAssetDefinition>>, MethodAssetRepositoryError>;

    /// Saves a definition with the supplied optimistic version.
    fn save_definition(
        &self,
        definition: MethodAssetDefinition,
        expected_version: Option<MethodAssetExpectedVersion>,
        uow: &mut dyn CommandUnitOfWork,
    ) -> Result<VersionedRef<MethodAssetDefinitionRef>, MethodAssetRepositoryError>;
}

/// Catalog-entry truth repository for the current boundary.
pub trait MethodAssetCatalogEntryRepository: Send + Sync {
    /// Loads a catalog entry by stable ref together with its current repository version.
    fn get_catalog_entry_with_version(
        &self,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
    ) -> Result<Option<Versioned<MethodAssetCatalogEntry>>, MethodAssetRepositoryError>;

    /// Resolves a catalog entry by linked definition and catalog scope.
    fn find_catalog_entry_by_definition_scope(
        &self,
        definition_ref: MethodAssetDefinitionRef,
        catalog_scope_ref: CatalogScopeRef,
    ) -> Result<Option<Versioned<MethodAssetCatalogEntry>>, MethodAssetRepositoryError>;

    /// Saves a catalog entry with the supplied optimistic version.
    fn save_catalog_entry(
        &self,
        catalog_entry: MethodAssetCatalogEntry,
        expected_version: Option<MethodAssetExpectedVersion>,
        uow: &mut dyn CommandUnitOfWork,
    ) -> Result<VersionedRef<MethodAssetCatalogEntryRef>, MethodAssetRepositoryError>;
}

/// Stored-result repository for duplicate replay and no-rerun behavior.
pub trait MethodAssetStoredOperationResultRepository: Send + Sync {
    /// Looks up the stored result scoped by idempotency key and dedup scope.
    fn find_command_result_by_idempotency(
        &self,
        idempotency_key_ref: method_library_contracts::MethodAssetIdempotencyKeyRef,
        dedup_scope_ref: method_library_contracts::MethodAssetDedupScopeRef,
    ) -> Result<Option<MethodAssetStoredOperationResult>, MethodAssetRepositoryError>;

    /// Loads a stored result by its stable ref.
    fn get_stored_operation_result(
        &self,
        stored_result_ref: method_library_contracts::MethodAssetStoredOperationResultRef,
    ) -> Result<Option<MethodAssetStoredOperationResult>, MethodAssetRepositoryError>;

    /// Persists a stored result for duplicate replay.
    fn save_command_result_for_idempotency(
        &self,
        idempotency_key_ref: method_library_contracts::MethodAssetIdempotencyKeyRef,
        dedup_scope_ref: method_library_contracts::MethodAssetDedupScopeRef,
        operation_digest_ref: method_library_contracts::MethodAssetOperationDigestRef,
        stored_result: MethodAssetStoredOperationResult,
        uow: &mut dyn CommandUnitOfWork,
    ) -> Result<
        method_library_contracts::MethodAssetStoredOperationResultRef,
        MethodAssetRepositoryError,
    >;
}

/// External summary validation carve-out for `commit-03-b`.
pub trait ExternalSourceSummaryValidationPort: Send + Sync {
    /// Validates that the current-boundary wrapper set contains only named refs.
    fn validate_named_refs(
        &self,
        refs: &method_library_contracts::ExternalSourceSummaryRefSet,
    ) -> Result<(), MethodAssetRepositoryError>;
}

/// Formalization-state truth repository for the current boundary.
pub trait FormalizationStateRepository: Send + Sync {
    /// Loads a formalization state by stable ref together with its current repository version.
    fn get_formalization_state_with_version(
        &self,
        formalization_state_ref: FormalizationStateRef,
    ) -> Result<Option<Versioned<FormalizationState>>, MethodAssetRepositoryError>;

    /// Resolves a formalization state by definition and catalog anchors.
    fn find_formalization_state_by_definition_catalog(
        &self,
        definition_ref: MethodAssetDefinitionRef,
        catalog_entry_ref: MethodAssetCatalogEntryRef,
    ) -> Result<Option<Versioned<FormalizationState>>, MethodAssetRepositoryError>;

    /// Saves a formalization state with the supplied optimistic version.
    fn save_formalization_state(
        &self,
        formalization_state: FormalizationState,
        expected_version: Option<MethodAssetExpectedVersion>,
        uow: &mut dyn CommandUnitOfWork,
    ) -> Result<VersionedRef<FormalizationStateRef>, MethodAssetRepositoryError>;
}

/// Formal method-version truth repository for the current boundary.
pub trait FormalMethodAssetVersionRepository: Send + Sync {
    /// Loads a formal method version by stable ref together with its current repository version.
    fn get_formal_method_asset_version_with_version(
        &self,
        formal_version_ref: FormalMethodAssetVersionRef,
    ) -> Result<Option<Versioned<FormalMethodAssetVersion>>, MethodAssetRepositoryError>;

    /// Resolves the current formal method version for a formalization state owner.
    fn find_current_formal_method_asset_version(
        &self,
        formalization_state_ref: FormalizationStateRef,
    ) -> Result<Option<Versioned<FormalMethodAssetVersion>>, MethodAssetRepositoryError>;

    /// Saves a formal method version with the supplied optimistic version.
    fn save_formal_method_asset_version(
        &self,
        formal_version: FormalMethodAssetVersion,
        expected_version: Option<MethodAssetExpectedVersion>,
        uow: &mut dyn CommandUnitOfWork,
    ) -> Result<VersionedRef<FormalMethodAssetVersionRef>, MethodAssetRepositoryError>;
}

/// Formalization basis-summary repository for the current boundary.
pub trait FormalizationBasisSummaryRepository: Send + Sync {
    /// Loads a basis summary by stable ref together with its current repository version.
    fn get_formalization_basis_summary_with_version(
        &self,
        basis_summary_ref: FormalizationBasisSummaryRef,
    ) -> Result<Option<Versioned<FormalizationBasisSummary>>, MethodAssetRepositoryError>;
}

/// Body-free formalization basis-resolution input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormalizationBasisResolutionInput {
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Optional linked catalog-entry context.
    pub catalog_entry_ref: Option<MethodAssetCatalogEntryRef>,
    /// Basis-summary refs to resolve.
    pub basis_summary_refs: FormalizationBasisSummaryRefSet,
    /// Optional governance basis anchor.
    pub governance_basis_ref: Option<GovernanceBasisRef>,
}

/// Body-free formalization basis-resolution output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormalizationBasisResolution {
    /// Accepted basis-summary refs after resolver validation.
    pub accepted_basis_summary_refs: FormalizationBasisSummaryRefSet,
    /// Optional pending marker copied from resolver output.
    pub pending_marker_ref: Option<MethodLibrarySafeMarker>,
    /// Optional safe eligibility rejection ref.
    pub rejection_reason_ref: Option<FormalizationEligibilityRejectionRef>,
}

/// Exact resolver seam for body-free formalization basis summaries.
pub trait FormalizationBasisResolverPort: Send + Sync {
    /// Resolves basis summaries into an accepted current-boundary basis decision.
    fn resolve_formalization_basis(
        &self,
        input: FormalizationBasisResolutionInput,
    ) -> Result<FormalizationBasisResolution, MethodAssetRepositoryError>;
}

/// Body-free formalization eligibility diagnostic output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormalizationEligibilityDiagnostic {
    /// Target formalization state kind chosen by policy.
    pub target_state_kind: FormalizationStateKind,
    /// Safe reason summary copied into the formalization state truth.
    pub reason_summary: FormalizationStateReasonSummary,
}

/// Body-free formal version change diagnostic output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormalVersionChangeDiagnostic {
    /// Safe accepted semantic-change marker.
    pub accepted_change_marker_ref: MethodLibrarySafeMarker,
    /// Optional safe blocking reason.
    pub blocking_reason_ref: Option<MethodAssetSafeRejectReasonRef>,
}

/// Exact policy-diagnostic builder seam for current-boundary formalization/version flows.
pub trait MethodAssetPolicyDiagnosticBuilderPort: Send + Sync {
    /// Builds the formalization eligibility diagnostic.
    fn build_formalization_eligibility_diagnostic(
        &self,
        definition: &MethodAssetDefinition,
        catalog_entry: &MethodAssetCatalogEntry,
        basis_resolution: &FormalizationBasisResolution,
        eligibility_rule_ref: FormalizationEligibilityRuleRef,
    ) -> Result<FormalizationEligibilityDiagnostic, MethodAssetRepositoryError>;

    /// Builds the formal version semantic-change diagnostic.
    fn build_formal_version_change_diagnostic(
        &self,
        formal_version: &FormalMethodAssetVersion,
        basis_summary_refs: &FormalizationBasisSummaryRefSet,
        governance_basis_ref: Option<GovernanceBasisRef>,
        semantic_change_marker_ref: MethodLibrarySafeMarker,
    ) -> Result<FormalVersionChangeDiagnostic, MethodAssetRepositoryError>;
}
