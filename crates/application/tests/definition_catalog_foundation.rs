use method_library_application::ports::{
    ExternalSourceSummaryValidationPort, MethodAssetCatalogEntryRepository,
    MethodAssetDefinitionRepository, MethodAssetStoredOperationResultRepository,
};
use method_library_application::unit_of_work::{CommandUnitOfWork, UnitOfWork};
use method_library_application::{
    MethodAssetDefinitionCatalogCommandFacade, MethodAssetDefinitionCatalogCommandSelector,
    MethodAssetDefinitionCatalogSupportRefFactory, MethodAssetEffectSummaryRefSet,
    MethodAssetStoredOperationResultKind,
};
use method_library_contracts::MethodAssetEffectSummaryRef;

macro_rules! assert_object_safe {
    ($($trait_name:ty),+ $(,)?) => {
        $(
            let _: Option<&$trait_name> = None;
        )+
    };
}

#[test]
fn current_boundary_selectors_and_result_kinds_are_closed() {
    let selectors = [
        MethodAssetDefinitionCatalogCommandSelector::EstablishDefinition,
        MethodAssetDefinitionCatalogCommandSelector::AdjustDefinition,
        MethodAssetDefinitionCatalogCommandSelector::RetireDefinition,
        MethodAssetDefinitionCatalogCommandSelector::RegisterCatalogEntry,
        MethodAssetDefinitionCatalogCommandSelector::ReclassifyCatalogEntry,
        MethodAssetDefinitionCatalogCommandSelector::RetireCatalogEntry,
    ];
    assert_eq!(selectors.len(), 6);

    let result_kinds = [
        MethodAssetStoredOperationResultKind::Accepted,
        MethodAssetStoredOperationResultKind::Rejected,
        MethodAssetStoredOperationResultKind::Ignored,
        MethodAssetStoredOperationResultKind::Conflict,
    ];
    assert_eq!(result_kinds.len(), 4);
}

#[test]
fn effect_summary_ref_set_keeps_first_insertion_order_after_dedup() {
    let first = MethodAssetEffectSummaryRef::new("effect:first");
    let second = MethodAssetEffectSummaryRef::new("effect:second");

    let mut refs = MethodAssetEffectSummaryRefSet::new();
    refs.insert(first.clone());
    refs.insert(first.clone());
    refs.insert(second.clone());

    assert_eq!(refs.refs, vec![first, second]);
}

#[test]
fn application_ports_and_facade_surfaces_are_object_safe() {
    assert_object_safe!(
        dyn MethodAssetDefinitionRepository,
        dyn MethodAssetCatalogEntryRepository,
        dyn MethodAssetStoredOperationResultRepository,
        dyn ExternalSourceSummaryValidationPort,
        dyn CommandUnitOfWork,
        dyn UnitOfWork,
        dyn MethodAssetDefinitionCatalogCommandFacade,
        dyn MethodAssetDefinitionCatalogSupportRefFactory,
    );
}
