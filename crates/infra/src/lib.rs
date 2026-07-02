//! Infrastructure skeleton for the method library workspace.

pub mod definition_catalog;
pub mod config {}
pub mod runtime_builder {}
pub mod repositories {}
pub mod material_stores {}
pub mod reference_stores {}
pub mod external_adapters {}
pub mod publishers {}
pub mod handoff_adapters {}
pub mod clock_id {}
pub mod errors {}

pub use definition_catalog::{
    InMemoryMethodAssetCatalogEntryRepository, InMemoryMethodAssetDefinitionCatalogRuntime,
    InMemoryMethodAssetDefinitionCatalogSupportRefFactory, InMemoryMethodAssetDefinitionRepository,
    InMemoryMethodAssetStoredOperationResultRepository, InMemoryUnitOfWorkFactory,
};
