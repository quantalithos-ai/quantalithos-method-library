# contract-domain-fast

- run_id: `20260701T050111Z-commit-03-a`
- boundary: `commit-03-a`
- scope: definition/catalog contracts and domain foundation
- status: pass

## Commands

- `cargo check`
- `cargo check -p method-library-contracts`
- `cargo check -p method-library-domain`
- `cargo test -p method-library-contracts`
- `cargo test -p method-library-domain`
- `bash scripts/checks/check_paths.sh --run-id 20260701T050111Z-commit-03-a --artifact-root artifacts/test/20260701T050111Z-commit-03-a --report-root reports/runs/20260701T050111Z-commit-03-a`
- `bash scripts/reports/generate_reports.sh --run-id 20260701T050111Z-commit-03-a --artifact-root artifacts/test/20260701T050111Z-commit-03-a --report-root reports/runs/20260701T050111Z-commit-03-a`

## Results

- Workspace check passed.
- `method-library-contracts` package check passed.
- `method-library-domain` package check passed.
- `method-library-contracts` tests passed: 3 integration tests in `contract_foundation_roundtrip`, 4 integration tests in `definition_catalog_contracts`.
- `method-library-domain` tests passed: 6 integration tests in `domain_foundation`, 5 integration tests in `method_asset_definition`, 2 compile-fail doc tests for `mark_deprecated` redlines.
- Path validation dry-run passed for the run-scoped artifact/report roots.
- Existing report script remains a dry-run shell; no additional generator logic was introduced in this boundary.

## Raw Artifacts

- `artifacts/test/20260701T050111Z-commit-03-a/suites/contract-domain-fast/cargo-check-workspace.txt`
- `artifacts/test/20260701T050111Z-commit-03-a/suites/contract-domain-fast/cargo-check-contracts.txt`
- `artifacts/test/20260701T050111Z-commit-03-a/suites/contract-domain-fast/cargo-check-domain.txt`
- `artifacts/test/20260701T050111Z-commit-03-a/suites/contract-domain-fast/cargo-test-contracts.txt`
- `artifacts/test/20260701T050111Z-commit-03-a/suites/contract-domain-fast/cargo-test-domain.txt`
- `artifacts/test/20260701T050111Z-commit-03-a/suites/contract-domain-fast/path-check.txt`
- `artifacts/test/20260701T050111Z-commit-03-a/suites/contract-domain-fast/report-dry-run.txt`
