#!/usr/bin/env bash
# Builds the Linux .deb and .AppImage natively in WSL (Ubuntu 22.04), without Docker.
# Same steps as linux-docker.sh. The repository is copied into the WSL home, so the
# Windows node_modules and target stay untouched; the bundles land in releases/linux/.
#
# From the repository root in PowerShell or cmd:
#   wsl -u root bash scripts/build/linux-wsl.sh --deps   # once: system packages
#   wsl bash scripts/build/linux-wsl.sh                  # build as your WSL user
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

NODE_VERSION="22.12.0"
OUTPUT_SUBDIR="releases/linux"
BUILD_ROOT="${WUKONG_WSL_BUILD_DIR:-$HOME/.cache/wukong-imager-build}"
NODE_DIR="$HOME/.local/share/wukong-imager-node-${NODE_VERSION}"

log_info() { printf '[INFO] %s\n' "$*"; }
log_error() { printf '[ERROR] %s\n' "$*" >&2; }
# WSL's resolver drops a name now and then: retry, and never pipe a half download into sh or tar.
fetch() { curl -fsSL --retry 5 --retry-all-errors --retry-delay 3 -o "$2" "$1"; }

usage() {
  cat <<USAGE
Usage: $(basename "$0") [--deps] [--clean] [-h|--help]

  --deps   Install the system packages (as root or with sudo) and exit
  (none)   Build .deb and .AppImage into ${OUTPUT_SUBDIR}/; Rust and Node go into \$HOME if missing
  --clean  Drop the build copy in ${BUILD_ROOT} first
USAGE
}

MODE="build"
CLEAN=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --deps) MODE="deps"; shift ;;
    --clean) CLEAN=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) log_error "Unknown argument: $1"; usage >&2; exit 2 ;;
  esac
done

PACKAGES=(
  build-essential ca-certificates curl file git patchelf pkg-config xz-utils
  libayatana-appindicator3-dev libgtk-3-dev librsvg2-dev libssl-dev libudev-dev
  libusb-1.0-0-dev libwebkit2gtk-4.1-dev libxdo-dev xdg-utils
)

if [ "$MODE" = "deps" ]; then
  SUDO=""
  if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi
  export DEBIAN_FRONTEND=noninteractive
  $SUDO apt-get update -qq
  $SUDO apt-get install -y -qq --no-install-recommends "${PACKAGES[@]}" >/dev/null
  log_info "System packages installed"
  exit 0
fi

if [ "$(id -u)" -eq 0 ]; then
  log_error "Build as your WSL user, not root: wsl bash scripts/build/linux-wsl.sh"
  exit 1
fi

missing=()
for pkg in "${PACKAGES[@]}"; do
  grep -q 'ok installed' <<<"$(dpkg-query -W -f='${Status}' "$pkg" 2>/dev/null)" || missing+=("$pkg")
done
if [ ${#missing[@]} -gt 0 ]; then
  log_error "Missing packages: ${missing[*]}"
  log_error "Install them first: wsl -u root bash scripts/build/linux-wsl.sh --deps"
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/cargo" ]; then
  log_info "Installing Rust into ~/.cargo"
  fetch https://sh.rustup.rs /tmp/wukong-rustup-init.sh
  sh /tmp/wukong-rustup-init.sh -y --profile minimal
  rm -f /tmp/wukong-rustup-init.sh
fi
export PATH="$HOME/.cargo/bin:$PATH"

if [ ! -x "$NODE_DIR/bin/node" ]; then
  log_info "Installing Node ${NODE_VERSION} into ${NODE_DIR}"
  fetch "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" /tmp/wukong-node.tar.xz
  mkdir -p "$NODE_DIR"
  tar -xJ -C "$NODE_DIR" --strip-components=1 -f /tmp/wukong-node.tar.xz
  rm -f /tmp/wukong-node.tar.xz
fi
export PATH="$NODE_DIR/bin:$PATH"

if [ "$CLEAN" = true ]; then
  log_info "Dropping ${BUILD_ROOT}"
  rm -rf "$BUILD_ROOT"
fi

# Keep node_modules and target between runs, copy the sources fresh.
mkdir -p "$BUILD_ROOT"
find "$BUILD_ROOT" -mindepth 1 -maxdepth 1 ! -name node_modules ! -name src-tauri -exec rm -rf {} +
find "$BUILD_ROOT/src-tauri" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf {} + 2>/dev/null || true
tar -C "$REPO_ROOT" \
  --exclude=./node_modules --exclude=./dist --exclude=./releases --exclude=./.git \
  --exclude=./.autopilot --exclude=./src-tauri/target --exclude="./crates/*/target" \
  -cf - . | tar -C "$BUILD_ROOT" -xf -

cd "$BUILD_ROOT"
# A Windows checkout may carry CRLF scripts, which bash refuses.
find scripts src-tauri/device -name '*.sh' -exec sed -i 's/\r$//' {} +

npm ci
# The tauri CLI fetches its AppImage tools itself and fails on WSL's resolver where curl does not;
# it skips what is already in its cache, so they are fetched here first.
TOOLS="$HOME/.cache/tauri"
mkdir -p "$TOOLS"
while IFS=' ' read -r name url; do
  [ -s "$TOOLS/$name" ] && continue
  fetch "$url" "$TOOLS/$name"
  chmod 755 "$TOOLS/$name"
done <<TOOLS_LIST
AppRun-x86_64 https://github.com/tauri-apps/binary-releases/releases/download/apprun-old/AppRun-x86_64
linuxdeploy-x86_64.AppImage https://github.com/tauri-apps/binary-releases/releases/download/linuxdeploy/linuxdeploy-x86_64.AppImage
linuxdeploy-plugin-gtk.sh https://raw.githubusercontent.com/tauri-apps/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh
linuxdeploy-plugin-gstreamer.sh https://raw.githubusercontent.com/tauri-apps/linuxdeploy-plugin-gstreamer/master/linuxdeploy-plugin-gstreamer.sh
linuxdeploy-plugin-appimage.AppImage https://github.com/linuxdeploy/linuxdeploy-plugin-appimage/releases/download/continuous/linuxdeploy-plugin-appimage-x86_64.AppImage
TOOLS_LIST

# WSL usually has no FUSE: let linuxdeploy extract itself instead of mounting.
APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1 npx tauri build --bundles deb,appimage

# Extraction leaves 0700 directories, which the current appimagetool refuses to pack.
cp scripts/build/appimage-unbundle-wayland.sh /tmp/wukong-appimage-unbundle-wayland.sh
sed -i "/--appimage-extract-and-run so appimagetool/i chmod -R a+rX \"\$EXTRACT_DIR/squashfs-root\"" \
  /tmp/wukong-appimage-unbundle-wayland.sh
grep -q "chmod -R a+rX" /tmp/wukong-appimage-unbundle-wayland.sh
bash /tmp/wukong-appimage-unbundle-wayland.sh

out="${REPO_ROOT}/${OUTPUT_SUBDIR}"
mkdir -p "$out"
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
(cd "$check_dir" && "$app" --appimage-extract >/dev/null)
bundled="$(find "$check_dir/squashfs-root" -name "libwayland-*.so*")"
rm -rf "$check_dir"
if [ -n "$bundled" ]; then
  log_error "AppImage still bundles libwayland: $bundled"
  exit 1
fi

ls -l "$out"
log_info "Done"
