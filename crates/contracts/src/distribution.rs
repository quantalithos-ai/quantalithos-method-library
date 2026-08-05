//! Distribution/handoff reference carriers closed for `commit-05-b`.

use serde::{Deserialize, Serialize};

use crate::refs::{MethodAssetAdapterSlotRef, MethodAssetHandoffTargetRef};
use crate::views::MethodLibrarySafeMarker;

fn push_unique<T>(items: &mut Vec<T>, next: T)
where
    T: PartialEq,
{
    if !items.iter().any(|existing| existing == &next) {
        items.push(next);
    }
}

macro_rules! named_safe_marker_ref {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
        pub struct $name {
            /// The exact body-free safe marker.
            pub safe_marker: MethodLibrarySafeMarker,
        }

        impl $name {
            /// Creates the named marker wrapper.
            pub fn new(safe_marker: MethodLibrarySafeMarker) -> Self {
                Self { safe_marker }
            }

            /// Returns the wrapped safe marker.
            pub fn as_safe_marker(&self) -> &MethodLibrarySafeMarker {
                &self.safe_marker
            }
        }
    };
}

named_safe_marker_ref!(
    MethodAssetPublicationBoundaryMarkerRef,
    "Named safe-marker wrapper for the publication boundary."
);
named_safe_marker_ref!(
    MethodAssetHandoffBoundaryMarkerRef,
    "Named safe-marker wrapper for the collaboration handoff boundary."
);

/// Named safe-marker wrapper for an event-candidate reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetEventCandidateReasonRef {
    /// The exact body-free safe marker.
    pub safe_marker: MethodLibrarySafeMarker,
}

impl MethodAssetEventCandidateReasonRef {
    /// Creates the named marker wrapper.
    pub fn new(safe_marker: MethodLibrarySafeMarker) -> Self {
        Self { safe_marker }
    }

    /// Returns the wrapped safe marker.
    pub fn as_safe_marker(&self) -> &MethodLibrarySafeMarker {
        &self.safe_marker
    }
}

/// Named safe-marker wrapper for a distribution-adjustment reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetDistributionAdjustmentReasonRef {
    /// The exact body-free safe marker.
    pub safe_marker: MethodLibrarySafeMarker,
}

impl MethodAssetDistributionAdjustmentReasonRef {
    /// Creates the named marker wrapper.
    pub fn new(safe_marker: MethodLibrarySafeMarker) -> Self {
        Self { safe_marker }
    }

    /// Returns the wrapped safe marker.
    pub fn as_safe_marker(&self) -> &MethodLibrarySafeMarker {
        &self.safe_marker
    }
}

/// Deterministic required adapter-slot refs.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetAdapterSlotRefSet {
    /// Refs in first-insertion order after canonical typed-ref dedup.
    pub refs: Vec<MethodAssetAdapterSlotRef>,
}

impl MethodAssetAdapterSlotRefSet {
    /// Creates an empty ref set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a deterministic set from refs.
    pub fn from_refs(refs: impl IntoIterator<Item = MethodAssetAdapterSlotRef>) -> Self {
        let mut set = Self::new();
        for next in refs {
            set.insert(next);
        }
        set
    }

    /// Inserts a ref if its canonical typed identity is not present.
    pub fn insert(&mut self, next: MethodAssetAdapterSlotRef) {
        push_unique(&mut self.refs, next);
    }

    /// Returns whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }
}

/// Deterministic body-free handoff-target refs.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct MethodAssetHandoffTargetRefSet {
    /// Refs in first-insertion order after canonical typed-ref dedup.
    pub refs: Vec<MethodAssetHandoffTargetRef>,
}

impl MethodAssetHandoffTargetRefSet {
    /// Creates an empty ref set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a deterministic set from refs.
    pub fn from_refs(refs: impl IntoIterator<Item = MethodAssetHandoffTargetRef>) -> Self {
        let mut set = Self::new();
        for next in refs {
            set.insert(next);
        }
        set
    }

    /// Inserts a ref if its canonical typed identity is not present.
    pub fn insert(&mut self, next: MethodAssetHandoffTargetRef) {
        push_unique(&mut self.refs, next);
    }

    /// Returns whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }
}
