//! Application-service entrypoints and port traits for the method-library service.

use method_library_contracts::PlaceholderContract;

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
    use super::BootstrapService;

    #[test]
    fn describes_workspace_placeholder() {
        let service = BootstrapService;

        assert_eq!(
            service.describe().message,
            "method-library contracts placeholder"
        );
    }
}
