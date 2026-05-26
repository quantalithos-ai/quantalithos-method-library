//! Domain policies shared by application services and workers.

use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};

use crate::content::{
    ApprovedGateRef, CanonicalBytes, CanonicalSchemaVersion, ContentRef, LifecycleState,
    MethodContent, PublishedContentRef, ReferenceState, canonicalize_json,
};
use crate::definitions::{
    MethodContentPayload, ViewActionRule, ViewConditionOperator, ViewFieldRule, ViewObjectKind,
    ViewProfile, ViewScopeRule,
};
use crate::error::{MethodLibraryError, MethodLibraryErrorCode};

/// Result of validating published references for a content aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceValidationResult {
    /// The source content identifier.
    pub content_id: String,
    /// References resolved to published targets.
    pub published_refs: Vec<PublishedContentRef>,
}

/// Candidate view-profile entry used during view resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewProfileCandidate {
    /// Published content reference of the candidate profile.
    pub content_ref: PublishedContentRef,
    /// View-profile payload.
    pub profile: ViewProfile,
}

/// Input required to resolve a published view profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewProfileResolveRequest {
    /// Role reference that should match the profile.
    pub role_ref: PublishedContentRef,
    /// Object kind that should match the profile.
    pub object_kind: ViewObjectKind,
    /// Structured scope input.
    pub scope: JsonValue,
}

/// Matched view-profile rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewProfileResolveResult {
    /// Published reference of the matched profile.
    pub view_profile_ref: PublishedContentRef,
    /// Matched scope rules.
    pub scope_rules: Vec<ViewScopeRule>,
    /// Matched field rules.
    pub field_rules: Vec<ViewFieldRule>,
    /// Matched action rules.
    pub action_rules: Vec<ViewActionRule>,
}

/// Guard that prevents Use truth from entering definition payloads.
pub struct DefinitionUseBoundaryGuard;

impl DefinitionUseBoundaryGuard {
    /// Ensures the payload only carries definition truth.
    pub fn ensure_definition_only(
        payload: &MethodContentPayload,
    ) -> Result<(), MethodLibraryError> {
        payload.validate_definition_boundary()
    }
}

/// Policy that validates publish preconditions.
pub struct PublishPolicy;

impl PublishPolicy {
    /// Validates publish preconditions for a content aggregate.
    pub fn validate_publish(
        content: &MethodContent,
        gate_ref: &ApprovedGateRef,
        refs: &[PublishedContentRef],
    ) -> Result<(), MethodLibraryError> {
        if !gate_ref.is_complete() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishGateRequired,
                "publish requires a complete approved gate reference",
            ));
        }

        if content.lifecycle.state != LifecycleState::InReview {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::LifecycleTransitionNotAllowed,
                "only in-review content may be published",
            )
            .with_detail("lifecycle_state", content.lifecycle.state.as_str()));
        }

        if content.version.is_some() || content.fingerprint.is_some() {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::PublishedContentImmutable,
                "published metadata may not be overwritten",
            ));
        }

        ReferenceValidationPolicy::validate_published_refs(content, &content.references, refs)?;
        Ok(())
    }
}

/// Policy that validates published references against draft references.
pub struct ReferenceValidationPolicy;

impl ReferenceValidationPolicy {
    /// Validates that every draft reference resolves to a published target.
    pub fn validate_published_refs(
        content: &MethodContent,
        refs: &[ContentRef],
        resolved_refs: &[PublishedContentRef],
    ) -> Result<ReferenceValidationResult, MethodLibraryError> {
        let mut published_refs = Vec::with_capacity(refs.len());

        for reference in refs {
            if !reference.target_kind.is_definition_kind() {
                return Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::ReferenceInvalid,
                    "reference target kind is outside the definition boundary",
                )
                .with_detail("target_kind", reference.target_kind.as_str()));
            }

            let resolved = resolved_refs
                .iter()
                .find(|candidate| {
                    candidate.content_id == reference.target_content_id
                        && candidate.kind == reference.target_kind
                })
                .ok_or_else(|| {
                    MethodLibraryError::validation(
                        MethodLibraryErrorCode::ReferenceNotPublished,
                        "reference target is not available as a published definition",
                    )
                    .with_detail("target_content_id", reference.target_content_id.clone())
                    .with_detail("target_kind", reference.target_kind.as_str())
                })?;

            if matches!(
                reference.required_state,
                ReferenceState::Published | ReferenceState::PublishedLike
            ) && resolved.content_id.trim().is_empty()
            {
                return Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::ReferenceNotPublished,
                    "published reference must carry a target content identifier",
                ));
            }

            published_refs.push(resolved.clone());
        }

        Ok(ReferenceValidationResult {
            content_id: content.content_id.clone(),
            published_refs,
        })
    }
}

/// Policy that canonicalizes a content aggregate for fingerprinting.
pub struct FingerprintPolicy;

impl FingerprintPolicy {
    /// Builds canonical bytes from a content aggregate.
    pub fn canonicalize(
        content: &MethodContent,
        schema_version: CanonicalSchemaVersion,
    ) -> Result<CanonicalBytes, MethodLibraryError> {
        DefinitionUseBoundaryGuard::ensure_definition_only(&content.payload)?;
        content.ensure_payload_matches_kind()?;

        let canonical = json!({
            "schema_version": schema_version,
            "content_id": content.content_id,
            "content_family_id": content.content_family_id,
            "kind": content.kind,
            "name": content.name,
            "description": content.description,
            "payload": content.payload,
            "references": content.references,
        });

        Ok(canonicalize_json(&canonical))
    }
}

