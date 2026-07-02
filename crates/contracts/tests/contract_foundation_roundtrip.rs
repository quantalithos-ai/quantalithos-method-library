use method_library_contracts::fixtures::{
    sample_command_shell, sample_event_shell, sample_job_shell, sample_query_shell,
    sample_view_shell,
};
use method_library_contracts::{
    MethodLibraryCapabilityKind, MethodLibraryOperationsJobKind, MethodLibraryTypedBoundaryRefKind,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

fn roundtrip<T>(value: &T)
where
    T: Clone + Debug + DeserializeOwned + Eq + Serialize,
{
    let encoded = serde_json::to_value(value).expect("value should serialize");
    let decoded: T =
        serde_json::from_value(encoded).expect("value should deserialize after roundtrip");
    assert_eq!(decoded, *value);
}

#[test]
fn command_query_event_job_and_view_shells_roundtrip() {
    roundtrip(&sample_command_shell());
    roundtrip(&sample_query_shell());
    roundtrip(&sample_event_shell());
    roundtrip(&sample_job_shell());
    roundtrip(&sample_view_shell());
}

#[test]
fn capability_and_job_kind_are_stable() {
    let capability = MethodLibraryCapabilityKind::DefinitionCatalog;
    let encoded = serde_json::to_string(&capability).expect("capability should serialize");
    assert_eq!(encoded, "\"definition_catalog\"");

    let job_kind = MethodLibraryOperationsJobKind::RefreshCatalogDefinitionReadMaterials;
    let encoded = serde_json::to_string(&job_kind).expect("job kind should serialize");
    assert_eq!(encoded, "\"refresh_catalog_definition_read_materials\"");
}

#[test]
fn typed_boundary_ref_kind_is_stable() {
    let kind = MethodLibraryTypedBoundaryRefKind::MethodAssetDefinitionEstablishIntent;
    let encoded = serde_json::to_string(&kind).expect("ref kind should serialize");
    assert_eq!(encoded, "\"method_asset_definition_establish_intent\"");
}
