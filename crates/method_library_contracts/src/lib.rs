//! Protocol contracts shared by the API, worker, and application layers.

use serde::{Deserialize, Serialize};

/// Minimal error response placeholder used before the full protocol surface is implemented.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceholderContract {
    /// Reserved message that proves the contracts crate is wired into the workspace.
    pub message: String,
}

impl Default for PlaceholderContract {
    fn default() -> Self {
        Self {
            message: "method-library contracts placeholder".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PlaceholderContract;

    #[test]
    fn serializes_placeholder_contract() {
        let payload = serde_json::to_string(&PlaceholderContract::default())
            .expect("placeholder contract should serialize");

        assert!(payload.contains("contracts placeholder"));
    }
}
