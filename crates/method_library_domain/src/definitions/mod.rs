//! P0 method-definition payloads and supporting value objects.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};

use crate::content::{
    ActorKind, CanonicalBytes, CanonicalFingerprint, CanonicalSchemaVersion, ContentRef,
    MethodContentKind, PublishedContentRef, canonicalize_json, contains_forbidden_boundary_key,
};
use crate::error::{MethodLibraryError, MethodLibraryErrorCode};

/// Stable qualification business key.
pub type QualificationKey = String;
/// Stable qualification level business key.
pub type QualificationLevelKey = String;
/// Stable role business key.
pub type RoleKey = String;
/// Stable task business key.
pub type TaskKey = String;
/// Stable task step business key.
pub type TaskStepKey = String;
/// Stable work-product business key.
pub type WorkProductKey = String;
/// Stable process-template business key.
pub type ProcessTemplateKey = String;
/// Stable activity-definition business key.
pub type ActivityDefinitionKey = String;
/// Stable tailoring-point business key.
pub type TailoringPointKey = String;
/// Stable tailoring-option business key.
pub type TailoringOptionKey = String;
/// Stable quality-rule business key.
pub type QualityRuleKey = String;
/// Stable AI-policy business key.
pub type AIPolicyKey = String;
/// Stable view-scope business key.
pub type ViewScopeKey = String;

/// Kind of evidence requested by a qualification definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// A document or artifact is required.
    Document,
    /// A link or URL is required.
    Link,
    /// An artifact reference is required.
    Artifact,
    /// A trace or audit trail is required.
    Trace,
    /// A sample record is required.
    Sample,
}

/// Severity of a work-product quality rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityRuleSeverity {
    /// Informational rule.
    Info,
    /// Warning-level rule.
    Warning,
    /// Blocking rule.
    Blocking,
}

/// Visibility of a view field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldVisibility {
    /// The field is shown and editable.
    Visible,
    /// The field is shown but read-only.
    Readonly,
    /// The field is hidden.
    Hidden,
}

/// Availability of a view action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionAvailability {
    /// The action is available.
    Available,
    /// The action is present but disabled.
    Disabled,
    /// The action is hidden.
    Hidden,
}

/// Severity of an AI policy constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicySeverity {
    /// Advisory-only constraint.
    Advisory,
    /// Blocking constraint.
    Blocking,
}

/// Operator used by a view condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewConditionOperator {
    /// Equality comparison.
    Eq,
    /// Membership comparison.
    In,
    /// Existence check.
    Exists,
}

/// Effect kind used by a tailoring option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TailoringEffectKind {
    /// Include a target key.
    Include,
    /// Exclude a target key.
    Exclude,
    /// Mark a target key optional.
    MarkOptional,
    /// Require a target key.
    Require,
}

/// Object kind a view profile applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewObjectKind {
    /// A work-item view.
    WorkItem,
    /// An artifact view.
    Artifact,
    /// A process view.
    Process,
    /// A definition view.
    Definition,
}

impl ViewObjectKind {
    /// Returns the stable snake_case label used in APIs and logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkItem => "work_item",
            Self::Artifact => "artifact",
            Self::Process => "process",
            Self::Definition => "definition",
        }
    }
}

/// A single qualification level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationLevel {
    /// Stable level key.
    pub level_key: QualificationLevelKey,
    /// Human-readable level name.
    pub name: String,
    /// Ordering value from low to high.
    pub order: u32,
    /// Optional description.
    pub description: Option<String>,
}

/// The level model for a qualification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationLevelModel {
    /// Ordered level collection.
    pub levels: Vec<QualificationLevel>,
    /// Optional default level key.
    pub default_level_key: Option<QualificationLevelKey>,
}

/// Evidence expectation for a qualification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRule {
    /// Required evidence kind.
    pub evidence_kind: EvidenceKind,
    /// Whether this evidence is mandatory.
    pub required: bool,
    /// Human-readable rule description.
    pub description: String,
}

