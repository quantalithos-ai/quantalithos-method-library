# quantalithos-method-library

Rust workspace for the L3 method-library P0 implementation.

## Workspace Layout

- `crates/method_library_domain`: domain entities, policies, and shared errors.
- `crates/method_library_contracts`: command, query, event, snapshot, and job DTOs.
- `crates/method_library_application`: application services and port traits.
- `crates/method_library_infra`: PostgreSQL, blob, bus, and governance adapters.
- `crates/method_library_api`: HTTP entrypoint for commands, queries, snapshots, and jobs.
- `crates/method_library_worker`: outbox relay and operations runners.

## Feature Flags

- `p1-plugin`: reserved for the future MethodPlugin implementation. Disabled by default.
- `p1-configuration`: reserved for the future MethodConfiguration implementation. Disabled by default.

## Local Gates

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
```
