#!/usr/bin/env bash
# Builds tests/fixtures/classic.img.gz with the mkfs options of the Wukong image
# (lib/functions/image/partitioning.sh: -m 2 -O ^64bit,^metadata_csum), plus a
# small blocks-per-group so that the 64 MiB fixture still has several groups.
# Needs a Linux Docker engine; the default context may be a Windows one.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fixtures="$here/../tests/fixtures"
docker_cmd=(docker)
if [ "$(docker info --format '{{.OSType}}' 2>/dev/null)" != "linux" ]; then
    docker_cmd=(docker --context desktop-linux)
fi
host_path="$fixtures"
command -v cygpath >/dev/null 2>&1 && host_path="$(cygpath -m "$fixtures")"

MSYS_NO_PATHCONV=1 "${docker_cmd[@]}" run --rm -i -v "$host_path:/out" debian:bullseye-slim bash -s <<'INNER'
set -euo pipefail
tree=/tmp/tree
mkdir -p "$tree"/etc/systemd/system/multi-user.target.wants \
         "$tree"/usr/lib/systemd/system "$tree"/htree "$tree"/data
echo 'hello from the fixture' > "$tree"/hello.txt
printf '[Unit]\nDescription=fixture firstrun\n' \
    > "$tree"/usr/lib/systemd/system/armbian-firstrun.service
ln -s usr/lib "$tree"/lib
ln -s /lib/systemd/system/armbian-firstrun.service \
    "$tree"/etc/systemd/system/multi-user.target.wants/armbian-firstrun.service
for i in $(seq 1 400); do
    printf 'file %s\n' "$i" > "$tree/htree/a-rather-long-file-name-to-fill-blocks-$i"
done
head -c 300000 /dev/zero | tr '\0' 'x' > "$tree"/data/blob.bin
# dir_index only applies to a directory after e2fsck -D rebuilds it.
export E2FSPROGS_FAKE_TIME=1700000000
mkfs.ext4 -q -b 4096 -I 256 -m 2 -g 8192 -O ^64bit,^metadata_csum \
    -E lazy_itable_init=0,lazy_journal_init=0 -J size=4 -L classic \
    -U 11111111-2222-3333-4444-555555555555 -d "$tree" /tmp/part.img 64M
e2fsck -fyD /tmp/part.img >/dev/null || [ $? -le 1 ]
e2fsck -fn /tmp/part.img
head -c 4194304 /dev/zero > /tmp/classic.img
# One MBR entry: type 0x83, LBA 8192, 131072 sectors.
printf '\x00\xfe\xff\xff\x83\xfe\xff\xff\x00\x20\x00\x00\x00\x00\x02\x00' |
    dd of=/tmp/classic.img bs=1 seek=446 conv=notrunc status=none
printf '\x55\xaa' | dd of=/tmp/classic.img bs=1 seek=510 conv=notrunc status=none
cat /tmp/part.img >> /tmp/classic.img
gzip -9n < /tmp/classic.img > /out/classic.img.gz
dumpe2fs -h /tmp/part.img 2>/dev/null | grep -E 'features|Block count|Inode size|group'
debugfs -R 'stat /htree' /tmp/part.img 2>/dev/null | head -1
INNER
