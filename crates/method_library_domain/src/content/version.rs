//! Version value objects for published method-content definitions.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{MethodLibraryError, MethodLibraryErrorCode};

/// Versioning scheme used by business versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionScheme {
    /// Semantic-version style values.
    SemanticVersion,
}

/// Published business version attached to a method-content definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentVersion {
    /// Raw version string persisted for business consumers.
    pub raw: String,
    /// Declared versioning scheme.
    pub scheme: VersionScheme,
}

impl ContentVersion {
    /// Creates a new business version and performs lightweight validation.
    pub fn new(raw: impl Into<String>) -> Result<Self, MethodLibraryError> {
        let raw = raw.into();
        if raw.trim().is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::ContentVersionConflict,
                "content version must not be empty",
            ));
        }

        if !raw
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-_+".contains(character))
        {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::ContentVersionConflict,
                "content version contains unsupported characters",
            )
            .with_detail("version", raw));
        }

        Ok(Self {
            raw,
            scheme: VersionScheme::SemanticVersion,
        })
    }
}

impl Serialize for ContentVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for ContentVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::ContentVersion;

    #[test]
    fn accepts_semantic_style_versions() {
        let version = ContentVersion::new("1.2.3-beta.1").expect("version should be valid");

        assert_eq!(version.raw, "1.2.3-beta.1");
    }

    #[test]
    fn rejects_blank_versions() {
        let error = ContentVersion::new(" ").expect_err("blank versions should fail");

        assert_eq!(
            error.code,
            crate::error::MethodLibraryErrorCode::ContentVersionConflict
        );
    }

    #[test]
    fn serializes_as_a_string() {
        let version = ContentVersion::new("1.2.3").expect("version should be valid");

        let json = serde_json::to_string(&version).expect("version should serialize");

        assert_eq!(json, "\"1.2.3\"");
    }
}
