//! Formal method-content kinds supported by the P0 scope.

use serde::{Deserialize, Serialize};

/// Formal method-definition kinds owned by the method-library P0 scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodContentKind {
    /// SPEM qualification semantics.
    Qualification,
    /// SPEM role-definition semantics.
    RoleDefinition,
    /// SPEM task-definition semantics.
    TaskDefinition,
    /// SPEM work-product-definition semantics.
    WorkProductDefinition,
    /// Process-template definition semantics.
    ProcessTemplateDef,
    /// View-profile definition semantics.
    ViewProfile,
    /// AI-policy definition semantics.
    AIPolicyDef,
}

impl MethodContentKind {
    /// Returns the persisted and serialized snake-case value for the kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Qualification => "qualification",
            Self::RoleDefinition => "role_definition",
            Self::TaskDefinition => "task_definition",
            Self::WorkProductDefinition => "work_product_definition",
            Self::ProcessTemplateDef => "process_template_def",
            Self::ViewProfile => "view_profile",
            Self::AIPolicyDef => "ai_policy_def",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MethodContentKind;

    #[test]
    fn serializes_to_snake_case() {
        let value = serde_json::to_string(&MethodContentKind::ProcessTemplateDef)
            .expect("kind should serialize");

        assert_eq!(value, "\"process_template_def\"");
    }
}
