#!/usr/bin/env bash
# The e2e run itself, inside the tools image: see run.sh. /input is read-only, /work is the scratch space.
# Every check prints one line, and report.txt keeps them. Secrets are test values made up here.
#
# shellcheck disable=SC2015 # pass and fail always return 0
# shellcheck disable=SC2029 # the commands are meant for the guest, expanded here
set -uo pipefail

SOURCE_IMAGE=$1
HERE=/source/scripts/e2e
# The container's own disk: a bind mount from Windows is too slow to boot from, and this one goes with the container.
CARD=/var/tmp/card.img
REPORT=/work/report.txt
ROOTFS=/var/tmp/rootfs.part
BOOT_DIR=/var/tmp/boot
SERIAL_SOCK=/tmp/wukong-serial.sock
SERIAL_LOG=/work/serial.log
QEMU_LOG=/work/qemu.log
SSH_PORT=2222
SSH_KEY=/work/e2e_key
CARD_SIZE=4G

E2E_HOSTNAME=wukong-e2e
E2E_USER=tester
E2E_TIMEZONE=Europe/Moscow
E2E_LOCALE=ru_RU.UTF-8
E2E_KEYBOARD=ru
E2E_WIFI_COUNTRY=RU
E2E_WIFI_SSID=wukong-e2e-absent-network
SSH_OPTIONS=(-q -i "$SSH_KEY" -p "$SSH_PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
  -o ConnectTimeout=20 -o BatchMode=yes)

failures=0
boot_verified=0
qemu_pid=

report() { printf '%s\n' "$*" | tee -a "$REPORT"; }
pass() { report "PASS $*"; }
fail() { report "FAIL $*"; failures=$((failures + 1)); }

finish() {
  local status=$1
  [[ -n $qemu_pid ]] && kill -9 "$qemu_pid" 2>/dev/null
  rm -f "$ROOTFS" "$SSH_KEY" "$SSH_KEY.pub" /work/settings.json /work/expected.env
  if [[ ${E2E_KEEP:-0} == 1 ]]; then
    cp --sparse=always "$CARD" /work/card.img
  fi
  exit "$status"
}

# ---- image ------------------------------------------------------------------

partition_offset() {
  fdisk -l -o Start,Type "$1" | awk '$2 == "Linux" { print $1 * 512; exit }'
}

on_rootfs() {
  debugfs -R "$1" "$CARD?offset=$offset" 2>/dev/null
}

file_mode() {
  on_rootfs "stat $1" | sed -n 's/.*Mode: *\([0-7]*\).*/\1/p'
}