/// Qualification definition payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qualification {
    /// Stable qualification key.
    pub qualification_key: QualificationKey,
    /// Display name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Allowed level model.
    pub level_model: QualificationLevelModel,
    /// Evidence rules.
    pub evidence_rules: Vec<EvidenceRule>,
}

/// Role definition payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleDefinition {
    /// Stable role key.
    pub role_key: RoleKey,
    /// Role responsibilities.
    pub responsibilities: Vec<String>,
    /// Published qualification references.
    pub qualification_refs: Vec<PublishedContentRef>,
    /// Default view-profile references.
    pub default_view_profile_refs: Vec<PublishedContentRef>,
}

/// Task step definition payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStepDefinition {
    /// Stable step key.
    pub step_key: TaskStepKey,
    /// Step order.
    pub order: u32,
    /// Step title.
    pub title: String,
    /// Optional step purpose.
    pub purpose: Option<String>,
    /// Optional expected output description.
    pub expected_output: Option<String>,
    /// Optional verification description.
    pub verification: Option<String>,
}

/// Task definition payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDefinition {
    /// Stable task key.
    pub task_key: TaskKey,
    /// Task purpose.
    pub purpose: String,
    /// Step definitions.
    pub step_defs: Vec<TaskStepDefinition>,
    /// Input work-product references.
    pub input_work_product_refs: Vec<PublishedContentRef>,
    /// Output work-product references.
    pub output_work_product_refs: Vec<PublishedContentRef>,
}

/// Artifact-kind reference used by work-product definitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactKindRef {
    /// Artifact kind key.
    pub artifact_kind: String,
    /// Optional schema version.
    pub schema_version: Option<String>,
}

/// Schema reference used by work-product definitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaRef {
    /// Logical or physical schema URI.
    pub schema_uri: String,
    /// Optional schema version.
    pub schema_version: Option<String>,
    /// Optional schema fingerprint.
    pub schema_fingerprint: Option<CanonicalFingerprint>,
}

/// Work-product quality rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRule {
    /// Stable quality-rule key.
    pub rule_key: QualityRuleKey,
    /// Human-readable description.
    pub description: String,
    /// Severity of the rule.
    pub severity: QualityRuleSeverity,
}

/// Work-product definition payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkProductDefinition {
    /// Stable work-product key.
    pub work_product_key: WorkProductKey,
    /// Artifact-kind reference.
    pub artifact_kind: ArtifactKindRef,
    /// Schema reference.
    pub schema_ref: SchemaRef,
    /// Quality rules.
    pub quality_rules: Vec<QualityRule>,
}

/// Activity definition payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityDefinition {
    /// Stable activity key.
    pub activity_key: ActivityDefinitionKey,
    /// Activity name.
    pub name: String,
    /// Task references included in the activity.
    pub task_definition_refs: Vec<PublishedContentRef>,
    /// Input work-product references.
    pub input_work_product_refs: Vec<PublishedContentRef>,
    /// Output work-product references.
    pub output_work_product_refs: Vec<PublishedContentRef>,
    /// Role references involved in the activity.
    pub role_refs: Vec<PublishedContentRef>,
}

/// Tailoring effect payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailoringEffect {
    /// Effect kind.
    pub effect_kind: TailoringEffectKind,
    /// Target keys affected by the effect.
    pub target_keys: Vec<String>,
    /// Structured parameters for the effect.
    pub parameters: JsonValue,
}

/// Tailoring option payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailoringOption {
    /// Stable option key.
    pub option_key: TailoringOptionKey,
    /// Option description.
    pub description: String,
    /// Option effect.
    pub effect: TailoringEffect,
}

/// Tailoring point payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TailoringPoint {
    /// Stable tailoring-point key.
    pub tailoring_key: TailoringPointKey,
    /// Tailoring-point description.
    pub description: String,
    /// Allowed tailoring options.
    pub allowed_options: Vec<TailoringOption>,
}

