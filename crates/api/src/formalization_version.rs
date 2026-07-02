//! Minimal API command entry for the `commit-04-b` formalization/version slice.

use std::sync::{Arc, Mutex};

use method_library_application::{
    MethodAssetFormalizationVersionCommandDispatchInput,
    MethodAssetFormalizationVersionCommandDispatchOutput,
    MethodAssetFormalizationVersionCommandFacade, MethodAssetFormalizationVersionCommandSource,
    MethodAssetFormalizationVersionSupportRefFactory,
};
use method_library_contracts::MethodLibraryCommandShell;

/// Transport-neutral API command handler for the current boundary.
pub struct MethodAssetFormalizationVersionApiCommandHandlerEntry {
    facade: Arc<dyn MethodAssetFormalizationVersionCommandFacade>,
    support_ref_factory: Arc<Mutex<Box<dyn MethodAssetFormalizationVersionSupportRefFactory>>>,
}

impl MethodAssetFormalizationVersionApiCommandHandlerEntry {
    /// Creates the current-boundary API command entry.
    pub fn new(
        facade: Arc<dyn MethodAssetFormalizationVersionCommandFacade>,
        support_ref_factory: Arc<Mutex<Box<dyn MethodAssetFormalizationVersionSupportRefFactory>>>,
    ) -> Self {
        Self {
            facade,
            support_ref_factory,
        }
    }

    /// Handles a formalization/version command without bypassing the application facade.
    pub fn handle_formalization_version_command(
        &self,
        command_shell: MethodLibraryCommandShell,
        command_source: MethodAssetFormalizationVersionCommandSource,
    ) -> MethodAssetFormalizationVersionCommandDispatchOutput {
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
            factory.formalization_version_dispatch_ref()
        };

        self.facade.dispatch_formalization_version_command(
            MethodAssetFormalizationVersionCommandDispatchInput {
                command_shell,
                command_source,
                api_entry_context_ref,
                application_dispatch_ref,
            },
        )
    }
}
