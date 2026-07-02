# service-flow-fast

- run_id: `20260702T155112Z-commit-04-b`
- suite: `service-flow-fast`
- artifact_root: `artifacts/test/20260702T155112Z-commit-04-b/suites/service-flow-fast`
- status: `pass`

## Commands

```text
cargo test -p method-library-contracts --test formalization_contracts
cargo test -p method-library-application --test formalization_version_foundation
cargo test -p method-library-infra --test formalization_version_runtime
cargo test -p method-library-api --test formalization_version_entry
bash scripts/checks/check_paths.sh --run-id 20260702T155112Z-commit-04-b --artifact-root artifacts/test/20260702T155112Z-commit-04-b --report-root reports/runs/20260702T155112Z-commit-04-b
bash scripts/reports/generate_reports.sh --run-id 20260702T155112Z-commit-04-b --artifact-root artifacts/test/20260702T155112Z-commit-04-b --report-root reports/runs/20260702T155112Z-commit-04-b
```

## Raw Artifacts

- `artifacts/test/20260702T155112Z-commit-04-b/suites/service-flow-fast/test-contracts.txt`
- `artifacts/test/20260702T155112Z-commit-04-b/suites/service-flow-fast/test-application.txt`
- `artifacts/test/20260702T155112Z-commit-04-b/suites/service-flow-fast/test-infra.txt`
- `artifacts/test/20260702T155112Z-commit-04-b/suites/service-flow-fast/test-api.txt`
- `artifacts/test/20260702T155112Z-commit-04-b/path-check.txt`
- `artifacts/test/20260702T155112Z-commit-04-b/report-dry-run.txt`

## Summary

- `formalization_contracts` passed 8 tests.
- `formalization_version_foundation` passed 3 tests.
- `formalization_version_runtime` passed 2 tests.
- `formalization_version_entry` passed 1 test.
- Path validation and report dry run both succeeded for the same run-scoped artifact/report roots.