make_settings() {
  E2E_PASSWORD=$(openssl rand -hex 12)
  E2E_USER_HASH=$(printf '%s' "$E2E_PASSWORD" | openssl passwd -6 -stdin)
  E2E_WIFI_PASSPHRASE=$(openssl rand -hex 12)
  E2E_WIFI_PSK=$(python3 -I -c 'import hashlib, sys
print(hashlib.pbkdf2_hmac("sha1", sys.argv[1].encode(), sys.argv[2].encode(), 4096, 32).hex())' \
    "$E2E_WIFI_PASSPHRASE" "$E2E_WIFI_SSID")
  ssh-keygen -q -t ed25519 -N '' -C wukong-e2e -f "$SSH_KEY"
  E2E_KEY_BODY=$(cut -d' ' -f2 "$SSH_KEY.pub")
  export E2E_PASSWORD

  python3 -I - <<PY
import json
json.dump({
    "hostname": "$E2E_HOSTNAME", "timezone": "$E2E_TIMEZONE", "locale": "$E2E_LOCALE",
    "keyboardLayout": "$E2E_KEYBOARD",
    "userName": "$E2E_USER", "userRealName": "E2E Tester", "userShell": "bash",
    "userPasswordHash": """$E2E_USER_HASH""",
    "wifiEnabled": True, "wifiSsid": "$E2E_WIFI_SSID", "wifiPsk": "$E2E_WIFI_PSK",
    "wifiCountryCode": "$E2E_WIFI_COUNTRY",
    "sshEnabled": True, "sshPasswordAuth": True, "sshRootLogin": False,
    "sshAuthorizedKeys": open("$SSH_KEY.pub").read(),
    "overlays": ["i2c0", "spi-spidev"],
    "userScript": "#!/bin/bash\necho wukong-e2e-user-script\ntouch /var/tmp/wukong-e2e-user-script\n",
}, open("/work/settings.json", "w"))
PY
  {
    declare -p E2E_HOSTNAME E2E_USER E2E_TIMEZONE E2E_LOCALE E2E_KEYBOARD E2E_WIFI_COUNTRY
    declare -p E2E_PASSWORD E2E_USER_HASH E2E_WIFI_PSK E2E_KEY_BODY
  } | sed 's/^declare -[-x]* //' >/work/expected.env
}

check_image() {
  local before after support dir=/etc/systemd/system/armbian-firstrun.service.d
  before=$(sha256sum "$SOURCE_IMAGE" | cut -d' ' -f1)
  cp --sparse=always "$SOURCE_IMAGE" "$CARD" || { fail "image: copy"; finish 1; }
  offset=$(partition_offset "$CARD")
  [[ -n $offset ]] || { fail "image: no Linux partition"; finish 1; }

  support=$(/work/provision probe "$CARD")
  [[ $support == Supported ]] && pass "image: probe says Supported" || fail "image: probe says $support"

  make_settings
  if /work/provision apply "$CARD" /work/settings.json; then
    pass "image: settings applied"
  else
    fail "image: settings not applied"
    finish 1
  fi

  dd if="$CARD" of="$ROOTFS" bs=1M iflag=skip_bytes skip="$offset" status=none
  if e2fsck -fn "$ROOTFS" >/work/e2fsck.log 2>&1; then
    pass "image: e2fsck -fn is clean"
  else
    fail "image: e2fsck -fn, see e2fsck.log"
  fi
  rm -f "$ROOTFS"

  expect_mode "$dir/10-wukong.conf" 0644
  expect_mode "$dir/firstboot.sh" 0755
  expect_mode "$dir/post-network.sh" 0755
  expect_mode "$dir/firstboot.env" 0600
  expect_mode "$dir/user-script.sh" 0700

  if grep -qaF -e "$E2E_PASSWORD" -e "$E2E_WIFI_PASSPHRASE" "$CARD"; then
    fail "image: a typed password is on the card"
  else
    pass "image: no typed password on the card"
  fi

  after=$(sha256sum "$SOURCE_IMAGE" | cut -d' ' -f1)
  [[ $before == "$after" ]] && pass "image: source unchanged" || fail "image: source changed"
}

expect_mode() {
  local mode
  mode=$(file_mode "$1")
  [[ $mode == "$2" ]] && pass "image: $1 is $2" || fail "image: $1 is '$mode', not $2"
}

# ---- boot -------------------------------------------------------------------

extract_boot() {
  mkdir -p "$BOOT_DIR"
  on_rootfs "dump /boot/vmlinuz-5.15.93-sunxi $BOOT_DIR/zImage" >/dev/null
  on_rootfs "dump /boot/initrd.img-5.15.93-sunxi $BOOT_DIR/initrd" >/dev/null
  on_rootfs "dump /boot/dtb-5.15.93-sunxi/$E2E_DTB $BOOT_DIR/board.dtb" >/dev/null
  on_rootfs "cat /boot/armbianEnv.txt" | sed -n 's/^rootdev=//p' | head -n 1 >"$BOOT_DIR/rootdev"
  [[ -s $BOOT_DIR/zImage && -s $BOOT_DIR/initrd && -s $BOOT_DIR/board.dtb && -s $BOOT_DIR/rootdev ]]
}

start_qemu() {
  # Sparse, and QEMU refuses an SD card whose size is not a power of two.
  truncate -s "$CARD_SIZE" "$CARD"
  qemu-system-arm -M orangepi-pc -smp 4 -m 1024 -nographic -monitor none \
    -kernel "$BOOT_DIR/zImage" -initrd "$BOOT_DIR/initrd" -dtb "$BOOT_DIR/board.dtb" \
    -append "root=$(cat "$BOOT_DIR/rootdev") rootwait rootfstype=ext4 console=ttyS0,115200 consoleblank=0 loglevel=4 cgroup_enable=memory swapaccount=1 panic=30" \
    -drive "if=sd,format=raw,file=$CARD,file.locking=off" \
    -nic "user,hostfwd=tcp:127.0.0.1:$SSH_PORT-:22" \
    -serial "unix:$SERIAL_SOCK,server=on,wait=off" \
    >"$QEMU_LOG" 2>&1 &
  qemu_pid=$!
}

guest() {
  ssh "${SSH_OPTIONS[@]}" "$E2E_USER@127.0.0.1" "$@"
}

guest_sudo() {
  printf '%s\n' "$E2E_PASSWORD" | guest "sudo -S -p '' $*"
}

wait_for_ssh() {
  local deadline=$((SECONDS + 600))
  while ((SECONDS < deadline)); do
    guest true </dev/null && return 0
    sleep 5
  done
  return 1
}

wait_for_post_network() {
  local deadline=$((SECONDS + E2E_SETTLE_TIMEOUT))
  while ((SECONDS < deadline)); do
    guest_sudo grep -q "'post-network: finished'" /var/log/wukong-firstboot.log && return 0
    sleep 10
  done
  return 1
}

check_guest() {
  local line
  scp "${SSH_OPTIONS[@]/-p/-P}" "$HERE/guest-checks.sh" /work/expected.env "$E2E_USER@127.0.0.1:/tmp/"
  while IFS= read -r line; do
    case "$line" in
      PASS*) report "$line" ;;
      FAIL*) report "$line"; failures=$((failures + 1)) ;;
      *) report "$line" ;;
    esac
  done < <(guest_sudo bash /tmp/guest-checks.sh /tmp/expected.env)
  guest_sudo rm -f /tmp/guest-checks.sh /tmp/expected.env
}

