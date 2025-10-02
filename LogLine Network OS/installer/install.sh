#!/usr/bin/env bash
set -euo pipefail

show_help() {
  cat <<'USAGE'
Usage: installer/install.sh [DESTINATION]

Copies the LogLine bundle (manifests, modules, optional bin/docs/profiles) to DESTINATION.
Defaults to /opt/logline when DESTINATION is omitted.
USAGE
}

if [[ ${1-} == "--help" ]]; then
  show_help
  exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUNDLE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEST="${1:-/opt/logline}"

BIN_SRC="${BUNDLE_ROOT}/bin"
MODULE_SRC="${BUNDLE_ROOT}/modules"
MANIFEST_SRC="${BUNDLE_ROOT}/manifests"
DOC_SRC="${BUNDLE_ROOT}/docs"
PROFILE_SRC="${BUNDLE_ROOT}/profiles"

mkdir -p "${DEST}/bin" "${DEST}/modules" "${DEST}/manifests" "${DEST}/docs" "${DEST}/profiles"

if [[ -d "${MODULE_SRC}" ]]; then
  cp -R "${MODULE_SRC}/"*.lll "${DEST}/modules/"
fi

if [[ -d "${MANIFEST_SRC}" ]]; then
  cp -R "${MANIFEST_SRC}/"*.lll "${DEST}/manifests/"
fi

declare -a opt_dirs=("${BIN_SRC}" "${DOC_SRC}" "${PROFILE_SRC}")
declare -a dest_dirs=("${DEST}/bin" "${DEST}/docs" "${DEST}/profiles")

for idx in "${!opt_dirs[@]}"; do
  src="${opt_dirs[$idx]}"
  target="${dest_dirs[$idx]}"
  if [[ -d "${src}" ]]; then
    cp -R "${src}/"* "${target}/"
  fi
done

echo "LogLine bundle installed at ${DEST}"
echo "- Modules:    ${DEST}/modules"
echo "- Manifests:  ${DEST}/manifests"
if [[ -d "${BIN_SRC}" ]]; then
  echo "- Binary:     ${DEST}/bin"
fi
if [[ -d "${DOC_SRC}" ]]; then
  echo "- Docs:       ${DEST}/docs"
fi
if [[ -d "${PROFILE_SRC}" ]]; then
  echo "- Profiles:   ${DEST}/profiles"
fi