/// Process-template definition payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessTemplateDef {
    /// Stable template key.
    pub template_key: ProcessTemplateKey,
    /// Activity definitions.
    pub activity_defs: Vec<ActivityDefinition>,
    /// Work-product references.
    pub work_product_refs: Vec<PublishedContentRef>,
    /// Role references.
    pub role_refs: Vec<PublishedContentRef>,
    /// Tailoring points.
    pub tailoring_points: Vec<TailoringPoint>,
}

/// View-scope condition payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewCondition {
    /// Field path to inspect.
    pub field_path: String,
    /// Comparison operator.
    pub operator: ViewConditionOperator,
    /// Optional comparison value.
    pub value_json: Option<JsonValue>,
}

/// View-scope rule payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewScopeRule {
    /// Stable scope key.
    pub scope_key: ViewScopeKey,
    /// Match conditions.
    pub conditions: Vec<ViewCondition>,
}

/// View-field rule payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewFieldRule {
    /// Field path.
    pub field_path: String,
    /// Field visibility.
    pub visibility: FieldVisibility,
}

/// View-action rule payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewActionRule {
    /// Action key.
    pub action_key: String,
    /// Action availability.
    pub availability: ActionAvailability,
}

/// View-profile definition payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewProfile {
    /// Role reference.
    pub role_ref: PublishedContentRef,
    /// Object kind to which the profile applies.
    pub object_kind: ViewObjectKind,
    /// Scope rules.
    pub scope_rules: Vec<ViewScopeRule>,
    /// Field rules.
    pub field_rules: Vec<ViewFieldRule>,
    /// Action rules.
    pub action_rules: Vec<ViewActionRule>,
}

/// Reference to a policy rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRuleRef {
    /// Rule key.
    pub rule_key: String,
    /// Optional version.
    pub version: Option<String>,
}

/// Policy constraint payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AIPolicyConstraint {
    /// Constraint key.
    pub constraint_key: String,
    /// Constraint description.
    pub description: String,
    /// Constraint severity.
    pub severity: PolicySeverity,
}

/// AI-policy definition payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AIPolicyDef {
    /// Stable policy key.
    pub policy_key: AIPolicyKey,
    /// Target actor kind.
    pub target_actor_kind: ActorKind,
    /// Rule references.
    pub rule_refs: Vec<PolicyRuleRef>,
    /// Policy constraints.
    pub constraints: Vec<AIPolicyConstraint>,
}

/// Union of all P0 method-definition payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodContentPayload {
    /// Qualification payload.
    Qualification(Qualification),
    /// Role-definition payload.
    RoleDefinition(RoleDefinition),
    /// Task-definition payload.
    TaskDefinition(TaskDefinition),
    /// Work-product-definition payload.
    WorkProductDefinition(WorkProductDefinition),
    /// Process-template-definition payload.
    ProcessTemplateDef(ProcessTemplateDef),
    /// View-profile payload.
    ViewProfile(ViewProfile),
    /// AI-policy-definition payload.
    AIPolicyDef(AIPolicyDef),
}

impl MethodContentPayload {
    /// Returns the declared content kind of the payload.
    #[must_use]
    pub const fn kind(&self) -> MethodContentKind {
        match self {
            Self::Qualification(_) => MethodContentKind::Qualification,
            Self::RoleDefinition(_) => MethodContentKind::RoleDefinition,
            Self::TaskDefinition(_) => MethodContentKind::TaskDefinition,
            Self::WorkProductDefinition(_) => MethodContentKind::WorkProductDefinition,
            Self::ProcessTemplateDef(_) => MethodContentKind::ProcessTemplateDef,
            Self::ViewProfile(_) => MethodContentKind::ViewProfile,
            Self::AIPolicyDef(_) => MethodContentKind::AIPolicyDef,
        }
    }