check_logins() {
  local out
  out=$(SSHPASS=$E2E_PASSWORD sshpass -e ssh -p "$SSH_PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    -o PreferredAuthentications=password -o PubkeyAuthentication=no "$E2E_USER@127.0.0.1" id -un 2>/dev/null)
  [[ $out == "$E2E_USER" ]] && pass "ssh: password login" || fail "ssh: password login"
  if ssh "${SSH_OPTIONS[@]}" root@127.0.0.1 true </dev/null; then
    fail "ssh: root may log in"
  else
    pass "ssh: root is refused"
  fi
}

# After a power off the disk itself must show that nothing of the bundle survived.
check_wiped_card() {
  guest_sudo poweroff >/dev/null 2>&1
  local waited=0
  while kill -0 "$qemu_pid" 2>/dev/null && ((waited < 120)); do
    if grep -qE 'reboot: (Power down|System halted)' "$SERIAL_LOG" 2>/dev/null; then
      break
    fi
    sleep 2
    waited=$((waited + 2))
  done
  kill -9 "$qemu_pid" 2>/dev/null
  wait "$qemu_pid" 2>/dev/null
  qemu_pid=
  if grep -qaF -e 'WK_USER_HASH=' -e "$E2E_WIFI_PASSPHRASE" -e "$E2E_PASSWORD" "$CARD"; then
    fail "wipe: traces of firstboot.env on the card"
  else
    pass "wipe: no trace of firstboot.env on the card"
  fi
}

boot_and_check() {
  local status
  if ! extract_boot; then
    report "UNVERIFIED boot: kernel, initrd or dtb not found in the image"
    return
  fi
  start_qemu
  python3 -I "$HERE/console.py" "$SERIAL_SOCK" "$SERIAL_LOG" "$E2E_USER" "$E2E_HOSTNAME" "$E2E_BOOT_TIMEOUT" \
    | tee -a "$REPORT"
  status=${PIPESTATUS[0]}
  if ((status == 3)); then
    report "UNVERIFIED boot: last lines of the console follow"
    tail -n 30 "$SERIAL_LOG" | sed 's/^/INFO | /' | tee -a "$REPORT"
    return
  fi
  boot_verified=1
  ((status == 0)) || failures=$((failures + 1))

  if ! wait_for_ssh; then
    fail "ssh: key login"
    return
  fi
  pass "ssh: key login"
  wait_for_post_network && pass "first boot: stage after the network finished" \
    || fail "first boot: stage after the network did not finish"
  check_logins
  check_guest
  check_wiped_card
}

: >"$REPORT"
check_image
if [[ ${E2E_OFFLINE:-0} == 1 ]]; then
  report "SKIP boot: --offline"
else
  boot_and_check
fi

report "SUMMARY failures=$failures boot_verified=$boot_verified"
if ((failures > 0)); then
  finish 1
fi
if [[ ${E2E_OFFLINE:-0} != 1 ]] && ((boot_verified == 0)); then
  finish 3
fi
finish 0
