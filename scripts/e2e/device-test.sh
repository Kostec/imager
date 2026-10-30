#!/usr/bin/env bash
# The device scripts on stubs: what QEMU cannot show (a working Wi-Fi, a wipe that fails) in seconds.
# Runs as root in the throwaway tools container, because the scripts have fixed paths. Prints PASS and FAIL lines.
# Usage: device-test.sh <directory with firstboot.sh and post-network.sh>
#
# shellcheck disable=SC2016 # the stubs are written out literally, and expand when they run
set -uo pipefail

DEVICE=$1
WK_DIR=/etc/systemd/system/armbian-firstrun.service.d
WK_LOG=/var/log/wukong-firstboot.log
WK_HDD=/var/log.hdd
WK_UNIT=/etc/systemd/system/wukong-post-network.service
RAMLOG_DROPIN=/etc/systemd/system/armbian-ramlog.service.d/10-wukong.conf
STUB=/tmp/device-test-stubs
CALLS=/tmp/device-test-calls
SSID=e2e-network-name
PSK=e2e-psk-value
HASH=e2e-hash-value

failures=0

pass() { printf 'PASS device: %s\n' "$*"; }
fail() { printf 'FAIL device: %s\n' "$*"; failures=$((failures + 1)); }
expect() {
  local name=$1
  shift
  if "$@" >/dev/null 2>&1; then pass "$name"; else fail "$name"; fi
}
refuse() {
  local name=$1
  shift
  if "$@" >/dev/null 2>&1; then fail "$name"; else pass "$name"; fi
}

stub() {
  printf '#!/bin/bash\n%s\n' "$2" >"$STUB/$1"
  chmod 755 "$STUB/$1"
}

write_stubs() {
  rm -rf "$STUB"
  mkdir -p "$STUB"
  stub sleep 'exit 0'
  stub chronyc 'exit 0'
  stub ip 'echo "default via 10.0.2.2 dev eth0"'
  stub systemctl 'echo "systemctl $*" >>'"$CALLS"
  stub dpkg 'echo "dpkg $*" >>'"$CALLS"
  stub dpkg-query '[[ -e /tmp/device-test-installed ]] && printf "install ok installed"'
  stub apt-get 'echo "apt-get $*" >>'"$CALLS"'
list=
for arg in "$@"; do [[ $arg == Dir::Etc::sourcelist=* ]] && list=${arg#*=}; done
[[ -n $list ]] && cat "$list" >>/tmp/device-test-apt-list
[[ " $* " == *" update "* && -e /tmp/device-test-update-fails ]] && exit 100
[[ " $* " == *" install "* && " $* " != *" --download-only "* ]] && touch /tmp/device-test-installed
exit 0'
  # NM_STATE: up, no-adapter or down.
  stub nmcli 'case "${NM_STATE:-up}:$*" in
  down:*) exit 8 ;;
  *"device show wlan0"*)
    echo "30 (disconnected)"
    echo "53 (The Wi-Fi network could not be found)" ;;
  no-adapter:*DEVICE,TYPE*) echo "eth0:ethernet" ;;
  *DEVICE,TYPE*) printf "eth0:ethernet\nwlan0:wifi\n" ;;
  active:*"connection show wukong-wifi"*) echo activated ;;
  *"connection show wukong-wifi"*) echo "" ;;
  *) echo running ;;
esac'
}

reset_card() {
  rm -rf "$WK_DIR" "$WK_UNIT" "$WK_LOG" "$WK_HDD" "${RAMLOG_DROPIN%/*}" /var/lib/wukong-*
  rm -f /tmp/device-test-installed /tmp/device-test-update-fails /tmp/device-test-apt-list /var/tmp/device-test-script-runs
  : >"$CALLS"
  mkdir -p "$WK_DIR" "$WK_HDD"
  cp "$DEVICE/firstboot.sh" "$DEVICE/post-network.sh" "$WK_DIR/"
  : >"$WK_UNIT"
  cat >"$WK_DIR/firstboot.env" <<ENV
WK_HOSTNAME='wukong-e2e'
WK_WIFI_ENABLED='1'
WK_WIFI_SSID='$SSID'
WK_WIFI_PSK='$PSK'
WK_USER_HASH='$HASH'
ENV
  cat >"$WK_DIR/user-script.sh" <<'SCRIPT'
#!/bin/bash
echo ran >>/var/tmp/device-test-script-runs
SCRIPT
}

