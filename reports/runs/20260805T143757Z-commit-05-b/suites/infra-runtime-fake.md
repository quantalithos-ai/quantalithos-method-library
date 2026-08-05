# infra-runtime-fake

- run_id: `20260805T143757Z-commit-05-b`
- suite: `infra-runtime-fake`
- artifact_root: `artifacts/test/20260805T143757Z-commit-05-b/suites/infra-runtime-fake`
- status: `pass`

## Commands

```text
cargo check -p method-library-infra
cargo test -p method-library-infra --test distribution_handoff_runtime
cargo test -p method-library-infra --test definition_catalog_runtime
```

## Raw Artifacts

- `artifacts/test/20260805T143757Z-commit-05-b/suites/infra-runtime-fake/cargo-check-infra.txt`
- `artifacts/test/20260805T143757Z-commit-05-b/suites/infra-runtime-fake/test-distribution-handoff-runtime.txt`
- `artifacts/test/20260805T143757Z-commit-05-b/suites/infra-runtime-fake/test-definition-catalog-regression.txt`

## Summary

- Infra check passed.
- Distribution/handoff fake runtime passed 15 tests, covering typed disabled diagnostics, adapter-first blocked/unavailable outcomes, target-state mapping, no publisher/handoff calls on safe precheck branches, duplicate replay, commit-unknown read-back and accepted-truth preservation.
- Definition/catalog regression runtime passed 3 tests, preserving prior lifecycle/status and rollback behavior.
