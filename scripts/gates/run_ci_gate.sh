#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/gates/run_ci_gate.sh --run-id <run_id> --artifact-root <path> --config-profile <profile>
EOF
}

run_id=""
artifact_root=""
config_profile=""

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
    --config-profile)
      config_profile="${2:-}"
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

if [[ -z "${run_id}" || -z "${artifact_root}" || -z "${config_profile}" ]]; then
  usage >&2
  exit 1
fi

case "${config_profile}" in
  local-dev|ci-test|integration-like|operations-replay)
    ;;
  *)
    echo "Unsupported config profile: ${config_profile}" >&2
    exit 1
    ;;
esac

profile_path="config/profiles/${config_profile}.json"
if [[ ! -f "${profile_path}" ]]; then
  echo "Missing config profile skeleton: ${profile_path}" >&2
  exit 1
fi

if [[ "${artifact_root}" != "artifacts/test/${run_id}" ]]; then
  echo "Artifact root must be artifacts/test/<run_id> for the provided run id." >&2
  exit 1
fi

echo "Dry run OK"
echo "run_id=${run_id}"
echo "artifact_root=${artifact_root}"
echo "config_profile=${config_profile}"
echo "profile_path=${profile_path}"
