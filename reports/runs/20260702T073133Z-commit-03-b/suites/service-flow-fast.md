# service-flow-fast

- run_id: `20260702T073133Z-commit-03-b`
- suite: `service-flow-fast`
- artifact_root: `artifacts/test/20260702T073133Z-commit-03-b/suites/service-flow-fast`
- status: `pass`

## Commands

```text
cargo test -p method-library-application -p method-library-api
bash scripts/checks/check_paths.sh --run-id 20260702T073133Z-commit-03-b --artifact-root artifacts/test/20260702T073133Z-commit-03-b --report-root reports/runs/20260702T073133Z-commit-03-b
bash scripts/reports/generate_reports.sh --run-id 20260702T073133Z-commit-03-b --artifact-root artifacts/test/20260702T073133Z-commit-03-b --report-root reports/runs/20260702T073133Z-commit-03-b
```

## Raw Artifacts

- `artifacts/test/20260702T073133Z-commit-03-b/suites/service-flow-fast/test-output.txt`
- `artifacts/test/20260702T073133Z-commit-03-b/path-check.txt`
- `artifacts/test/20260702T073133Z-commit-03-b/report-dry-run.txt`

## Summary

- `definition_catalog_foundation` passed 3 tests.
- `definition_catalog_entry` passed 1 test.
- No test failed, no test was ignored, and both package doc-test runs passed.
- Path validation and report dry run both succeeded for the same run-scoped artifact/report roots.