post_network() {
  PATH="$STUB:$PATH" bash "$WK_DIR/post-network.sh"
}

in_log() { grep -qF -- "$1" "$WK_LOG"; }
script_runs() { wc -l <"/var/tmp/device-test-script-runs" 2>/dev/null || echo 0; }
called() { grep -qF -- "$1" "$CALLS"; }
no_secret_in_log() { ! grep -qF -e "$SSID" -e "$PSK" -e "$HASH" "$WK_LOG"; }
same_as_log() { cmp -s "$WK_LOG" "$WK_HDD/wukong-firstboot.log"; }
apt_list_is_trimmed() {
  [[ -s /tmp/device-test-apt-list ]] && grep -q ' bullseye main' /tmp/device-test-apt-list &&
    ! grep -qE 'bullseye-backports|security' /tmp/device-test-apt-list
}

SOURCES_SAVED=/tmp/device-test-sources
restore_sources() { cp "$SOURCES_SAVED" /etc/apt/sources.list; }

test_online() {
  reset_card
  NM_STATE=active post_network
  expect 'avahi is installed' in_log 'post-network: OK avahi-daemon installed'
  expect 'apt works without backports and security' apt_list_is_trimmed
  expect 'an interrupted dpkg is finished first' called 'dpkg --configure -a'
  expect 'Wi-Fi up gives OK wifi' grep -qE 'post-network: OK wifi$' "$WK_LOG"
  refuse 'Wi-Fi up gives no FAIL wifi' in_log 'FAIL wifi'
  expect 'the settings are wiped' in_log 'OK settings wiped'
  expect 'the unit is disabled' called 'systemctl disable wukong-post-network.service'
  refuse 'the directory is gone' test -e "$WK_DIR"
  refuse 'the unit file is gone' test -e "$WK_UNIT"
  expect 'the script ran once' test "$(script_runs)" = 1
  expect 'the stage finished' in_log 'post-network: finished'
  expect 'the log reached the disk' same_as_log
}

test_update_errors() {
  reset_card
  touch /tmp/device-test-update-fails
  NM_STATE=active post_network
  expect 'a list that fails to update does not stop the install' in_log 'post-network: OK avahi-daemon installed'
}

test_wifi_absent_network() {
  reset_card
  post_network
  expect 'FAIL wifi gives the state and the reason' \
    grep -qE 'post-network: FAIL wifi: .*30 \(disconnected\).*53 \(The Wi-Fi network could not be found\)' "$WK_LOG"
  expect 'FAIL wifi does not stop avahi' in_log 'post-network: OK avahi-daemon installed'
  expect 'FAIL wifi does not stop the wipe' in_log 'OK settings wiped'
  expect 'no value of firstboot.env in the log' no_secret_in_log
}

test_wifi_no_adapter() {
  reset_card
  NM_STATE=no-adapter post_network
  expect 'no adapter is named as such' grep -qE 'post-network: FAIL wifi: no Wi-Fi device' "$WK_LOG"
  reset_card
  NM_STATE=down post_network
  expect 'a stopped NetworkManager is named as such' grep -qE 'post-network: FAIL wifi: NetworkManager is not running' "$WK_LOG"
}