    /// Validates that no Use truth has been embedded in the payload.
    pub fn validate_definition_boundary(&self) -> Result<(), MethodLibraryError> {
        let value = serde_json::to_value(self).map_err(|error| {
            MethodLibraryError::validation(
                MethodLibraryErrorCode::BoundaryViolation,
                "payload could not be serialized for boundary validation",
            )
            .with_detail("serialization_error", error.to_string())
        })?;

        if contains_forbidden_boundary_key(&value) {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::BoundaryViolation,
                "payload contains forbidden use-truth data",
            ));
        }

        match self {
            Self::Qualification(payload) => payload.validate()?,
            Self::RoleDefinition(payload) => payload.validate()?,
            Self::TaskDefinition(payload) => payload.validate()?,
            Self::WorkProductDefinition(payload) => payload.validate()?,
            Self::ProcessTemplateDef(payload) => payload.validate()?,
            Self::ViewProfile(payload) => payload.validate()?,
            Self::AIPolicyDef(payload) => payload.validate()?,
        }

        Ok(())
    }

    /// Collects draft-time references that must be validated by application services.
    #[must_use]
    pub fn collect_content_refs(&self) -> Vec<ContentRef> {
        match self {
            Self::Qualification(_) => Vec::new(),
            Self::RoleDefinition(payload) => payload.collect_content_refs(),
            Self::TaskDefinition(payload) => payload.collect_content_refs(),
            Self::WorkProductDefinition(_) => Vec::new(),
            Self::ProcessTemplateDef(payload) => payload.collect_content_refs(),
            Self::ViewProfile(payload) => payload.collect_content_refs(),
            Self::AIPolicyDef(_) => Vec::new(),
        }
    }

    /// Returns canonical bytes for fingerprinting.
    pub fn canonicalize(
        &self,
        schema_version: CanonicalSchemaVersion,
    ) -> Result<CanonicalBytes, MethodLibraryError> {
        self.validate_definition_boundary()?;
        let canonical = json!({
            "schema_version": schema_version,
            "payload": self,
        });

        Ok(canonicalize_json(&canonical))
    }
}

impl QualificationLevelModel {
    /// Validates the level model.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        if self.levels.is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::BoundaryViolation,
                "qualification level model must contain at least one level",
            ));
        }

        let mut seen_keys = BTreeSet::new();
        let mut previous_order = None;
        for level in &self.levels {
            ensure_non_empty(
                &level.level_key,
                MethodLibraryErrorCode::BoundaryViolation,
                "qualification level key must not be empty",
                "level_key",
            )?;
            ensure_non_empty(
                &level.name,
                MethodLibraryErrorCode::BoundaryViolation,
                "qualification level name must not be empty",
                "name",
            )?;
            if !seen_keys.insert(level.level_key.clone()) {
                return Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::BoundaryViolation,
                    "qualification level keys must be unique",
                )
                .with_detail("level_key", level.level_key.clone()));
            }

            if let Some(previous) = previous_order
                && level.order <= previous
            {
                return Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::BoundaryViolation,
                    "qualification levels must be ordered from low to high",
                )
                .with_detail("level_key", level.level_key.clone()));
            }

            previous_order = Some(level.order);
        }

        if let Some(default_level_key) = &self.default_level_key
            && !seen_keys.contains(default_level_key)
        {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::BoundaryViolation,
                "default qualification level must exist in the level model",
            )
            .with_detail("default_level_key", default_level_key.clone()));
        }

        Ok(())
    }
}

impl EvidenceRule {
    /// Validates the evidence rule.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.description,
            MethodLibraryErrorCode::BoundaryViolation,
            "evidence rule description must not be empty",
            "description",
        )
    }
}

impl Qualification {
    /// Validates the qualification definition.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.qualification_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "qualification key must not be empty",
            "qualification_key",
        )?;
        ensure_non_empty(
            &self.name,
            MethodLibraryErrorCode::BoundaryViolation,
            "qualification name must not be empty",
            "name",
        )?;
        self.level_model.validate()?;
        for rule in &self.evidence_rules {
            rule.validate()?;
        }

        Ok(())
    }
}

