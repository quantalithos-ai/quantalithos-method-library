# service-flow-fast

- run_id: `20260805T143757Z-commit-05-b`
- suite: `service-flow-fast`
- artifact_root: `artifacts/test/20260805T143757Z-commit-05-b/suites/service-flow-fast`
- status: `pass`

## Commands

```text
cargo fmt --all -- --check
cargo check
cargo check -p method-library-application
cargo test --workspace
cargo test -p method-library-application
cargo test -p method-library-contracts
cargo test -p method-library-infra --test distribution_handoff_runtime
bash scripts/checks/check_paths.sh --run-id 20260805T143757Z-commit-05-b --artifact-root artifacts/test/20260805T143757Z-commit-05-b --report-root reports/runs/20260805T143757Z-commit-05-b
bash scripts/reports/generate_reports.sh --run-id 20260805T143757Z-commit-05-b --artifact-root artifacts/test/20260805T143757Z-commit-05-b --report-root reports/runs/20260805T143757Z-commit-05-b
```

## Raw Artifacts

- `artifacts/test/20260805T143757Z-commit-05-b/suites/service-flow-fast/cargo-fmt-check.txt`
- `artifacts/test/20260805T143757Z-commit-05-b/suites/service-flow-fast/cargo-check-workspace.txt`
- `artifacts/test/20260805T143757Z-commit-05-b/suites/service-flow-fast/cargo-check-application.txt`
- `artifacts/test/20260805T143757Z-commit-05-b/suites/service-flow-fast/cargo-test-workspace.txt`
- `artifacts/test/20260805T143757Z-commit-05-b/suites/service-flow-fast/test-application.txt`
- `artifacts/test/20260805T143757Z-commit-05-b/suites/service-flow-fast/test-contracts.txt`
- `artifacts/test/20260805T143757Z-commit-05-b/suites/service-flow-fast/test-distribution-handoff.txt`
- `artifacts/test/20260805T143757Z-commit-05-b/suites/service-flow-fast/dependency-boundary.txt`
- `artifacts/test/20260805T143757Z-commit-05-b/suites/service-flow-fast/redaction-boundary.txt`
- `artifacts/test/20260805T143757Z-commit-05-b/path-check.txt`
- `artifacts/test/20260805T143757Z-commit-05-b/report-dry-run.txt`

## Summary

- Workspace, application and format checks passed.
- Contract and application regression tests passed.
- Distribution/handoff runtime passed 15 tests, including selector/source safe rejection, duplicate no-rerun, commit-unknown replay, accepted-truth preservation, disabled/degraded/unavailable mapping, publisher/handoff no-call branches and post-commit persistence failure handling.
- Dependency-boundary and redaction-boundary scans passed; no legacy truth identifiers were found.
- Path validation and report dry run passed for the same run-scoped artifact/report roots.
