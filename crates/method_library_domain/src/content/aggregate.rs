//! MethodContent aggregate root and lifecycle operations.

use serde::{Deserialize, Serialize};

use crate::error::{MethodLibraryError, MethodLibraryErrorCode};

use super::fingerprint::CanonicalFingerprint;
use super::kind::MethodContentKind;
use super::lifecycle::{LifecycleState, MethodContentLifecycle};
use super::reference::{ContentRef, ReferenceState};
use super::version::ContentVersion;
use super::{
    ActorId, ApprovedGateRef, ContentFamilyId, ContentId, MethodContentPayload, Revision, Timestamp,
};

/// Aggregate root for P0 method-definition truth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodContent {
    /// Definition identifier.
    pub content_id: ContentId,
    /// Content lineage identifier.
    pub content_family_id: ContentFamilyId,
    /// P0 method-content kind.
    pub kind: MethodContentKind,
    /// Human-readable name.
    pub name: String,
    /// Optional descriptive text.
    pub description: Option<String>,
    /// Definition payload.
    pub payload: MethodContentPayload,
    /// Draft-time and published definition references.
    pub references: Vec<ContentRef>,
    /// Lifecycle metadata.
    pub lifecycle: MethodContentLifecycle,
    /// Published business version.
    pub version: Option<ContentVersion>,
    /// Published canonical fingerprint.
    pub fingerprint: Option<CanonicalFingerprint>,
    /// Old content superseded by this content.
    pub supersedes_content_id: Option<ContentId>,
    /// New content replacing this content.
    pub superseded_by_content_id: Option<ContentId>,
    /// Creator actor identifier.
    pub created_by: ActorId,
    /// Creation time.
    pub created_at: Timestamp,
    /// Last update time.
    pub updated_at: Timestamp,
    /// Optimistic-lock revision.
    pub revision: Revision,
}

impl MethodContent {
    /// Creates a new draft aggregate.
    pub fn create_draft(
        content_id: ContentId,
        content_family_id: ContentFamilyId,
        kind: MethodContentKind,
        name: String,
        description: Option<String>,
        payload: MethodContentPayload,
        actor_id: ActorId,
        now: Timestamp,
    ) -> Result<Self, MethodLibraryError> {
        let mut content = Self {
            content_id,
            content_family_id,
            kind,
            name,
            description,
            payload,
            references: Vec::new(),
            lifecycle: MethodContentLifecycle::initial_draft(actor_id.clone(), now),
            version: None,
            fingerprint: None,
            supersedes_content_id: None,
            superseded_by_content_id: None,
            created_by: actor_id,
            created_at: now,
            updated_at: now,
            revision: 1,
        };
        content.ensure_payload_matches_kind()?;
        content.ensure_definition_boundary()?;
        content.references = content.payload.collect_content_refs();
        Ok(content)
    }

    /// Rehydrates a persisted aggregate snapshot.
    pub fn rehydrate(content: Self) -> Result<Self, MethodLibraryError> {
        content.ensure_payload_matches_kind()?;
        content.ensure_definition_boundary()?;
        Ok(content)
    }

