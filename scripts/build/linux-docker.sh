#!/usr/bin/env bash
# Builds the Linux .deb and .AppImage inside ubuntu:22.04, from Linux or Git Bash on Windows.
# The repository is copied into the container, so the host node_modules and target stay untouched.
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

BASE_IMAGE="ubuntu:22.04"
NODE_VERSION="22.12.0"
OUTPUT_SUBDIR="releases/linux"
CARGO_VOLUME="wukong-imager-cargo"
TARGET_VOLUME="wukong-imager-target"

log_info() { printf '[INFO] %s\n' "$*"; }
log_error() { printf '[ERROR] %s\n' "$*" >&2; }

usage() {
  cat <<USAGE
Usage: $(basename "$0") [--test] [--clean] [-h|--help] [-- <test name filters>]

  (no flag)  Build .deb and .AppImage into ${OUTPUT_SUBDIR}/
  --test     Run the Rust tests, fmt and clippy instead of bundling
  --clean    Drop the cached cargo and target volumes first
USAGE
}

MODE="bundle"
CLEAN=false
TEST_ARGS=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --test) MODE="test"; shift ;;
    --clean) CLEAN=true; shift ;;
    -h|--help) usage; exit 0 ;;
    --) shift; TEST_ARGS="$*"; break ;;
    *) log_error "Unknown argument: $1"; usage >&2; exit 2 ;;
  esac
done

if ! command -v docker >/dev/null 2>&1; then
  log_error "Docker not found. Install Docker first: https://docs.docker.com/get-docker/"
  exit 1
fi

# Docker Desktop in Windows containers mode still has its Linux engine under another context.
if [ "$(docker version --format '{{.Server.Os}}' 2>/dev/null || true)" != "linux" ]; then
  if docker context inspect desktop-linux >/dev/null 2>&1; then
    export DOCKER_CONTEXT=desktop-linux
  else
    log_error "Docker has no Linux engine: switch it to Linux containers"
    exit 1
  fi
fi

# Git Bash would rewrite the container paths below, and Docker wants a Windows path for the mount.
HOST_ROOT="$REPO_ROOT"
if command -v cygpath >/dev/null 2>&1; then
  HOST_ROOT="$(cygpath -w "$REPO_ROOT")"
  export MSYS_NO_PATHCONV=1
fi

if [ "$CLEAN" = true ]; then
  log_info "Dropping cached volumes"
  docker volume rm -f "$CARGO_VOLUME" "$TARGET_VOLUME" >/dev/null
fi

mkdir -p "${REPO_ROOT}/${OUTPUT_SUBDIR}"

log_info "Running in ${BASE_IMAGE} (mode: ${MODE})"
docker run --rm \
  -e MODE="$MODE" \
  -e TEST_ARGS="$TEST_ARGS" \
  -e NODE_VERSION="$NODE_VERSION" \
  -e OUTPUT_SUBDIR="$OUTPUT_SUBDIR" \
  -e CARGO_HOME=/root/.cargo \
  -e RUSTUP_HOME=/root/.cargo/rustup \
  -e APPIMAGE_EXTRACT_AND_RUN=1 \
  -e NO_STRIP=1 \
  -v "${HOST_ROOT}:/source" \
  -v "${CARGO_VOLUME}:/root/.cargo" \
  -v "${TARGET_VOLUME}:/build/src-tauri/target" \
  "$BASE_IMAGE" \
  bash -c '
    set -euo pipefail

    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends \
      build-essential ca-certificates curl file git patchelf pkg-config xz-utils \
      libayatana-appindicator3-dev libgtk-3-dev librsvg2-dev libssl-dev libudev-dev \
      libusb-1.0-0-dev libwebkit2gtk-4.1-dev libxdo-dev xdg-utils >/dev/null

    if ! ls "$RUSTUP_HOME"/toolchains/*/bin/cargo >/dev/null 2>&1; then
      curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal -c rustfmt -c clippy
    fi
    export PATH="/root/.cargo/bin:/opt/node/bin:$PATH"

    mkdir -p /opt/node
    curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" \
      | tar -xJ -C /opt/node --strip-components=1

    # target is a volume mounted inside /build: keep it, copy everything else fresh.
    tar -C /source \
      --exclude=./node_modules --exclude=./dist --exclude=./releases --exclude=./.git \
      --exclude=./.autopilot --exclude=./src-tauri/target --exclude="./crates/*/target" \
      -cf - . | tar -C /build -xf -

    cd /build
    if [ "$MODE" = "test" ]; then
      mkdir -p dist
      cd src-tauri
      # shellcheck disable=SC2086
      cargo test -- $TEST_ARGS
      cargo fmt --check
      cargo clippy --all-targets -- -D warnings
      exit 0
    fi

    npm ci
    npx tauri build --bundles deb,appimage

    # Same step as CI. A Windows checkout has CRLF scripts, which bash refuses, and the
    # AppImage must really extract here, not extract and run the app.
    tr -d "\r" < scripts/build/appimage-unbundle-wayland.sh > /tmp/appimage-unbundle-wayland.sh
    # Extraction leaves 0700 directories, which the current appimagetool refuses to pack.
    sed -i "/--appimage-extract-and-run so appimagetool/i chmod -R a+rX \"\$EXTRACT_DIR/squashfs-root\"" \
      /tmp/appimage-unbundle-wayland.sh
    grep -q "chmod -R a+rX" /tmp/appimage-unbundle-wayland.sh
    env -u APPIMAGE_EXTRACT_AND_RUN bash /tmp/appimage-unbundle-wayland.sh

    out="/source/${OUTPUT_SUBDIR}"
    rm -f "$out"/*.deb "$out"/*.AppImage "$out"/*.appimage
    for f in src-tauri/target/release/bundle/deb/*.deb src-tauri/target/release/bundle/appimage/*.AppImage; do
      name="$(basename "$f")"
      name="${name// /-}"
      stem="${name%.*}"
      cp -f "$f" "$out/${stem,,}.${name##*.}"
    done

    # Fail the build if the published AppImage still carries libwayland.
    check_dir="$(mktemp -d)"
    app="$(realpath "$out"/*.AppImage)"
    (cd "$check_dir" && env -u APPIMAGE_EXTRACT_AND_RUN "$app" --appimage-extract >/dev/null)
    bundled="$(find "$check_dir/squashfs-root" -name "libwayland-*.so*")"
    rm -rf "$check_dir"
    if [ -n "$bundled" ]; then
      echo "AppImage still bundles libwayland:" >&2
      echo "$bundled" >&2
      exit 1
    fi
    echo "AppImage bundles no libwayland"

    dpkg-deb -f "$out"/*.deb Package Version
    ls -la "$out"
  '

log_info "Done"
