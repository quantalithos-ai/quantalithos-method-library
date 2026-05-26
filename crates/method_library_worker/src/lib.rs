//! Worker bootstrap surface for outbox relay and operations jobs.

/// Returns the placeholder worker name.
#[must_use]
pub fn worker_name() -> &'static str {
    "method-library-worker"
}

#[cfg(test)]
mod tests {
    use super::worker_name;

    #[test]
    fn exposes_worker_placeholder() {
        assert_eq!(worker_name(), "method-library-worker");
    }
}
