#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/checks/check_config_profiles.sh --config-profile <profile>
EOF
}

config_profile=""

while [[ $# -gt 0 ]]; do
  case "$1" in
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

if [[ -z "${config_profile}" ]]; then
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

if [[ ! -s "${profile_path}" ]]; then
  echo "Profile skeleton must not be empty: ${profile_path}" >&2
  exit 1
fi

python3 - <<'PY' "${profile_path}"
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
with path.open("r", encoding="utf-8") as handle:
    json.load(handle)
PY

echo "Config profile OK"
echo "config_profile=${config_profile}"
echo "profile_path=${profile_path}"
