//! Shared fixture helpers for contract-domain-fast validation.

use crate::commands::{MethodLibraryCapabilityKind, MethodLibraryCommandShell};
use crate::events::MethodLibraryEventShell;
use crate::jobs::{MethodLibraryJobShell, MethodLibraryOperationsJobKind};
use crate::metadata::{
    ActorContext, ActorKind, ActorRef, CommandMetadata, QueryMetadata, RequestId, RequestMetadata,
    RequestOrigin, Timestamp, TraceId,
};
use crate::queries::MethodLibraryQueryShell;
use crate::refs::{MethodLibraryTypedBoundaryRef, MethodLibraryTypedBoundaryRefKind};
use crate::views::{MethodLibrarySafeMarker, MethodLibraryViewShell};
use core_contracts::metadata::PageRequest;

fn sample_actor_context(origin: RequestOrigin) -> ActorContext {
    ActorContext::new(ActorRef::new("actor-1", ActorKind::Human), origin)
}

fn sample_request_metadata() -> RequestMetadata {
    RequestMetadata::new(
        RequestId::new("request-1"),
        TraceId::new("trace-1"),
        Some("idem-1".into()),
        Timestamp::new("2026-06-29T14:00:00Z"),
    )
}

fn sample_boundary_ref() -> MethodLibraryTypedBoundaryRef {
    MethodLibraryTypedBoundaryRef::from_verified_source(
        MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionRef,
        "ml:def:001",
    )
}

fn sample_marker() -> MethodLibrarySafeMarker {
    MethodLibrarySafeMarker::no_body(sample_boundary_ref())
}

/// Returns a sample command shell fixture.
pub fn sample_command_shell() -> MethodLibraryCommandShell {
    MethodLibraryCommandShell {
        capability_kind: MethodLibraryCapabilityKind::DefinitionCatalog,
        actor_context: sample_actor_context(RequestOrigin::Command),
        metadata: CommandMetadata {
            request: sample_request_metadata(),
            reason: Some(crate::metadata::ChangeReason {
                value: "fixture".to_owned(),
            }),
            external_ref: None,
        },
        boundary_ref: sample_boundary_ref(),
        typed_refs: vec![sample_boundary_ref()],
        safe_markers: vec![sample_marker()],
    }
}

/// Returns a sample query shell fixture.
pub fn sample_query_shell() -> MethodLibraryQueryShell {
    MethodLibraryQueryShell {
        capability_kind: MethodLibraryCapabilityKind::DefinitionCatalog,
        actor_context: sample_actor_context(RequestOrigin::Query),
        metadata: QueryMetadata {
            request: sample_request_metadata(),
            consistency: crate::metadata::QueryConsistency::Strong,
            page: Some(PageRequest {
                limit: 20,
                page_token: None,
            }),
        },
        boundary_ref: sample_boundary_ref(),
        typed_refs: vec![sample_boundary_ref()],
        safe_markers: vec![sample_marker()],
    }
}

/// Returns a sample event shell fixture.
pub fn sample_event_shell() -> MethodLibraryEventShell {
    MethodLibraryEventShell {
        capability_kind: MethodLibraryCapabilityKind::TraceConsistency,
        request_metadata: sample_request_metadata(),
        trace_id: TraceId::new("trace-1"),
        typed_refs: vec![sample_boundary_ref()],
        safe_markers: vec![sample_marker()],
    }
}

/// Returns a sample job shell fixture.
pub fn sample_job_shell() -> MethodLibraryJobShell {
    MethodLibraryJobShell {
        job_kind: MethodLibraryOperationsJobKind::RefreshCatalogDefinitionReadMaterials,
        request_metadata: sample_request_metadata(),
        typed_refs: vec![sample_boundary_ref()],
        safe_markers: vec![sample_marker()],
    }
}

/// Returns a sample view shell fixture.
pub fn sample_view_shell() -> MethodLibraryViewShell {
    MethodLibraryViewShell {
        capability_kind: MethodLibraryCapabilityKind::DefinitionCatalog,
        typed_refs: vec![sample_boundary_ref()],
        safe_markers: vec![sample_marker()],
    }
}