    /// Ensures the current optimistic-lock revision matches the caller expectation.
    pub fn ensure_revision(&self, expected_revision: Revision) -> Result<(), MethodLibraryError> {
        if self.revision != expected_revision {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::RevisionConflict,
                "expected revision does not match the stored revision",
            )
            .with_detail("expected_revision", expected_revision.to_string())
            .with_detail("actual_revision", self.revision.to_string()));
        }

        Ok(())
    }

    /// Ensures the payload variant matches the declared method-content kind.
    pub fn ensure_payload_matches_kind(&self) -> Result<(), MethodLibraryError> {
        if self.kind != self.payload.kind() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::PayloadKindMismatch,
                "payload kind does not match the declared method-content kind",
            )
            .with_detail("kind", self.kind.as_str())
            .with_detail("payload_kind", self.payload.kind().as_str()));
        }

        Ok(())
    }

    /// Ensures the aggregate does not contain use-truth data.
    pub fn ensure_definition_boundary(&self) -> Result<(), MethodLibraryError> {
        self.payload.validate_definition_boundary()
    }

    /// Replaces the aggregate's stored references.
    pub fn replace_references(
        &mut self,
        references: Vec<ContentRef>,
        actor_id: ActorId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        if !self.lifecycle.allows_draft_update() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "references may only be replaced while the content is a draft",
            ));
        }

        self.references = references;
        self.updated_at = now;
        self.lifecycle.changed_by = actor_id;
        self.lifecycle.changed_at = now;
        Ok(())
    }

    /// Updates the draft payload and metadata.
    pub fn update_draft(
        &mut self,
        name: String,
        description: Option<String>,
        payload: MethodContentPayload,
        actor_id: ActorId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        if !self.lifecycle.allows_draft_update() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "draft content can only be updated while in draft",
            ));
        }

        self.name = name;
        self.description = description;
        self.payload = payload;
        self.ensure_payload_matches_kind()?;
        self.ensure_definition_boundary()?;
        self.references = self.payload.collect_content_refs();
        self.updated_at = now;
        self.lifecycle.changed_by = actor_id;
        self.lifecycle.changed_at = now;
        self.revision += 1;
        Ok(())
    }

    /// Moves the content from draft to in-review.
    pub fn submit_for_review(
        &mut self,
        actor_id: ActorId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        if self.lifecycle.state != LifecycleState::Draft {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::LifecycleTransitionNotAllowed,
                "only draft content can be submitted for review",
            ));
        }

        self.lifecycle.transition_to(
            LifecycleState::InReview,
            actor_id,
            now,
            Some("submitted for review".to_string()),
        )?;
        self.updated_at = now;
        self.revision += 1;
        Ok(())
    }

    /// Publishes the content and stores the semantic version and fingerprint.
    pub fn publish(
        &mut self,
        _gate_ref: ApprovedGateRef,
        version: ContentVersion,
        fingerprint: CanonicalFingerprint,
        actor_id: ActorId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        if self.lifecycle.state != LifecycleState::InReview {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::LifecycleTransitionNotAllowed,
                "only in-review content can be published",
            ));
        }

        if self.version.is_some() || self.fingerprint.is_some() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "published metadata may not be overwritten",
            ));
        }

        self.lifecycle.transition_to(
            LifecycleState::Published,
            actor_id,
            now,
            Some("published".to_string()),
        )?;
        self.version = Some(version);
        self.fingerprint = Some(fingerprint);
        self.updated_at = now;
        self.revision += 1;
        Ok(())
    }

    /// Marks the content as deprecated.
    pub fn deprecate(
        &mut self,
        reason: String,
        actor_id: ActorId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        if self.lifecycle.state != LifecycleState::Published {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::LifecycleTransitionNotAllowed,
                "only published content can be deprecated",
            ));
        }

        self.lifecycle
            .transition_to(LifecycleState::Deprecated, actor_id, now, Some(reason))?;
        self.updated_at = now;
        self.revision += 1;
        Ok(())
    }

    /// Marks the content as retired.
    pub fn retire(
        &mut self,
        reason: String,
        actor_id: ActorId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        if !matches!(
            self.lifecycle.state,
            LifecycleState::Published | LifecycleState::Deprecated
        ) {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::LifecycleTransitionNotAllowed,
                "only published or deprecated content can be retired",
            ));
        }

        self.lifecycle
            .transition_to(LifecycleState::Retired, actor_id, now, Some(reason))?;
        self.updated_at = now;
        self.revision += 1;
        Ok(())
    }

    /// Marks the content as superseded by a newer definition.
    pub fn mark_superseded_by(
        &mut self,
        next_content_id: ContentId,
        reason: String,
        actor_id: ActorId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        if !matches!(
            self.lifecycle.state,
            LifecycleState::Published | LifecycleState::Deprecated
        ) {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::LifecycleTransitionNotAllowed,
                "only published or deprecated content can be superseded",
            ));
        }

        if next_content_id.trim().is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::SupersedeTargetRequired,
                "supersede target content id must not be empty",
            ));
        }

        self.superseded_by_content_id = Some(next_content_id);
        self.lifecycle
            .transition_to(LifecycleState::Superseded, actor_id, now, Some(reason))?;
        self.updated_at = now;
        self.revision += 1;
        Ok(())
    }

    /// Returns whether the content can be referenced as a published-like definition.
    #[must_use]
    pub const fn is_published_like(&self) -> bool {
        matches!(
            self.lifecycle.state,
            LifecycleState::Published | LifecycleState::Deprecated
        )
    }
}

