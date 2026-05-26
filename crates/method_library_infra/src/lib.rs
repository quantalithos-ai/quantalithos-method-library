//! Infrastructure adapters and persistence helpers for the method-library service.

/// Verifies that the infrastructure crate is linked into the workspace.
#[must_use]
pub fn adapter_name() -> &'static str {
    "method-library-infra"
}

#[cfg(test)]
mod tests {
    use super::adapter_name;

    #[test]
    fn exposes_infrastructure_placeholder() {
        assert_eq!(adapter_name(), "method-library-infra");
    }
}
