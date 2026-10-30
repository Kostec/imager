#!/usr/bin/env bash
# End-to-end check of first boot settings on a real image, from Linux or Git Bash on Windows.
# Works on a copy: the image from WUKONG_TEST_IMAGE is mounted read-only.
#
#   WUKONG_TEST_IMAGE=/path/to/Armbian_*.img scripts/e2e/run.sh [--offline] [--keep]
#
# Exit codes: 0 all checks passed, 1 a check failed, 2 the run could not start,
# 3 the image is fine but the boot was not verified (see the last lines of the output).
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

BUILD_IMAGE="ubuntu:22.04"
TOOLS_IMAGE="wukong-imager-e2e"
CARGO_VOLUME="wukong-imager-cargo"
TARGET_VOLUME="wukong-imager-target"

log_info() { printf '[INFO] %s\n' "$*"; }
log_error() { printf '[ERROR] %s\n' "$*" >&2; }

usage() {
  cat <<USAGE
Usage: WUKONG_TEST_IMAGE=<image> $(basename "$0") [--offline] [--keep] [-h|--help]

  --offline   Stop after the checks of the image itself, no QEMU
  --keep      Copy the card, as it is at the end, into the working directory

Environment:
  WUKONG_TEST_IMAGE   Image to test, required; never changed
  E2E_WORK_DIR        For the report and the logs (default: a new temporary one)
  E2E_DTB             Device tree inside /boot/dtb (default: sun8i-h2-plus-wukongpi.dtb)
  E2E_BOOT_TIMEOUT    Seconds to wait for the login prompt (default: 3600)
  E2E_SETTLE_TIMEOUT  Seconds to wait for the stage after the network (default: 1200)
USAGE
}

OFFLINE=0
KEEP=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --offline) OFFLINE=1; shift ;;
    --keep) KEEP=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) log_error "Unknown argument: $1"; usage >&2; exit 2 ;;
  esac
done

if [[ -z "${WUKONG_TEST_IMAGE:-}" || ! -f "${WUKONG_TEST_IMAGE}" ]]; then
  log_error "Set WUKONG_TEST_IMAGE to an existing image file"
  exit 2
fi
if ! command -v docker >/dev/null 2>&1; then
  log_error "Docker not found"
  exit 2
fi

# Docker Desktop in Windows containers mode still has its Linux engine under another context.
if [ "$(docker version --format '{{.Server.Os}}' 2>/dev/null || true)" != "linux" ]; then
  if docker context inspect desktop-linux >/dev/null 2>&1; then
    export DOCKER_CONTEXT=desktop-linux
  else
    log_error "Docker has no Linux engine: switch it to Linux containers"
    exit 2
  fi
fi

WORK_DIR="${E2E_WORK_DIR:-$(mktemp -d)}"
mkdir -p "$WORK_DIR"
IMAGE_DIR="$(cd "$(dirname "$WUKONG_TEST_IMAGE")" && pwd)"
IMAGE_NAME="$(basename "$WUKONG_TEST_IMAGE")"
HOST_ROOT="$REPO_ROOT"
HOST_CONTEXT="$SCRIPT_DIR"
HOST_WORK="$(cd "$WORK_DIR" && pwd)"
HOST_IMAGE_DIR="$IMAGE_DIR"

# Git Bash would rewrite the container paths below, and Docker wants Windows paths for the mounts.
if command -v cygpath >/dev/null 2>&1; then
  HOST_ROOT="$(cygpath -w "$HOST_ROOT")"
  HOST_CONTEXT="$(cygpath -w "$HOST_CONTEXT")"
  HOST_WORK="$(cygpath -w "$HOST_WORK")"
  HOST_IMAGE_DIR="$(cygpath -w "$HOST_IMAGE_DIR")"
  export MSYS_NO_PATHCONV=1
fi

if [ -f "$WORK_DIR/provision" ] && [ "${E2E_REUSE_BUILD:-0}" = "1" ]; then
  log_info "Reusing the built provision tool"
else
  log_info "Building examples/provision.rs in ${BUILD_IMAGE}"
  docker run --rm \
    -e CARGO_HOME=/root/.cargo \
    -e RUSTUP_HOME=/root/.cargo/rustup \
    -v "${HOST_ROOT}:/source:ro" \
    -v "${HOST_WORK}:/work" \
    -v "${CARGO_VOLUME}:/root/.cargo" \
    -v "${TARGET_VOLUME}:/build/src-tauri/target" \
    "$BUILD_IMAGE" \
    bash /source/scripts/e2e/build-provision.sh
fi

log_info "Preparing the tools image"
docker build -q -t "$TOOLS_IMAGE" "$HOST_CONTEXT" >/dev/null

status=0
docker run --rm \
  -e E2E_OFFLINE="$OFFLINE" \
  -e E2E_KEEP="$KEEP" \
  -e E2E_DTB="${E2E_DTB:-sun8i-h2-plus-wukongpi.dtb}" \
  -e E2E_BOOT_TIMEOUT="${E2E_BOOT_TIMEOUT:-3600}" \
  -e E2E_SETTLE_TIMEOUT="${E2E_SETTLE_TIMEOUT:-1200}" \
  -v "${HOST_ROOT}:/source:ro" \
  -v "${HOST_IMAGE_DIR}:/input:ro" \
  -v "${HOST_WORK}:/work" \
  "$TOOLS_IMAGE" \
  bash /source/scripts/e2e/in-container.sh "/input/${IMAGE_NAME}" || status=$?

grep '^WARN' "$WORK_DIR/report.txt" || true

case "$status" in
  0) log_info "All checks passed. Report: ${WORK_DIR}/report.txt" ;;
  3) log_error "Image checks passed, the boot was NOT verified. Report: ${WORK_DIR}/report.txt" ;;
  *) log_error "Checks failed. Report: ${WORK_DIR}/report.txt" ;;
esac
exit "$status"