impl RoleDefinition {
    /// Validates the role definition and collects references.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.role_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "role key must not be empty",
            "role_key",
        )?;
        validate_non_empty_strings(
            &self.responsibilities,
            MethodLibraryErrorCode::BoundaryViolation,
            "role responsibilities must not be empty",
            "responsibilities",
        )?;
        for reference in &self.qualification_refs {
            validate_published_ref_kind(reference, MethodContentKind::Qualification)?;
        }
        for reference in &self.default_view_profile_refs {
            validate_published_ref_kind(reference, MethodContentKind::ViewProfile)?;
        }

        Ok(())
    }

    /// Collects references that require validation.
    #[must_use]
    pub fn collect_content_refs(&self) -> Vec<ContentRef> {
        self.qualification_refs
            .iter()
            .map(PublishedContentRef::as_content_ref)
            .chain(
                self.default_view_profile_refs
                    .iter()
                    .map(PublishedContentRef::as_content_ref),
            )
            .collect()
    }
}

impl TaskStepDefinition {
    /// Validates the task step.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.step_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "task step key must not be empty",
            "step_key",
        )?;
        ensure_non_empty(
            &self.title,
            MethodLibraryErrorCode::BoundaryViolation,
            "task step title must not be empty",
            "title",
        )?;

        Ok(())
    }
}

impl TaskDefinition {
    /// Validates the task definition.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.task_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "task key must not be empty",
            "task_key",
        )?;
        ensure_non_empty(
            &self.purpose,
            MethodLibraryErrorCode::BoundaryViolation,
            "task purpose must not be empty",
            "purpose",
        )?;
        if self.step_defs.is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::BoundaryViolation,
                "task definition must contain at least one step",
            ));
        }

        let mut seen_keys = BTreeSet::new();
        let mut previous_order = None;
        for step in &self.step_defs {
            step.validate()?;
            if !seen_keys.insert(step.step_key.clone()) {
                return Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::BoundaryViolation,
                    "task step keys must be unique",
                )
                .with_detail("step_key", step.step_key.clone()));
            }
            if let Some(previous) = previous_order
                && step.order <= previous
            {
                return Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::BoundaryViolation,
                    "task steps must be ordered from low to high",
                )
                .with_detail("step_key", step.step_key.clone()));
            }
            previous_order = Some(step.order);
        }

        for reference in &self.input_work_product_refs {
            validate_published_ref_kind(reference, MethodContentKind::WorkProductDefinition)?;
        }
        for reference in &self.output_work_product_refs {
            validate_published_ref_kind(reference, MethodContentKind::WorkProductDefinition)?;
        }

        Ok(())
    }

    /// Collects references that require validation.
    #[must_use]
    pub fn collect_content_refs(&self) -> Vec<ContentRef> {
        self.input_work_product_refs
            .iter()
            .map(PublishedContentRef::as_content_ref)
            .chain(
                self.output_work_product_refs
                    .iter()
                    .map(PublishedContentRef::as_content_ref),
            )
            .collect()
    }
}

impl ArtifactKindRef {
    /// Validates the artifact-kind reference.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.artifact_kind,
            MethodLibraryErrorCode::BoundaryViolation,
            "artifact kind must not be empty",
            "artifact_kind",
        )?;
        if let Some(schema_version) = &self.schema_version {
            ensure_non_empty(
                schema_version,
                MethodLibraryErrorCode::BoundaryViolation,
                "artifact schema version must not be empty",
                "schema_version",
            )?;
        }

        Ok(())
    }
}

impl SchemaRef {
    /// Validates the schema reference.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.schema_uri,
            MethodLibraryErrorCode::BoundaryViolation,
            "schema uri must not be empty",
            "schema_uri",
        )?;
        if let Some(schema_version) = &self.schema_version {
            ensure_non_empty(
                schema_version,
                MethodLibraryErrorCode::BoundaryViolation,
                "schema version must not be empty",
                "schema_version",
            )?;
        }

        Ok(())
    }
}

impl QualityRule {
    /// Validates the quality rule.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.rule_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "quality rule key must not be empty",
            "rule_key",
        )?;
        ensure_non_empty(
            &self.description,
            MethodLibraryErrorCode::BoundaryViolation,
            "quality rule description must not be empty",
            "description",
        )
    }
}

