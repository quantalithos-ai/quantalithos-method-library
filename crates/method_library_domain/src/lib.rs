//! Domain primitives and business rules for the method-library service.

/// Placeholder domain module tree populated in later implementation phases.
pub mod prelude {
    /// Workspace-level P1 feature reservation.
    pub const P1_PLUGIN_FEATURE: &str = "p1-plugin";
    /// Workspace-level P1 configuration reservation.
    pub const P1_CONFIGURATION_FEATURE: &str = "p1-configuration";
}

#[cfg(test)]
mod tests {
    use super::prelude::{P1_CONFIGURATION_FEATURE, P1_PLUGIN_FEATURE};

    #[test]
    fn reserves_p1_feature_names() {
        assert_eq!(P1_PLUGIN_FEATURE, "p1-plugin");
        assert_eq!(P1_CONFIGURATION_FEATURE, "p1-configuration");
    }
}
