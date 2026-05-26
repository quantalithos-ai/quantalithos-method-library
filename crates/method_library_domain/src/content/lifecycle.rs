//! Lifecycle states and transition rules for method-content definitions.

use serde::{Deserialize, Serialize};

use crate::error::{MethodLibraryError, MethodLibraryErrorCode};

use super::{ActorId, Timestamp};

/// Lifecycle state of a method-content definition aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    /// Draft content may change payload and unresolved references.
    Draft,
    /// In-review content is waiting for an approved publish gate.
    InReview,
    /// Published content is the authoritative definition reference.
    Published,
    /// Deprecated content remains traceable but should not receive new usage.
    Deprecated,
    /// Retired content is terminal and trace-only.
    Retired,
    /// Superseded content is terminal and replaced by a newer published definition.
    Superseded,
}

/// Lifecycle metadata stored alongside the current state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodContentLifecycle {
    /// Current lifecycle state.
    pub state: LifecycleState,
    /// Actor that last changed the lifecycle state.
    pub changed_by: ActorId,
    /// Timestamp of the most recent lifecycle change.
    pub changed_at: Timestamp,
    /// Optional reason attached to lifecycle transitions that require explanation.
    pub reason: Option<String>,
}

impl MethodContentLifecycle {
    /// Creates the initial lifecycle used by freshly created drafts.
    #[must_use]
    pub fn initial_draft(actor_id: ActorId, now: Timestamp) -> Self {
        Self {
            state: LifecycleState::Draft,
            changed_by: actor_id,
            changed_at: now,
            reason: None,
        }
    }

    /// Restores a lifecycle from persisted state.
    pub fn from_persisted(
        value: &str,
        changed_by: ActorId,
        changed_at: Timestamp,
        reason: Option<String>,
    ) -> Result<Self, MethodLibraryError> {
        let state = match value {
            "draft" => LifecycleState::Draft,
            "in_review" => LifecycleState::InReview,
            "published" => LifecycleState::Published,
            "deprecated" => LifecycleState::Deprecated,
            "retired" => LifecycleState::Retired,
            "superseded" => LifecycleState::Superseded,
            _ => {
                return Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::LifecycleTransitionNotAllowed,
                    "persisted lifecycle state is unknown",
                )
                .with_detail("persisted_state", value));
            }
        };

        Ok(Self {
            state,
            changed_by,
            changed_at,
            reason,
        })
    }

    /// Returns whether the current state may transition to `target`.
    #[must_use]
    pub fn can_transition_to(&self, target: LifecycleState) -> bool {
        match (self.state, target) {
            (LifecycleState::Draft, LifecycleState::InReview)
            | (LifecycleState::InReview, LifecycleState::Published)
            | (LifecycleState::Published, LifecycleState::Deprecated)
            | (LifecycleState::Published, LifecycleState::Retired)
            | (LifecycleState::Published, LifecycleState::Superseded)
            | (LifecycleState::Deprecated, LifecycleState::Retired)
            | (LifecycleState::Deprecated, LifecycleState::Superseded) => true,
            (current, next) if current == next && current == LifecycleState::Draft => true,
            _ => false,
        }
    }

    /// Returns whether the lifecycle is terminal.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            LifecycleState::Retired | LifecycleState::Superseded
        )
    }

    /// Returns whether draft payload updates remain allowed.
    #[must_use]
    pub const fn allows_draft_update(&self) -> bool {
        matches!(self.state, LifecycleState::Draft)
    }

    /// Returns whether the current state requires a version to be present.
    #[must_use]
    pub const fn requires_version(&self) -> bool {
        matches!(
            self.state,
            LifecycleState::Published
                | LifecycleState::Deprecated
                | LifecycleState::Retired
                | LifecycleState::Superseded
        )
    }

    /// Applies a validated lifecycle transition and refreshes metadata.
    pub fn transition_to(
        &mut self,
        target: LifecycleState,
        actor_id: ActorId,
        now: Timestamp,
        reason: Option<String>,
    ) -> Result<(), MethodLibraryError> {
        if !self.can_transition_to(target) {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::LifecycleTransitionNotAllowed,
                "lifecycle transition is not allowed",
            )
            .with_detail("from_state", self.state.as_str())
            .with_detail("to_state", target.as_str()));
        }

        if matches!(
            target,
            LifecycleState::Deprecated | LifecycleState::Retired | LifecycleState::Superseded
        ) && reason.as_deref().is_none_or(str::is_empty)
        {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::LifecycleTransitionNotAllowed,
                "reason is required for terminal or deprecating transitions",
            )
            .with_detail("to_state", target.as_str()));
        }

        self.state = target;
        self.changed_by = actor_id;
        self.changed_at = now;
        self.reason = reason;
        Ok(())
    }
}

impl LifecycleState {
    /// Returns the persisted and serialized snake-case value for the state.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::InReview => "in_review",
            Self::Published => "published",
            Self::Deprecated => "deprecated",
            Self::Retired => "retired",
            Self::Superseded => "superseded",
        }
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::{LifecycleState, MethodContentLifecycle};

    #[test]
    fn creates_initial_draft_lifecycle() {
        let lifecycle = MethodContentLifecycle::initial_draft(
            "actor-1".to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        );

        assert_eq!(lifecycle.state, LifecycleState::Draft);
        assert!(lifecycle.allows_draft_update());
        assert!(!lifecycle.requires_version());
    }

    #[test]
    fn supports_valid_transition_sequence() {
        let mut lifecycle = MethodContentLifecycle::initial_draft(
            "actor-1".to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        );

        lifecycle
            .transition_to(
                LifecycleState::InReview,
                "actor-2".to_string(),
                datetime!(2026-05-26 09:00:00 UTC),
                None,
            )
            .expect("draft should move to review");
        lifecycle
            .transition_to(
                LifecycleState::Published,
                "actor-3".to_string(),
                datetime!(2026-05-26 10:00:00 UTC),
                Some("approved for release".to_string()),
            )
            .expect("review should move to published");
        lifecycle
            .transition_to(
                LifecycleState::Deprecated,
                "actor-4".to_string(),
                datetime!(2026-05-26 11:00:00 UTC),
                Some("superseded by policy revision".to_string()),
            )
            .expect("published should move to deprecated");
        lifecycle
            .transition_to(
                LifecycleState::Retired,
                "actor-5".to_string(),
                datetime!(2026-05-26 12:00:00 UTC),
                Some("kept for trace only".to_string()),
            )
            .expect("deprecated should move to retired");

        assert!(lifecycle.is_terminal());
        assert!(lifecycle.requires_version());
    }

    #[test]
    fn rejects_illegal_transition() {
        let mut lifecycle = MethodContentLifecycle::initial_draft(
            "actor-1".to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        );

        let error = lifecycle
            .transition_to(
                LifecycleState::Retired,
                "actor-2".to_string(),
                datetime!(2026-05-26 09:00:00 UTC),
                Some("not allowed".to_string()),
            )
            .expect_err("draft cannot retire directly");

        assert_eq!(
            error.code,
            crate::error::MethodLibraryErrorCode::LifecycleTransitionNotAllowed
        );
    }
}
