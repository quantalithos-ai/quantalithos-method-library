//! Reference value objects connecting related method-content definitions.

use serde::{Deserialize, Serialize};

use crate::error::{MethodLibraryError, MethodLibraryErrorCode};

use super::{CanonicalFingerprint, ContentId, ContentVersion, MethodContentKind};

/// Required state of a target reference during validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceState {
    /// The target must be published.
    Published,
    /// The target may be published or deprecated but must remain versioned.
    PublishedLike,
}

/// Draft-time reference to another definition aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentRef {
    /// Target definition identifier.
    pub target_content_id: ContentId,
    /// Expected target kind.
    pub target_kind: MethodContentKind,
    /// Required target lifecycle state.
    pub required_state: ReferenceState,
}

impl ContentRef {
    /// Creates a new draft-time reference and validates the target identifier.
    pub fn new(
        target_content_id: impl Into<String>,
        target_kind: MethodContentKind,
        required_state: ReferenceState,
    ) -> Result<Self, MethodLibraryError> {
        let target_content_id = target_content_id.into();
        if target_content_id.trim().is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::ReferenceInvalid,
                "reference target content id must not be empty",
            ));
        }

        Ok(Self {
            target_content_id,
            target_kind,
            required_state,
        })
    }
}

/// Published reference pinned to a specific version and fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublishedContentRef {
    /// Target definition identifier.
    pub content_id: ContentId,
    /// Target definition kind.
    pub kind: MethodContentKind,
    /// Target published version.
    pub version: ContentVersion,
    /// Target canonical fingerprint.
    pub fingerprint: CanonicalFingerprint,
}

impl PublishedContentRef {
    /// Creates a draft-time reference representing the stable published target.
    #[must_use]
    pub fn as_content_ref(&self) -> ContentRef {
        ContentRef {
            target_content_id: self.content_id.clone(),
            target_kind: self.kind,
            required_state: ReferenceState::PublishedLike,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ContentRef, ReferenceState};

    #[test]
    fn rejects_blank_reference_targets() {
        let error = ContentRef::new(
            " ",
            crate::content::MethodContentKind::Qualification,
            ReferenceState::Published,
        )
        .expect_err("blank target ids should fail");

        assert_eq!(
            error.code,
            crate::error::MethodLibraryErrorCode::ReferenceInvalid
        );
    }
}
