//! Minimal API command entry for the `commit-03-b` definition/catalog slice.

use std::sync::{Arc, Mutex};

use method_library_application::{
    MethodAssetDefinitionCatalogCommandDispatchInput,
    MethodAssetDefinitionCatalogCommandDispatchOutput, MethodAssetDefinitionCatalogCommandFacade,
    MethodAssetDefinitionCatalogCommandSource, MethodAssetDefinitionCatalogSupportRefFactory,
};
use method_library_contracts::MethodLibraryCommandShell;

/// Transport-neutral API command handler for the current boundary.
pub struct MethodAssetApiCommandHandlerEntry {
    facade: Arc<dyn MethodAssetDefinitionCatalogCommandFacade>,
    support_ref_factory: Arc<Mutex<Box<dyn MethodAssetDefinitionCatalogSupportRefFactory>>>,
}

impl MethodAssetApiCommandHandlerEntry {
    /// Creates the current-boundary API command entry.
    pub fn new(
        facade: Arc<dyn MethodAssetDefinitionCatalogCommandFacade>,
        support_ref_factory: Arc<Mutex<Box<dyn MethodAssetDefinitionCatalogSupportRefFactory>>>,
    ) -> Self {
        Self {
            facade,
            support_ref_factory,
        }
    }

    /// Handles a definition/catalog command without bypassing the application facade.
    pub fn handle_definition_catalog_command(
        &self,
        command_shell: MethodLibraryCommandShell,
        command_source: MethodAssetDefinitionCatalogCommandSource,
    ) -> MethodAssetDefinitionCatalogCommandDispatchOutput {
        let api_entry_context_ref = {
            let mut factory = self
                .support_ref_factory
                .lock()
                .expect("support ref factory lock poisoned");
            factory.new_api_entry_context_ref()
        };
        let application_dispatch_ref = {
            let factory = self
                .support_ref_factory
                .lock()
                .expect("support ref factory lock poisoned");
            factory.definition_catalog_dispatch_ref()
        };

        self.facade.dispatch_definition_catalog_command(
            MethodAssetDefinitionCatalogCommandDispatchInput {
                command_shell,
                command_source,
                api_entry_context_ref,
                application_dispatch_ref,
            },
        )
    }
}
