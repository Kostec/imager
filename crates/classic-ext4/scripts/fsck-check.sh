#!/usr/bin/env bash
# Runs `e2fsck -fn` from Debian bullseye, the e2fsprogs of the image itself, on
# the rootfs of the given images. Without arguments it first runs the crate's
# tests, which leave one image per scenario, and checks those.
#
#   scripts/fsck-check.sh                     all test scenarios
#   WUKONG_TEST_IMAGE=/path/to.img scripts/fsck-check.sh     the real image too
#   scripts/fsck-check.sh a.img b.img         given images, read-only
#
# Needs a Linux Docker engine; the default context may be a Windows one.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
docker_cmd=(docker)
if [ "$(docker info --format '{{.OSType}}' 2>/dev/null)" != "linux" ]; then
    docker_cmd=(docker --context desktop-linux)
fi
to_host() {
    if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s' "$1"; fi
}

keep_dir=""
cleanup() { [ -z "$keep_dir" ] || rm -rf "$keep_dir"; }
trap cleanup EXIT

if [ "$#" -eq 0 ]; then
    keep_dir="$(mktemp -d)"
    CLASSIC_EXT4_KEEP_DIR="$(to_host "$keep_dir")" \
        cargo test --quiet --manifest-path "$here/../Cargo.toml" --test create --test real_image
    set -- "$keep_dir"/*.img
fi

status=0
for image in "$@"; do
    dir="$(cd "$(dirname "$image")" && pwd)"
    name="$(basename "$image")"
    echo "== $name"
    MSYS_NO_PATHCONV=1 "${docker_cmd[@]}" run --rm -i -v "$(to_host "$dir"):/w:ro" \
        debian:bullseye-slim bash -s "$name" <<'INNER' || status=1
set -uo pipefail
image="/w/$1"
# The first MBR entry of type 0x83, as ClassicExt4 picks it.
offset=""
for i in 0 1 2 3; do
    entry=$((446 + i * 16))
    type=$(od -An -tu1 -j $((entry + 4)) -N1 "$image" | tr -d ' ')
    if [ "$type" = 131 ]; then
        lba=$(od -An -tu4 -j $((entry + 8)) -N4 "$image" | tr -d ' ')
        offset=$((lba * 512))
        break
    fi
done
[ -n "$offset" ] || { echo 'no 0x83 partition'; exit 1; }
target="$image?offset=$offset"
e2fsck -fn "$target"
rc=$?
debugfs -R 'ls -l /etc/systemd/system/armbian-firstrun.service.d' "$target" 2>/dev/null |
    grep -v '^$' || true
exit "$rc"
INNER
done
exit "$status"
