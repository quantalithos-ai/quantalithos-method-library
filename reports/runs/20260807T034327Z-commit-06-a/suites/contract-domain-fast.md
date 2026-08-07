# contract-domain-fast

- run_id: `20260807T034327Z-commit-06-a`
- boundary: `commit-06-a`
- scope: trace, impact, audit, and evidence-lineage contracts/domain
- raw_artifact_root: `artifacts/test/20260807T034327Z-commit-06-a/suites/contract-domain-fast`
- status: pass

## Commands

- `cargo fmt --all -- --check`
- `cargo check -p method-library-contracts`
- `cargo check -p method-library-domain`
- `cargo test -p method-library-contracts`
- `cargo test -p method-library-domain`
- Targeted forbidden-field scan over the `commit-06-a` contracts/domain sources and fixtures.

## Results

- Formatting and both package checks passed.
- Contracts tests passed: 29 integration tests, including 5 focused trace/audit contract tests.
- Domain tests passed: 41 integration tests and 3 compile-fail doctests, including 10 focused trace/audit state tests and the body-candidate type redline.
- Targeted redaction scan found no raw method/provider/archive/report/log body, path, secret, credential, token, stack-trace, raw-reason, config-value, or fake-only-marker fields.
- This run does not claim application service, repository, persistence, query, API, worker, job, replay, or report-generator coverage.

## Raw Artifacts

- `artifacts/test/20260807T034327Z-commit-06-a/suites/contract-domain-fast/cargo-fmt-check.txt`
- `artifacts/test/20260807T034327Z-commit-06-a/suites/contract-domain-fast/cargo-check-contracts.txt`
- `artifacts/test/20260807T034327Z-commit-06-a/suites/contract-domain-fast/cargo-check-domain.txt`
- `artifacts/test/20260807T034327Z-commit-06-a/suites/contract-domain-fast/cargo-test-contracts.txt`
- `artifacts/test/20260807T034327Z-commit-06-a/suites/contract-domain-fast/cargo-test-domain.txt`
- `artifacts/test/20260807T034327Z-commit-06-a/suites/contract-domain-fast/redaction-scan.txt`
