//! API entry skeleton for the method library workspace.

pub mod definition_catalog;
pub mod formalization_version;
pub mod command_handlers {}
pub mod query_handlers {}
pub mod routes {}
pub mod errors {}

pub use definition_catalog::MethodAssetApiCommandHandlerEntry;
pub use formalization_version::MethodAssetFormalizationVersionApiCommandHandlerEntry;