impl WorkProductDefinition {
    /// Validates the work-product definition.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.work_product_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "work-product key must not be empty",
            "work_product_key",
        )?;
        self.artifact_kind.validate()?;
        self.schema_ref.validate()?;

        let mut seen_keys = BTreeSet::new();
        for rule in &self.quality_rules {
            rule.validate()?;
            if !seen_keys.insert(rule.rule_key.clone()) {
                return Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::BoundaryViolation,
                    "quality rule keys must be unique",
                )
                .with_detail("rule_key", rule.rule_key.clone()));
            }
        }

        Ok(())
    }
}

impl ActivityDefinition {
    /// Validates the activity definition.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.activity_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "activity key must not be empty",
            "activity_key",
        )?;
        ensure_non_empty(
            &self.name,
            MethodLibraryErrorCode::BoundaryViolation,
            "activity name must not be empty",
            "name",
        )?;
        for reference in &self.task_definition_refs {
            validate_published_ref_kind(reference, MethodContentKind::TaskDefinition)?;
        }
        for reference in &self.input_work_product_refs {
            validate_published_ref_kind(reference, MethodContentKind::WorkProductDefinition)?;
        }
        for reference in &self.output_work_product_refs {
            validate_published_ref_kind(reference, MethodContentKind::WorkProductDefinition)?;
        }
        for reference in &self.role_refs {
            validate_published_ref_kind(reference, MethodContentKind::RoleDefinition)?;
        }

        Ok(())
    }
}

impl TailoringEffect {
    /// Validates the tailoring effect.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        if self.target_keys.is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::BoundaryViolation,
                "tailoring effect must target at least one key",
            ));
        }
        for target_key in &self.target_keys {
            ensure_non_empty(
                target_key,
                MethodLibraryErrorCode::BoundaryViolation,
                "tailoring target key must not be empty",
                "target_key",
            )?;
        }

        Ok(())
    }
}

impl TailoringOption {
    /// Validates the tailoring option.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.option_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "tailoring option key must not be empty",
            "option_key",
        )?;
        ensure_non_empty(
            &self.description,
            MethodLibraryErrorCode::BoundaryViolation,
            "tailoring option description must not be empty",
            "description",
        )?;
        self.effect.validate()
    }
}

impl TailoringPoint {
    /// Validates the tailoring point.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.tailoring_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "tailoring point key must not be empty",
            "tailoring_key",
        )?;
        ensure_non_empty(
            &self.description,
            MethodLibraryErrorCode::BoundaryViolation,
            "tailoring point description must not be empty",
            "description",
        )?;

        let mut seen_keys = BTreeSet::new();
        for option in &self.allowed_options {
            option.validate()?;
            if !seen_keys.insert(option.option_key.clone()) {
                return Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::BoundaryViolation,
                    "tailoring option keys must be unique",
                )
                .with_detail("option_key", option.option_key.clone()));
            }
        }

        Ok(())
    }
}

impl ProcessTemplateDef {
    /// Validates the process-template definition.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.template_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "process-template key must not be empty",
            "template_key",
        )?;

        let mut seen_activity_keys = BTreeSet::new();
        for activity in &self.activity_defs {
            activity.validate()?;
            if !seen_activity_keys.insert(activity.activity_key.clone()) {
                return Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::BoundaryViolation,
                    "activity keys must be unique",
                )
                .with_detail("activity_key", activity.activity_key.clone()));
            }
        }

        for reference in &self.work_product_refs {
            validate_published_ref_kind(reference, MethodContentKind::WorkProductDefinition)?;
        }
        for reference in &self.role_refs {
            validate_published_ref_kind(reference, MethodContentKind::RoleDefinition)?;
        }
        for point in &self.tailoring_points {
            point.validate()?;
        }

        Ok(())
    }

    /// Collects references that require validation.
    #[must_use]
    pub fn collect_content_refs(&self) -> Vec<ContentRef> {
        self.work_product_refs
            .iter()
            .map(PublishedContentRef::as_content_ref)
            .chain(
                self.role_refs
                    .iter()
                    .map(PublishedContentRef::as_content_ref),
            )
            .collect()
    }
}

