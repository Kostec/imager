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
  # Only to catch a fetch coming back: the stage must not need the internet.
  stub apt-get 'echo "apt-get $*" >>'"$CALLS"
  stub dpkg 'echo "dpkg $*" >>'"$CALLS"
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
  rm -f /var/tmp/device-test-script-runs
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
MDNS_RESOLVED=/etc/systemd/resolved.conf.d/10-wukong-mdns.conf
MDNS_NM=/etc/NetworkManager/conf.d/30-wukong-mdns.conf

test_online() {
  reset_card
  NM_STATE=active post_network
  refuse 'nothing is fetched from the internet' grep -q -e apt-get -e dpkg "$CALLS"
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

test_wifi_absent_network() {
  reset_card
  post_network
  expect 'FAIL wifi gives the state and the reason' \
    grep -qE 'post-network: FAIL wifi: .*30 \(disconnected\).*53 \(The Wi-Fi network could not be found\)' "$WK_LOG"
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

# name.local comes from systemd-resolved, offline, before NetworkManager starts.
test_firstboot_mdns() {
  reset_card
  rm -f "$MDNS_RESOLVED" "$MDNS_NM"
  printf "WK_HOSTNAME='wukong-e2e'
" >"$WK_DIR/firstboot.env"
  PATH="$STUB:$PATH" bash "$WK_DIR/firstboot.sh"
  expect 'a name turns mDNS on' in_log 'firstboot: OK mdns'
  expect 'systemd-resolved answers mDNS' grep -qx 'MulticastDNS=yes' "$MDNS_RESOLVED"
  expect 'NetworkManager gives it every link' grep -qx 'connection.mdns=2' "$MDNS_NM"
  expect 'systemd-resolved is enabled' called 'systemctl enable systemd-resolved.service'
  expect 'and started this boot' called 'systemctl --no-block start systemd-resolved.service'

  reset_card
  rm -f "$MDNS_RESOLVED" "$MDNS_NM"
  : >"$WK_DIR/firstboot.env"
  PATH="$STUB:$PATH" bash "$WK_DIR/firstboot.sh"
  refuse 'no name leaves mDNS alone' test -e "$MDNS_RESOLVED"
  refuse 'and systemd-resolved too' called 'systemd-resolved'
}

mkdir -p /etc/systemd/system /var/log
write_stubs
test_online
test_wifi_absent_network
test_wifi_no_adapter
test_wipe_fails
test_stage_fails
test_log_already_on_disk
test_firstboot_log
test_firstboot_mdns
rm -rf "$WK_DIR" "$WK_UNIT" "$WK_LOG" "$WK_HDD" "$MDNS_RESOLVED" "$MDNS_NM" /tmp/device-test-* /var/tmp/device-test-*

printf 'INFO device: %s failed\n' "$failures"
((failures == 0))
