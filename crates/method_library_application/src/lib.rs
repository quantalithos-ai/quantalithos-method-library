//! Application-service entrypoints and port traits for the method-library service.

use method_library_contracts::PlaceholderContract;

pub mod ports;
pub mod services;

pub use ports::*;
pub use services::MethodContentCommandService;

/// Minimal application surface kept intentionally small for the workspace bootstrap phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapService;

impl BootstrapService {
    /// Returns a placeholder contract proving the application crate can depend on contracts.
    #[must_use]
    pub fn describe(&self) -> PlaceholderContract {
        PlaceholderContract::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{BootstrapService, ports::FeatureFlag};

    #[test]
    fn describes_workspace_placeholder() {
        let service = BootstrapService;

        assert_eq!(
            service.describe().message,
            "method-library contracts placeholder"
        );
    }

    #[test]
    fn reexports_feature_flags() {
        assert_eq!(
            serde_json::to_string(&FeatureFlag::P1Plugin).expect("feature flag should serialize"),
            "\"p1_plugin\""
        );
    }
}
