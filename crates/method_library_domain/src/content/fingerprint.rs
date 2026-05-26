//! Fingerprint value objects for canonical definition content.

use serde::{Deserialize, Serialize};

use crate::error::{MethodLibraryError, MethodLibraryErrorCode};

/// Fingerprint hashing algorithm identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintAlgorithm {
    /// SHA-256 fingerprinting.
    Sha256,
}

/// Canonical fingerprint schema version label.
pub type CanonicalSchemaVersion = String;

/// Canonical semantic fingerprint of a published definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CanonicalFingerprint {
    /// Hashing algorithm used to build the fingerprint.
    pub algorithm: FingerprintAlgorithm,
    /// Stable textual hash value.
    pub value: String,
    /// Canonicalization schema version used when generating the hash.
    pub canonical_schema_version: CanonicalSchemaVersion,
}

impl CanonicalFingerprint {
    /// Creates a new canonical fingerprint and validates its shape.
    pub fn new(
        algorithm: FingerprintAlgorithm,
        value: impl Into<String>,
        canonical_schema_version: impl Into<String>,
    ) -> Result<Self, MethodLibraryError> {
        let value = value.into();
        let canonical_schema_version = canonical_schema_version.into();

        if value.trim().is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::FingerprintBuildFailed,
                "fingerprint value must not be empty",
            ));
        }

        if canonical_schema_version.trim().is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::FingerprintBuildFailed,
                "canonical schema version must not be empty",
            ));
        }

        let normalized_value = match algorithm {
            FingerprintAlgorithm::Sha256 if value.starts_with("sha256:") => value,
            FingerprintAlgorithm::Sha256 => format!("sha256:{value}"),
        };

        Ok(Self {
            algorithm,
            value: normalized_value,
            canonical_schema_version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{CanonicalFingerprint, FingerprintAlgorithm};

    #[test]
    fn normalizes_sha256_prefix() {
        let fingerprint = CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "abc123", "1.0")
            .expect("fingerprint should be valid");

        assert_eq!(fingerprint.value, "sha256:abc123");
    }

    #[test]
    fn rejects_blank_schema_version() {
        let error = CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "abc", " ")
            .expect_err("blank schema version should fail");

        assert_eq!(
            error.code,
            crate::error::MethodLibraryErrorCode::FingerprintBuildFailed
        );
    }
}