test_wipe_fails() {
  reset_card
  # The one file neither shred nor rm can take away, and the file that would bring the script back.
  stub shred '[[ " $* " == *" '"$WK_DIR"'/user-script.sh "* ]] && exit 1; exec /usr/bin/shred "$@"'
  stub rm '[[ " $* " == *" '"$WK_DIR"'/user-script.sh "* ]] && exit 1; exec /bin/rm "$@"'
  NM_STATE=active post_network
  expect 'a failed wipe is logged' in_log 'post-network: FAIL settings not wiped'
  refuse 'a failed wipe does not say OK' in_log 'OK settings wiped'
  refuse 'the unit stays enabled' called 'disable wukong-post-network.service'
  expect 'the unit file stays' test -e "$WK_UNIT"
  expect 'the stage stays, for the retry' test -f "$WK_DIR/post-network.sh"
  refuse 'the settings are gone all the same' test -e "$WK_DIR/firstboot.env"
  expect 'the script ran' test "$(script_runs)" = 1

  post_network
  expect 'the next boot does not repeat the script' test "$(script_runs)" = 1
  refuse 'the next boot does not repeat apt' test "$(grep -c 'apt-get.* update' "$CALLS")" -gt 1
  expect 'still not wiped, still logged' in_log 'post-network: FAIL settings not wiped'

  rm -f "$STUB/shred" "$STUB/rm"
  post_network
  expect 'with the obstacle gone the retry wipes' in_log 'OK settings wiped'
  refuse 'and the directory goes' test -e "$WK_DIR"
  refuse 'and the unit file goes' test -e "$WK_UNIT"
  expect 'and the unit is disabled' called 'disable wukong-post-network.service'
  expect 'the script still ran once' test "$(script_runs)" = 1
}

test_stage_fails() {
  reset_card
  stub install 'exit 1'
  NM_STATE=active post_network
  rm -f "$STUB/install"
  expect 'a user script that cannot be staged is logged' in_log 'post-network: FAIL user script not staged'
  expect 'and the rest goes on' in_log 'post-network: finished'
}

# When armbian-ramlog failed to start, both paths are the one file on the card.
test_log_already_on_disk() {
  reset_card
  rm -rf "$WK_HDD"
  ln -s /var/log "$WK_HDD"
  NM_STATE=active post_network
  refuse 'a log already on the card is no failure' in_log 'FAIL log not copied'
  rm -f "$WK_HDD"
}

test_firstboot_log() {
  reset_card
  : >"$WK_DIR/firstboot.env"
  PATH="$STUB:$PATH" bash "$WK_DIR/firstboot.sh"
  expect 'firstboot.sh finished' in_log 'firstboot: finished'
  expect 'its log reached the disk' same_as_log
  expect 'armbian-ramlog gets the time to bring the log back' grep -qx 'TimeoutStartSec=5min' "$RAMLOG_DROPIN"
  expect 'and systemd can read that' test "$(stat -c %a "$RAMLOG_DROPIN")" = 644
  expect 'even from a private umask' test "$(stat -c %a "${RAMLOG_DROPIN%/*}")" = 755
}

mkdir -p /etc/systemd/system /etc/apt /var/log
cp /etc/apt/sources.list "$SOURCES_SAVED" 2>/dev/null || : >"$SOURCES_SAVED"
cat >/etc/apt/sources.list <<'LIST'
deb http://deb.debian.org/debian bullseye main contrib non-free
deb http://deb.debian.org/debian bullseye-updates main contrib non-free
deb http://deb.debian.org/debian bullseye-backports main contrib non-free
deb http://security.debian.org/ bullseye-security main contrib non-free
LIST
write_stubs
test_online
test_update_errors
test_wifi_absent_network
test_wifi_no_adapter
test_wipe_fails
test_stage_fails
test_log_already_on_disk
test_firstboot_log
restore_sources
rm -rf "$WK_DIR" "$WK_UNIT" "$WK_LOG" "$WK_HDD" /tmp/device-test-* /var/tmp/device-test-*

printf 'INFO device: %s failed\n' "$failures"
((failures == 0))