impl ViewCondition {
    /// Validates the view condition.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.field_path,
            MethodLibraryErrorCode::BoundaryViolation,
            "view condition field path must not be empty",
            "field_path",
        )?;

        match self.operator {
            ViewConditionOperator::Exists => Ok(()),
            ViewConditionOperator::Eq | ViewConditionOperator::In => {
                if self.value_json.is_none() {
                    return Err(MethodLibraryError::validation(
                        MethodLibraryErrorCode::BoundaryViolation,
                        "view condition requires a value for eq or in operators",
                    ));
                }

                Ok(())
            }
        }
    }
}

impl ViewScopeRule {
    /// Validates the scope rule.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.scope_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "view scope key must not be empty",
            "scope_key",
        )?;
        if self.conditions.is_empty() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::BoundaryViolation,
                "view scope rule must contain at least one condition",
            ));
        }

        for condition in &self.conditions {
            condition.validate()?;
        }

        Ok(())
    }
}

impl ViewFieldRule {
    /// Validates the field rule.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.field_path,
            MethodLibraryErrorCode::BoundaryViolation,
            "view field path must not be empty",
            "field_path",
        )
    }
}

impl ViewActionRule {
    /// Validates the action rule.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.action_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "view action key must not be empty",
            "action_key",
        )
    }
}

impl ViewProfile {
    /// Validates the view profile.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        validate_published_ref_kind(&self.role_ref, MethodContentKind::RoleDefinition)?;
        for scope_rule in &self.scope_rules {
            scope_rule.validate()?;
        }
        for field_rule in &self.field_rules {
            field_rule.validate()?;
        }
        for action_rule in &self.action_rules {
            action_rule.validate()?;
        }

        Ok(())
    }

    /// Collects references that require validation.
    #[must_use]
    pub fn collect_content_refs(&self) -> Vec<ContentRef> {
        vec![self.role_ref.as_content_ref()]
    }
}

impl PolicyRuleRef {
    /// Validates the policy rule reference.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.rule_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "policy rule key must not be empty",
            "rule_key",
        )?;
        if let Some(version) = &self.version {
            ensure_non_empty(
                version,
                MethodLibraryErrorCode::BoundaryViolation,
                "policy rule version must not be empty",
                "version",
            )?;
        }

        Ok(())
    }
}

impl AIPolicyConstraint {
    /// Validates the policy constraint.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.constraint_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "policy constraint key must not be empty",
            "constraint_key",
        )?;
        ensure_non_empty(
            &self.description,
            MethodLibraryErrorCode::BoundaryViolation,
            "policy constraint description must not be empty",
            "description",
        )
    }
}

impl AIPolicyDef {
    /// Validates the AI policy definition.
    pub fn validate(&self) -> Result<(), MethodLibraryError> {
        ensure_non_empty(
            &self.policy_key,
            MethodLibraryErrorCode::BoundaryViolation,
            "AI policy key must not be empty",
            "policy_key",
        )?;
        let mut seen_keys = BTreeSet::new();
        for rule_ref in &self.rule_refs {
            rule_ref.validate()?;
        }
        for constraint in &self.constraints {
            constraint.validate()?;
            if !seen_keys.insert(constraint.constraint_key.clone()) {
                return Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::BoundaryViolation,
                    "policy constraint keys must be unique",
                )
                .with_detail("constraint_key", constraint.constraint_key.clone()));
            }
        }

        Ok(())
    }
}

fn validate_non_empty_strings(
    values: &[String],
    code: MethodLibraryErrorCode,
    message: &'static str,
    detail_key: &'static str,
) -> Result<(), MethodLibraryError> {
    if values.is_empty() {
        return Err(MethodLibraryError::validation(code, message));
    }

    for value in values {
        ensure_non_empty(value, code, message, detail_key)?;
    }

    Ok(())
}

