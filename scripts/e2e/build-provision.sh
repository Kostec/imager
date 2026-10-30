#!/usr/bin/env bash
# Runs inside ubuntu:22.04: builds examples/provision.rs and leaves the binary in /work.
# The example shares the crate's dependencies, so this needs the same packages as the app.
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends \
  build-essential ca-certificates curl file git patchelf pkg-config xz-utils \
  libayatana-appindicator3-dev libgtk-3-dev librsvg2-dev libssl-dev libudev-dev \
  libusb-1.0-0-dev libwebkit2gtk-4.1-dev libxdo-dev xdg-utils >/dev/null

if ! ls "$RUSTUP_HOME"/toolchains/*/bin/cargo >/dev/null 2>&1; then
  curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
export PATH="/root/.cargo/bin:$PATH"

# target is a volume mounted inside /build: keep it, copy everything else fresh.
tar -C /source \
  --exclude=./node_modules --exclude=./dist --exclude=./releases --exclude=./.git \
  --exclude=./.autopilot --exclude=./src-tauri/target --exclude="./crates/*/target" \
  -cf - . | tar -C /build -xf -

mkdir -p /build/dist
cd /build/src-tauri
cargo build --example provision
cp -f target/debug/examples/provision /work/provision
