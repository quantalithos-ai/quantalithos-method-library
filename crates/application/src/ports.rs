//! Exact repository ports for the definition/catalog accepted-service slice.

use method_library_contracts::{
    CatalogScopeRef, MethodAssetCatalogEntryRef, MethodAssetDefinitionRef, MethodAssetIdentityKey,
};
use method_library_domain::{MethodAssetCatalogEntry, MethodAssetDefinition};

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
