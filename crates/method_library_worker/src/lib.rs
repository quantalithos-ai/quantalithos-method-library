//! Worker bootstrap surface for outbox relay and operations jobs.

pub mod outbox_relay;

pub use outbox_relay::{OutboxRelaySettings, OutboxRelayWorker};

/// Returns the placeholder worker name.
#[must_use]
pub fn worker_name() -> &'static str {
    "method-library-worker"
}

#[cfg(test)]
mod tests {
    use super::{OutboxRelaySettings, worker_name};

    #[test]
    fn exposes_worker_placeholder() {
        assert_eq!(worker_name(), "method-library-worker");
    }

    #[test]
    fn exposes_outbox_relay_settings_shape() {
        let settings = OutboxRelaySettings {
            batch_size: 10,
            lease_seconds: 60,
            max_attempts: 3,
            retry_backoff_ms: 30_000,
            definition_events_topic: "method-library.definition.events".to_string(),
            lifecycle_events_topic: "method-library.lifecycle.events".to_string(),
        };

        assert_eq!(settings.batch_size, 10);
        assert_eq!(settings.max_attempts, 3);
    }
}