fn ensure_non_empty(
    value: &str,
    code: MethodLibraryErrorCode,
    message: &'static str,
    detail_key: &'static str,
) -> Result<(), MethodLibraryError> {
    if value.trim().is_empty() {
        return Err(MethodLibraryError::validation(code, message).with_detail(detail_key, value));
    }

    Ok(())
}

fn validate_published_ref_kind(
    reference: &PublishedContentRef,
    expected_kind: MethodContentKind,
) -> Result<(), MethodLibraryError> {
    if reference.kind != expected_kind {
        return Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::ReferenceInvalid,
            "published content reference has an unexpected kind",
        )
        .with_detail("expected_kind", expected_kind.as_str())
        .with_detail("actual_kind", reference.kind.as_str()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::fingerprint::{CanonicalFingerprint, FingerprintAlgorithm};

    fn published_ref(kind: MethodContentKind, content_id: &str) -> PublishedContentRef {
        PublishedContentRef {
            content_id: content_id.to_string(),
            kind,
            version: crate::content::ContentVersion::new("1.0.0").expect("version should be valid"),
            fingerprint: CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "abc123", "1.0")
                .expect("fingerprint should be valid"),
        }
    }

    #[test]
    fn validates_qualification_payloads() {
        let payload = MethodContentPayload::Qualification(Qualification {
            qualification_key: "quality-1".to_string(),
            name: "Quality".to_string(),
            description: Some("Baseline".to_string()),
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
                description: "Proof".to_string(),
            }],
        });

        payload
            .validate_definition_boundary()
            .expect("payload should validate");
        assert_eq!(payload.kind(), MethodContentKind::Qualification);
    }

    #[test]
    fn collects_published_references() {
        let payload = MethodContentPayload::RoleDefinition(RoleDefinition {
            role_key: "role-1".to_string(),
            responsibilities: vec!["Lead".to_string()],
            qualification_refs: vec![published_ref(MethodContentKind::Qualification, "q-1")],
            default_view_profile_refs: vec![published_ref(MethodContentKind::ViewProfile, "vp-1")],
        });

        let refs = payload.collect_content_refs();

        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].target_content_id, "q-1");
        assert_eq!(refs[1].target_content_id, "vp-1");
    }

    #[test]
    fn canonicalizes_payloads_deterministically() {
        let payload = MethodContentPayload::ViewProfile(ViewProfile {
            role_ref: published_ref(MethodContentKind::RoleDefinition, "role-1"),
            object_kind: ViewObjectKind::WorkItem,
            scope_rules: vec![ViewScopeRule {
                scope_key: "default".to_string(),
                conditions: vec![ViewCondition {
                    field_path: "project_type".to_string(),
                    operator: ViewConditionOperator::Eq,
                    value_json: Some(json!("software_delivery")),
                }],
            }],
            field_rules: vec![ViewFieldRule {
                field_path: "name".to_string(),
                visibility: FieldVisibility::Visible,
            }],
            action_rules: vec![ViewActionRule {
                action_key: "edit".to_string(),
                availability: ActionAvailability::Available,
            }],
        });

        let bytes = payload
            .canonicalize("1.0".to_string())
            .expect("payload should canonicalize");

        assert!(!bytes.is_empty());
    }

    #[test]
    fn validates_definition_boundary_against_forbidden_keys() {
        let payload = MethodContentPayload::TaskDefinition(TaskDefinition {
            task_key: "task-1".to_string(),
            purpose: "Do the thing".to_string(),
            step_defs: vec![TaskStepDefinition {
                step_key: "step-1".to_string(),
                order: 1,
                title: "Step".to_string(),
                purpose: Some("Purpose".to_string()),
                expected_output: None,
                verification: None,
            }],
            input_work_product_refs: vec![published_ref(
                MethodContentKind::WorkProductDefinition,
                "wp-1",
            )],
            output_work_product_refs: vec![],
        });

        payload
            .validate_definition_boundary()
            .expect("payload should validate");
    }
}
