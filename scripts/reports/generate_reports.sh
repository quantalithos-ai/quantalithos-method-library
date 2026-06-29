#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/reports/generate_reports.sh --run-id <run_id> --artifact-root <path> --report-root <path>
EOF
}

run_id=""
artifact_root=""
report_root=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --run-id)
      run_id="${2:-}"
      shift 2
      ;;
    --artifact-root)
      artifact_root="${2:-}"
      shift 2
      ;;
    --report-root)
      report_root="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "${run_id}" || -z "${artifact_root}" || -z "${report_root}" ]]; then
  usage >&2
  exit 1
fi

if [[ "${artifact_root}" != "artifacts/test/${run_id}" ]]; then
  echo "Artifact root must be artifacts/test/<run_id> for the provided run id." >&2
  exit 1
fi

if [[ "${report_root}" != "reports/runs/${run_id}" ]]; then
  echo "Report root must be reports/runs/<run_id> for the provided run id." >&2
  exit 1
fi

echo "Dry run OK"
echo "run_id=${run_id}"
echo "artifact_root=${artifact_root}"
echo "report_root=${report_root}"
