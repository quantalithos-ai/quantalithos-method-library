//! Method definition and catalog truth objects closed for `commit-03-a`.

use method_library_contracts::{
    CatalogScopeRef, ExternalSourceSummaryRef, ExternalSourceSummaryRefSet,
    MethodAssetApplicabilitySummary, MethodAssetCatalogClassification, MethodAssetCatalogEntryRef,
    MethodAssetCatalogEntryRefSet, MethodAssetCatalogEntryStatus, MethodAssetDefinitionKind,
    MethodAssetDefinitionRef, MethodAssetDefinitionSummary, MethodAssetIdentityKey,
    MethodLibrarySafeMarker,
};

use crate::errors::MethodLibraryDomainError;

/// Method asset definition truth owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetDefinition {
    /// Stable definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Closed current-boundary definition kind.
    pub definition_kind: MethodAssetDefinitionKind,
    /// Stable definition identity key.
    pub identity_key: MethodAssetIdentityKey,
    /// Body-free definition summary.
    pub definition_summary: MethodAssetDefinitionSummary,
    /// Accepted external safe-summary refs.
    pub source_summary_refs: ExternalSourceSummaryRefSet,
    /// Linked catalog-entry refs.
    pub catalog_entry_refs: MethodAssetCatalogEntryRefSet,
}

impl MethodAssetDefinition {
    /// Confirms the provided identity matches the truth owner.
    pub fn assert_same_identity(
        &self,
        identity_key: &MethodAssetIdentityKey,
    ) -> Result<(), MethodLibraryDomainError> {
        if &self.identity_key == identity_key {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invariant_violation())
        }
    }

    /// Links a catalog entry without copying any catalog view.
    pub fn link_catalog_entry(&mut self, catalog_entry_ref: MethodAssetCatalogEntryRef) {
        self.catalog_entry_refs.insert(catalog_entry_ref);
    }

    /// Accepts an external safe-summary ref.
    pub fn accept_source_summary(&mut self, source_summary_ref: ExternalSourceSummaryRef) {
        self.source_summary_refs.insert(source_summary_ref);
    }

    /// Verifies the body-free boundary for the summary and source refs.
    pub fn assert_body_free(&self) -> Result<(), MethodLibraryDomainError> {
        if !self.definition_summary.summary_marker_ref.assert_no_body() {
            return Err(MethodLibraryDomainError::body_free_boundary_violation());
        }

        Ok(())
    }
}

/// Method asset catalog-entry truth owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetCatalogEntry {
    /// Stable catalog-entry anchor.
    pub catalog_entry_ref: MethodAssetCatalogEntryRef,
    /// Linked definition anchor.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Catalog scope anchor.
    pub catalog_scope_ref: CatalogScopeRef,
    /// Body-free catalog classification.
    pub catalog_classification: MethodAssetCatalogClassification,
    /// Body-free applicability summary.
    pub applicability_summary: MethodAssetApplicabilitySummary,
    /// Current-boundary public/truth summary status.
    pub catalog_status: MethodAssetCatalogEntryStatus,
}

impl MethodAssetCatalogEntry {
    /// Confirms the entry remains bound to the original definition.
    pub fn assert_for_definition(
        &self,
        definition_ref: &MethodAssetDefinitionRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if &self.definition_ref == definition_ref {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invariant_violation())
        }
    }

    /// Checks whether the entry still covers the provided scope.
    pub fn covers_scope(&self, catalog_scope_ref: &CatalogScopeRef) -> bool {
        &self.catalog_scope_ref == catalog_scope_ref
    }

    /// Updates the body-free catalog classification.
    pub fn update_classification(
        &mut self,
        classification: MethodAssetCatalogClassification,
    ) -> Result<(), MethodLibraryDomainError> {
        if classification.catalog_scope_ref != self.catalog_scope_ref {
            return Err(MethodLibraryDomainError::invariant_violation());
        }

        self.catalog_classification = classification;
        Ok(())
    }

    /// Marks the entry deprecated with an explicit safe marker.
    ///
    /// ```compile_fail
    /// use method_library_domain::MethodAssetCatalogEntry;
    ///
    /// fn raw_reason(entry: &mut MethodAssetCatalogEntry) {
    ///     let _ = entry.mark_deprecated("raw reason");
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use method_library_domain::MethodAssetCatalogEntry;
    ///
    /// fn parameterless(entry: &mut MethodAssetCatalogEntry) {
    ///     let _ = entry.mark_deprecated();
    /// }
    /// ```
    pub fn mark_deprecated(
        &mut self,
        reason_ref: MethodLibrarySafeMarker,
    ) -> Result<(), MethodLibraryDomainError> {
        if self.catalog_status == MethodAssetCatalogEntryStatus::Retired {
            return Err(MethodLibraryDomainError::invalid_transition());
        }

        if !reason_ref.is_public_safe() {
            return Err(MethodLibraryDomainError::policy_rejected());
        }

        self.catalog_status = MethodAssetCatalogEntryStatus::Deprecated;
        Ok(())
    }
}