impl MethodContentKind {
    /// Returns whether the current kind is suitable for content-level references.
    #[must_use]
    pub const fn is_definition_kind(self) -> bool {
        matches!(
            self,
            Self::Qualification
                | Self::RoleDefinition
                | Self::TaskDefinition
                | Self::WorkProductDefinition
                | Self::ProcessTemplateDef
                | Self::ViewProfile
                | Self::AIPolicyDef
        )
    }
}

impl ContentRef {
    /// Returns whether this reference requires published-like target state.
    #[must_use]
    pub const fn requires_published_like(&self) -> bool {
        matches!(self.required_state, ReferenceState::PublishedLike)
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;
    use crate::content::fingerprint::{CanonicalFingerprint, FingerprintAlgorithm};
    use crate::content::version::ContentVersion;
    use crate::definitions::{
        EvidenceKind, EvidenceRule, Qualification, QualificationLevel, QualificationLevelModel,
    };

    fn sample_payload() -> MethodContentPayload {
        MethodContentPayload::Qualification(Qualification {
            qualification_key: "quality-1".to_string(),
            name: "Quality".to_string(),
            description: Some("Quality baseline".to_string()),
            level_model: QualificationLevelModel {
                levels: vec![QualificationLevel {
                    level_key: "basic".to_string(),
                    name: "Basic".to_string(),
                    order: 1,
                    description: None,
                }],
                default_level_key: Some("basic".to_string()),
            },
            evidence_rules: vec![EvidenceRule {
                evidence_kind: EvidenceKind::Document,
                required: true,
                description: "Proof document".to_string(),
            }],
        })
    }

    #[test]
    fn creates_and_updates_draft_content() {
        let mut content = MethodContent::create_draft(
            "content-1".to_string(),
            "family-1".to_string(),
            MethodContentKind::Qualification,
            "Quality".to_string(),
            None,
            sample_payload(),
            "actor-1".to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("draft should be valid");

        assert_eq!(content.revision, 1);
        content
            .update_draft(
                "Quality v2".to_string(),
                Some("updated".to_string()),
                sample_payload(),
                "actor-2".to_string(),
                datetime!(2026-05-26 09:00:00 UTC),
            )
            .expect("draft should update");
        assert_eq!(content.revision, 2);
    }

    #[test]
    fn publishes_and_supersedes_content() {
        let mut content = MethodContent::create_draft(
            "content-1".to_string(),
            "family-1".to_string(),
            MethodContentKind::Qualification,
            "Quality".to_string(),
            None,
            sample_payload(),
            "actor-1".to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("draft should be valid");

        content
            .submit_for_review("actor-2".to_string(), datetime!(2026-05-26 09:00:00 UTC))
            .expect("draft should move to review");
        content
            .publish(
                ApprovedGateRef {
                    gate_id: "gate-1".to_string(),
                    gate_decision_id: "decision-1".to_string(),
                    approved_at: datetime!(2026-05-26 09:30:00 UTC),
                },
                ContentVersion::new("1.0.0").expect("version should be valid"),
                CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "abc123", "1.0")
                    .expect("fingerprint should be valid"),
                "actor-3".to_string(),
                datetime!(2026-05-26 10:00:00 UTC),
            )
            .expect("review content should publish");
        assert!(content.is_published_like());

        content
            .mark_superseded_by(
                "content-2".to_string(),
                "replaced".to_string(),
                "actor-4".to_string(),
                datetime!(2026-05-26 11:00:00 UTC),
            )
            .expect("published content should supersede");
        assert_eq!(content.lifecycle.state, LifecycleState::Superseded);
    }
}