/// Policy that matches the best published view profile for a request.
pub struct ViewProfileMatchPolicy;

impl ViewProfileMatchPolicy {
    /// Resolves the first candidate matching the role, object kind, and scope rules.
    pub fn match_profile(
        request: &ViewProfileResolveRequest,
        profiles: &[ViewProfileCandidate],
    ) -> Result<ViewProfileResolveResult, MethodLibraryError> {
        for candidate in profiles {
            if candidate.profile.role_ref.content_id != request.role_ref.content_id
                || candidate.profile.object_kind != request.object_kind
            {
                continue;
            }

            if candidate.profile.scope_rules.is_empty()
                || candidate
                    .profile
                    .scope_rules
                    .iter()
                    .any(|scope_rule| scope_rule_matches(scope_rule, &request.scope))
            {
                return Ok(ViewProfileResolveResult {
                    view_profile_ref: candidate.content_ref.clone(),
                    scope_rules: candidate.profile.scope_rules.clone(),
                    field_rules: candidate.profile.field_rules.clone(),
                    action_rules: candidate.profile.action_rules.clone(),
                });
            }
        }

        Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::MethodContentNotFound,
            "no published view profile matched the request",
        ))
    }
}

fn scope_rule_matches(rule: &ViewScopeRule, scope: &JsonValue) -> bool {
    rule.conditions.iter().all(|condition| {
        condition_matches(
            condition.field_path.as_str(),
            condition.operator,
            condition.value_json.as_ref(),
            scope,
        )
    })
}

fn condition_matches(
    field_path: &str,
    operator: ViewConditionOperator,
    expected_value: Option<&JsonValue>,
    scope: &JsonValue,
) -> bool {
    let Some(actual_value) = scope_lookup(scope, field_path) else {
        return false;
    };

    match operator {
        ViewConditionOperator::Exists => true,
        ViewConditionOperator::Eq => expected_value == Some(actual_value),
        ViewConditionOperator::In => match expected_value {
            Some(JsonValue::Array(values)) => values.iter().any(|value| value == actual_value),
            Some(value) => match actual_value {
                JsonValue::Array(values) => values.iter().any(|candidate| candidate == value),
                _ => actual_value == value,
            },
            None => false,
        },
    }
}

fn scope_lookup<'a>(scope: &'a JsonValue, field_path: &str) -> Option<&'a JsonValue> {
    let mut current = scope;
    for segment in field_path.split('.') {
        current = current.get(segment)?;
    }

    Some(current)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use time::macros::datetime;

    use super::*;
    use crate::content::{
        CanonicalFingerprint, ContentVersion, FingerprintAlgorithm, MethodContentKind,
    };
    use crate::definitions::{
        ActionAvailability, EvidenceKind, EvidenceRule, FieldVisibility, MethodContentPayload,
        Qualification, QualificationLevel, QualificationLevelModel, ViewActionRule, ViewCondition,
        ViewFieldRule, ViewProfile, ViewScopeRule,
    };

    fn published_ref(kind: MethodContentKind, content_id: &str) -> PublishedContentRef {
        PublishedContentRef {
            content_id: content_id.to_string(),
            kind,
            version: ContentVersion::new("1.0.0").expect("version should be valid"),
            fingerprint: CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "abc123", "1.0")
                .expect("fingerprint should be valid"),
        }
    }

    fn sample_content() -> MethodContent {
        MethodContent::create_draft(
            "content-1".to_string(),
            "family-1".to_string(),
            MethodContentKind::Qualification,
            "Quality".to_string(),
            None,
            MethodContentPayload::Qualification(Qualification {
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
            }),
            "actor-1".to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("content should be valid")
    }

    #[test]
    fn rejects_publish_without_gate() {
        let mut content = sample_content();
        content
            .submit_for_review("actor-2".to_string(), datetime!(2026-05-26 09:00:00 UTC))
            .expect("draft should move to review");

        let error = PublishPolicy::validate_publish(
            &content,
            &ApprovedGateRef {
                gate_id: " ".to_string(),
                gate_decision_id: "decision-1".to_string(),
                approved_at: datetime!(2026-05-26 09:10:00 UTC),
            },
            &[],
        )
        .expect_err("blank gate should be rejected");

        assert_eq!(error.code, MethodLibraryErrorCode::PublishGateRequired);
    }

    #[test]
    fn canonicalizes_content_for_fingerprinting() {
        let bytes = FingerprintPolicy::canonicalize(&sample_content(), "1.0".to_string())
            .expect("content should canonicalize");

        assert!(!bytes.is_empty());
    }

    #[test]
    fn matches_view_profile_candidates() {
        let request = ViewProfileResolveRequest {
            role_ref: published_ref(MethodContentKind::RoleDefinition, "role-1"),
            object_kind: ViewObjectKind::WorkItem,
            scope: json!({
                "project": {
                    "type": "software_delivery"
                }
            }),
        };
        let candidate = ViewProfileCandidate {
            content_ref: published_ref(MethodContentKind::ViewProfile, "vp-1"),
            profile: ViewProfile {
                role_ref: request.role_ref.clone(),
                object_kind: ViewObjectKind::WorkItem,
                scope_rules: vec![ViewScopeRule {
                    scope_key: "default".to_string(),
                    conditions: vec![ViewCondition {
                        field_path: "project.type".to_string(),
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
            },
        };

        let result = ViewProfileMatchPolicy::match_profile(&request, &[candidate])
            .expect("profile should match");

        assert_eq!(result.view_profile_ref.content_id, "vp-1");
    }
}
