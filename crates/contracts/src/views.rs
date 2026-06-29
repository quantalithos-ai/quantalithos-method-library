//! Shared public shell and marker carriers for method library contracts.

use serde::{Deserialize, Serialize};

use crate::refs::MethodLibraryTypedBoundaryRef;

/// The family of a safe public marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodLibrarySafeMarkerKind {
    /// The shell contains only body-free data.
    NoBodyMarker,
    /// The shell carries freshness-related hints.
    FreshnessMarker,
    /// The shell carries availability-related hints.
    AvailabilityMarker,
    /// The shell carries a safe boundary hint.
    BoundaryMarker,
    /// The shell carries lineage or traceability hints.
    LineageMarker,
}

/// Safe public marker copied from formal policy, resolver or material output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodLibrarySafeMarker {
    /// The marker family.
    pub marker_kind: MethodLibrarySafeMarkerKind,
    /// The source anchor for the marker.
    pub source_ref: MethodLibraryTypedBoundaryRef,
}

impl MethodLibrarySafeMarker {
    /// Creates a new safe marker.
    pub fn new(
        marker_kind: MethodLibrarySafeMarkerKind,
        source_ref: MethodLibraryTypedBoundaryRef,
    ) -> Self {
        Self {
            marker_kind,
            source_ref,
        }
    }

    /// Creates a no-body marker.
    pub fn no_body(source_ref: MethodLibraryTypedBoundaryRef) -> Self {
        Self::new(MethodLibrarySafeMarkerKind::NoBodyMarker, source_ref)
    }

    /// Creates a freshness marker.
    pub fn freshness(source_ref: MethodLibraryTypedBoundaryRef) -> Self {
        Self::new(MethodLibrarySafeMarkerKind::FreshnessMarker, source_ref)
    }

    /// Creates a boundary marker.
    pub fn boundary(source_ref: MethodLibraryTypedBoundaryRef) -> Self {
        Self::new(MethodLibrarySafeMarkerKind::BoundaryMarker, source_ref)
    }

    /// Returns the marker family.
    pub fn marker_kind(&self) -> MethodLibrarySafeMarkerKind {
        self.marker_kind
    }

    /// Returns whether the marker is safe for public use.
    pub fn is_public_safe(&self) -> bool {
        true
    }

    /// Returns whether the marker preserves the no-body boundary.
    pub fn assert_no_body(&self) -> bool {
        self.marker_kind == MethodLibrarySafeMarkerKind::NoBodyMarker
    }
}

/// The family of a shared public shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodLibraryShellKind {
    /// Safe public view shell.
    View,
    /// Safe public material shell.
    Material,
    /// Safe public summary shell.
    Summary,
    /// Event candidate shell.
    Event,
    /// Operations job shell.
    Job,
    /// Command or query wrapper shell.
    Protocol,
}

/// Shared shell boundary for body-free public surfaces.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodLibraryPublicShell {
    /// The public shell family.
    pub shell_kind: MethodLibraryShellKind,
    /// Public typed references carried by the shell.
    pub public_refs: Vec<MethodLibraryTypedBoundaryRef>,
    /// Safe markers carried by the shell.
    pub safe_markers: Vec<MethodLibrarySafeMarker>,
}

impl MethodLibraryPublicShell {
    /// Creates a new public shell boundary.
    pub fn new(shell_kind: MethodLibraryShellKind) -> Self {
        Self {
            shell_kind,
            public_refs: Vec::new(),
            safe_markers: Vec::new(),
        }
    }

    /// Creates a shell from a domain view reference.
    pub fn from_domain_view_ref(view_ref: MethodLibraryTypedBoundaryRef) -> Self {
        let mut shell = Self::new(MethodLibraryShellKind::View);
        shell.public_refs.push(view_ref);
        shell
    }

    /// Creates a shell from a summary reference.
    pub fn from_summary_ref(summary_ref: MethodLibraryTypedBoundaryRef) -> Self {
        let mut shell = Self::new(MethodLibraryShellKind::Summary);
        shell.public_refs.push(summary_ref);
        shell
    }

    /// Creates a shell from a job boundary reference.
    pub fn from_job_boundary(job_ref: MethodLibraryTypedBoundaryRef) -> Self {
        let mut shell = Self::new(MethodLibraryShellKind::Job);
        shell.public_refs.push(job_ref);
        shell
    }

    /// Returns the shell family.
    pub fn shell_kind(&self) -> MethodLibraryShellKind {
        self.shell_kind
    }

    /// Returns the shell public refs.
    pub fn public_refs(&self) -> &[MethodLibraryTypedBoundaryRef] {
        &self.public_refs
    }

    /// Returns the shell safe markers.
    pub fn safe_markers(&self) -> &[MethodLibrarySafeMarker] {
        &self.safe_markers
    }

    /// Returns whether the shell preserves the body-free boundary.
    pub fn assert_body_free(&self) -> bool {
        true
    }
}

/// Concrete shared view shell foundation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodLibraryViewShell {
    /// The shared capability family.
    pub capability_kind: crate::commands::MethodLibraryCapabilityKind,
    /// The typed refs exposed by the shell.
    pub typed_refs: Vec<MethodLibraryTypedBoundaryRef>,
    /// Safe markers exposed by the shell.
    pub safe_markers: Vec<MethodLibrarySafeMarker>,
}
