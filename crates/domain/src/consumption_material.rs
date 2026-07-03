//! Controlled-consumption domain objects closed for `commit-05-a`.

use method_library_contracts::{
    ConsumptionContextRef, DownstreamConsumptionBoundaryRef, FormalMethodAssetVersionRef,
    MethodAssetConsumptionAvailabilityMarker, MethodAssetConsumptionAvailabilityMarkerSource,
    MethodAssetConsumptionAvailabilityTarget, MethodAssetConsumptionMaterialCursorRef,
    MethodAssetConsumptionMaterialRef, MethodAssetConsumptionMaterialState,
    MethodAssetConsumptionSummary, MethodAssetDefinitionRef, MethodLibrarySafeMarker,
    MethodLibrarySafeMarkerKind, MethodLibraryTypedBoundaryRefKind,
};

use crate::errors::MethodLibraryDomainError;

/// Controlled consumption read material truth owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAssetConsumptionMaterial {
    /// Stable consumption material anchor.
    pub consumption_material_ref: MethodAssetConsumptionMaterialRef,
    /// Controlled formal version source.
    pub formal_version_ref: FormalMethodAssetVersionRef,
    /// Controlled definition source.
    pub definition_ref: MethodAssetDefinitionRef,
    /// Controlled consumption context.
    pub consumption_context_ref: ConsumptionContextRef,
    /// Related downstream boundary.
    pub boundary_ref: DownstreamConsumptionBoundaryRef,
    /// Body-free downstream consumption summary.
    pub consumption_summary: MethodAssetConsumptionSummary,
    /// Source cursor used by later refresh surfaces.
    pub source_cursor_ref: MethodAssetConsumptionMaterialCursorRef,
    /// Exact current-boundary material state.
    pub material_state: MethodAssetConsumptionMaterialState,
    /// Optional copy-only availability marker.
    pub availability_marker: Option<MethodAssetConsumptionAvailabilityMarker>,
}

impl MethodAssetConsumptionMaterial {
    /// Creates a prepared controlled material from a formal version and boundary.
    pub fn from_formal_version(
        consumption_material_ref: MethodAssetConsumptionMaterialRef,
        formal_version_ref: FormalMethodAssetVersionRef,
        definition_ref: MethodAssetDefinitionRef,
        boundary_ref: DownstreamConsumptionBoundaryRef,
        consumption_context_ref: ConsumptionContextRef,
        consumption_summary: MethodAssetConsumptionSummary,
        source_cursor_ref: MethodAssetConsumptionMaterialCursorRef,
    ) -> Self {
        Self {
            consumption_material_ref,
            formal_version_ref,
            definition_ref,
            consumption_context_ref,
            boundary_ref,
            consumption_summary,
            source_cursor_ref,
            material_state: MethodAssetConsumptionMaterialState::Prepared,
            availability_marker: None,
        }
    }

    /// Confirms the material remains anchored to the expected formal version.
    pub fn assert_from_formal_version(
        &self,
        formal_version_ref: &FormalMethodAssetVersionRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if &self.formal_version_ref == formal_version_ref {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invariant_violation())
        }
    }

    /// Confirms the material remains anchored to the expected consumption context.
    pub fn assert_context(
        &self,
        consumption_context_ref: &ConsumptionContextRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if &self.consumption_context_ref == consumption_context_ref {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invariant_violation())
        }
    }

    /// Confirms the material remains governed by the expected downstream boundary.
    pub fn assert_boundary(
        &self,
        boundary_ref: &DownstreamConsumptionBoundaryRef,
    ) -> Result<(), MethodLibraryDomainError> {
        if &self.boundary_ref == boundary_ref {
            Ok(())
        } else {
            Err(MethodLibraryDomainError::invariant_violation())
        }
    }

    /// Copies an exact availability marker into the material state.
    pub fn apply_availability_marker(&mut self, marker: MethodAssetConsumptionAvailabilityMarker) {
        self.material_state = marker.material_state();
        self.availability_marker = Some(marker);
    }

    /// Marks the material stale using a copy-only availability marker.
    pub fn mark_stale(
        &mut self,
        reason_ref: MethodLibrarySafeMarker,
    ) -> Result<(), MethodLibraryDomainError> {
        if !reason_ref.is_public_safe() {
            return Err(MethodLibraryDomainError::policy_rejected());
        }

        let availability_marker = self
            .availability_marker
            .clone()
            .or_else(|| {
                let reason_source = &reason_ref.source_ref;
                if reason_ref.marker_kind() == MethodLibrarySafeMarkerKind::BoundaryMarker
                    && reason_source.kind()
                        == MethodLibraryTypedBoundaryRefKind::DownstreamConsumptionBoundary
                {
                    Some(MethodAssetConsumptionAvailabilityMarker::new(
                        reason_ref.clone(),
                        MethodAssetConsumptionAvailabilityTarget::Stale,
                        MethodAssetConsumptionAvailabilityMarkerSource::DownstreamConsumptionBoundaryGuard,
                        reason_ref.clone(),
                        Some(reason_ref.clone()),
                    ))
                } else {
                    None
                }
            })
            .ok_or_else(MethodLibraryDomainError::missing_required_typed_input)?;

        self.apply_availability_marker(MethodAssetConsumptionAvailabilityMarker::new(
            availability_marker.marker_ref,
            MethodAssetConsumptionAvailabilityTarget::Stale,
            availability_marker.source_kind,
            availability_marker.source_marker_ref,
            Some(reason_ref),
        ));
        Ok(())
    }
}
