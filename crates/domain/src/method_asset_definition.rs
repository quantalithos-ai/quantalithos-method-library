//! Method definition and catalog truth objects closed for `commit-03-b`.

use method_library_contracts::{
    CatalogScopeRef, ExternalSourceSummaryRef, ExternalSourceSummaryRefSet,
    MethodAssetApplicabilitySummary, MethodAssetCatalogClassification, MethodAssetCatalogEntryRef,
    MethodAssetCatalogEntryRefSet, MethodAssetCatalogEntryStatus, MethodAssetDefinitionKind,
    MethodAssetDefinitionRef, MethodAssetDefinitionSummary, MethodAssetIdentityKey,
    MethodLibrarySafeMarker,
};

use crate::errors::MethodLibraryDomainError;

/// Persisted lifecycle state for method asset definitions in the current boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodAssetDefinitionLifecycle {
    /// Definition is active and mutable within the current boundary.
    Active,
    /// Definition is retired and can no longer be adjusted.
    Retired,
}

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
    /// Persisted lifecycle state for the definition truth.
    pub definition_lifecycle: MethodAssetDefinitionLifecycle,
}

impl MethodAssetDefinition {
    /// Creates a definition truth and initializes the lifecycle to `Active`.
    pub fn create(
        definition_ref: MethodAssetDefinitionRef,
        identity_key: MethodAssetIdentityKey,
        definition_summary: MethodAssetDefinitionSummary,
    ) -> Self {
        Self {
            definition_kind: identity_key.definition_kind,
            definition_ref,
            identity_key,
            definition_summary,
            source_summary_refs: ExternalSourceSummaryRefSet::new(),
            catalog_entry_refs: MethodAssetCatalogEntryRefSet::new(),
            definition_lifecycle: MethodAssetDefinitionLifecycle::Active,
        }
    }

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

    /// Verifies the body-free boundary for the summary and source refs.
    pub fn assert_body_free(&self) -> Result<(), MethodLibraryDomainError> {
        if !self.definition_summary.summary_marker_ref.assert_no_body() {
            return Err(MethodLibraryDomainError::body_free_boundary_violation());
        }

        Ok(())
    }

    /// Confirms the definition is still active before an adjust operation.
    pub fn assert_active_for_adjust(&self) -> Result<(), MethodLibraryDomainError> {
        if self.definition_lifecycle == MethodAssetDefinitionLifecycle::Active {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invalid_transition())
        }
    }

    /// Applies a body-free summary/source adjustment while preserving `Active`.
    pub fn apply_adjustment(
        &mut self,
        replacement_definition_summary: MethodAssetDefinitionSummary,
        replacement_source_summary_refs: ExternalSourceSummaryRefSet,
    ) -> Result<(), MethodLibraryDomainError> {
        self.assert_active_for_adjust()?;
        self.definition_summary = replacement_definition_summary;
        self.source_summary_refs = replacement_source_summary_refs;
        self.definition_lifecycle = MethodAssetDefinitionLifecycle::Active;
        Ok(())
    }

    /// Marks the definition retired using an explicit safe marker.
    pub fn mark_retired(
        &mut self,
        retirement_marker_ref: MethodLibrarySafeMarker,
    ) -> Result<(), MethodLibraryDomainError> {
        self.assert_active_for_adjust()?;
        if !retirement_marker_ref.is_public_safe() {
            return Err(MethodLibraryDomainError::policy_rejected());
        }
        self.definition_lifecycle = MethodAssetDefinitionLifecycle::Retired;
        Ok(())
    }

    /// Links a catalog entry without copying any catalog view.
    pub fn link_catalog_entry(&mut self, catalog_entry_ref: MethodAssetCatalogEntryRef) {
        self.catalog_entry_refs.insert(catalog_entry_ref);
    }

    /// Accepts an external safe-summary ref.
    pub fn accept_source_summary(&mut self, source_summary_ref: ExternalSourceSummaryRef) {
        self.source_summary_refs.insert(source_summary_ref);
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
    fn assert_scope_consistency(
        catalog_scope_ref: &CatalogScopeRef,
        catalog_classification: &MethodAssetCatalogClassification,
        applicability_summary: &MethodAssetApplicabilitySummary,
    ) -> Result<(), MethodLibraryDomainError> {
        if &catalog_classification.catalog_scope_ref == catalog_scope_ref
            && &applicability_summary.applicability_scope_ref == catalog_scope_ref
        {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invariant_violation())
        }
    }

    /// Creates a visible catalog entry for a linked definition.
    pub fn create_for_definition(
        catalog_entry_ref: MethodAssetCatalogEntryRef,
        definition_ref: MethodAssetDefinitionRef,
        catalog_scope_ref: CatalogScopeRef,
        catalog_classification: MethodAssetCatalogClassification,
        applicability_summary: MethodAssetApplicabilitySummary,
    ) -> Result<Self, MethodLibraryDomainError> {
        Self::assert_scope_consistency(
            &catalog_scope_ref,
            &catalog_classification,
            &applicability_summary,
        )?;

        Ok(Self {
            catalog_entry_ref,
            definition_ref,
            catalog_scope_ref,
            catalog_classification,
            applicability_summary,
            catalog_status: MethodAssetCatalogEntryStatus::Visible,
        })
    }

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

    /// Updates the body-free catalog classification only through current-boundary guard logic.
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

    /// Confirms the catalog entry is visible before reclassification.
    pub fn assert_visible_for_reclassify(&self) -> Result<(), MethodLibraryDomainError> {
        if self.catalog_status == MethodAssetCatalogEntryStatus::Visible {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invalid_transition())
        }
    }

    /// Reclassifies the catalog entry and preserves the visible current-boundary state.
    pub fn reclassify(
        &mut self,
        new_catalog_classification: MethodAssetCatalogClassification,
        new_applicability_summary: MethodAssetApplicabilitySummary,
    ) -> Result<(), MethodLibraryDomainError> {
        self.assert_visible_for_reclassify()?;
        Self::assert_scope_consistency(
            &new_catalog_classification.catalog_scope_ref,
            &new_catalog_classification,
            &new_applicability_summary,
        )?;

        self.catalog_scope_ref = new_catalog_classification.catalog_scope_ref.clone();
        self.catalog_classification = new_catalog_classification;
        self.applicability_summary = new_applicability_summary;
        self.catalog_status = MethodAssetCatalogEntryStatus::Visible;
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

    /// Marks a visible catalog entry retired.
    pub fn mark_retired(
        &mut self,
        retirement_marker_ref: MethodLibrarySafeMarker,
    ) -> Result<(), MethodLibraryDomainError> {
        self.assert_visible_for_reclassify()?;
        if !retirement_marker_ref.is_public_safe() {
            return Err(MethodLibraryDomainError::policy_rejected());
        }
        self.catalog_status = MethodAssetCatalogEntryStatus::Retired;
        Ok(())
    }
}
