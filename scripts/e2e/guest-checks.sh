#!/bin/bash
# Runs on the guest as root, through the sudo of the new user, once first boot has finished.
# Prints one PASS or FAIL line per check. Usage: guest-checks.sh <expected.env>
#
# shellcheck source=/dev/null
. "$1"

WK_DIR=/etc/systemd/system/armbian-firstrun.service.d
WK_LOG=/var/log/wukong-firstboot.log
PROFILE=/etc/NetworkManager/system-connections/wukong-wifi.nmconnection

check() {
	local name=$1
	shift
	if "$@" >/dev/null 2>&1; then
		printf 'PASS %s\n' "$name"
	else
		printf 'FAIL %s\n' "$name"
	fi
}

absent() { [[ ! -e $1 ]]; }
has_line() { grep -qxF "$2" "$1"; }

same_hostname() { [[ $(hostname) == "$E2E_HOSTNAME" && $(cat /etc/hostname) == "$E2E_HOSTNAME" ]]; }
hosts_has_name() { grep -qw "$E2E_HOSTNAME" /etc/hosts && ! grep -qw wukongpi /etc/hosts; }
sudo_of_user() { [[ ${SUDO_USER:-} == "$E2E_USER" ]]; }
root_locked() { [[ $(passwd -S root | cut -d' ' -f2) == L ]]; }

user_groups() {
	local wanted
	for wanted in sudo netdev audio video disk tty users games dialout plugdev input systemd-journal ssh; do
		[[ " $(id -nG "$E2E_USER") " == *" $wanted "* ]] || return 1
	done
}

timezone_set() { [[ $(readlink -f /etc/localtime) == "/usr/share/zoneinfo/$E2E_TIMEZONE" ]]; }
locale_set() { locale -a | grep -qi "^${E2E_LOCALE%.*}\.utf-\?8$" && grep -q "^LANG=$E2E_LOCALE$" /etc/default/locale; }
keyboard_set() { grep -q "^XKBLAYOUT=\"$E2E_KEYBOARD\"$" /etc/default/keyboard; }
wifi_country_set() { grep -q "^REGDOMAIN=$E2E_WIFI_COUNTRY$" /etc/default/crda; }

overlays_set() {
	local line
	line=$(sed -n 's/^overlays=//p' /boot/armbianEnv.txt)
	[[ " $line " == *" usbhost2 "* && " $line " == *" usbhost3 "* && " $line " == *" i2c0 "* && " $line " == *" spi-spidev "* ]] &&
		grep -q '^param_spidev_spi_bus=0$' /boot/armbianEnv.txt
}

wifi_profile_private() { [[ $(stat -c '%a %U' "$PROFILE") == '600 root' ]]; }
wifi_psk_is_derived() { grep -qx "psk=$E2E_WIFI_PSK" "$PROFILE" && [[ ${#E2E_WIFI_PSK} == 64 ]]; }
ethernet_up() { [[ -n $(ip route show default) ]] && nmcli -t -f TYPE connection show --active | grep -q ethernet; }

sshd_value() { [[ $(/usr/sbin/sshd -T | sed -n "s/^$1 //p") == "$2" ]]; }

log_has_no_secret() {
	[[ -s $WK_LOG ]] || return 1
	! grep -qF -e "$E2E_PASSWORD" -e "$E2E_USER_HASH" -e "$E2E_WIFI_PSK" -e "$E2E_KEY_BODY" -e "$E2E_WIFI_SSID" "$WK_LOG"
}
log_has_summary() { grep -q 'firstboot: finished' "$WK_LOG" && grep -q 'post-network: finished' "$WK_LOG"; }
log_has_mdns() { grep -q 'firstboot: OK mdns' "$WK_LOG"; }
# name.local comes from systemd-resolved; the host cannot ask over QEMU's user network, so the board side is checked.
mdns_on_cable() {
	local dev
	dev=$(ip route show default | awk '{ print $5; exit }')
	[[ -n $dev ]] && resolvectl mdns "$dev" | grep -q ': yes$'
}
mdns_listens() { ss -lun | grep -q ':5353 '; }
dns_unchanged() { [[ ! -L /etc/resolv.conf ]] && ! grep -q '^nameserver 127.0.0.53' /etc/resolv.conf; }

# QEMU has no Wi-Fi adapter, so the line has to say that, and not just that something failed.
log_names_wifi_failure() {
	local line
	line=$(grep -m1 'post-network: FAIL wifi: ' "$WK_LOG") || return 1
	! nmcli -t -f TYPE device | grep -qx wifi && [[ $line == *'no Wi-Fi device in NetworkManager, connection inactive' ]]
}

# The monotonic time of the first journal line of a unit that matches a pattern.
first_seen() {
	journalctl -b --no-pager -o short-monotonic -u "$1" | grep -m1 -E "$2" | sed -E 's/^\[ *([0-9.]+)\].*/\1/'
}

# armbian-firstrun counts as started only when ExecStartPre has finished, and that is firstboot.sh.
started_after_firstboot() {
	local done_at at
	done_at=$(first_seen armbian-firstrun.service 'Started ')
	at=$(first_seen "$1" "$2")
	[[ -n $done_at && -n $at ]] && awk -v a="$done_at" -v b="$at" 'BEGIN { exit !(a < b) }'
}

one_boot() { [[ $(journalctl --list-boots --no-pager | wc -l) == 1 ]]; }
host_keys_exist() { compgen -G '/etc/ssh/ssh_host_*_key' >/dev/null; }

check 'hostname: name is set' same_hostname
check 'hostname: /etc/hosts follows' hosts_has_name
check 'user: sudo works for the new user' sudo_of_user
check 'user: the groups of the wizard' user_groups
check 'root: password locked' root_locked
check 'wizard: marker removed' absent /root/.not_logged_in_yet
check 'wizard: serial autologin removed' absent /etc/systemd/system/serial-getty@.service.d/override.conf
check 'wizard: console autologin removed' absent /etc/systemd/system/getty@.service.d/override.conf
check 'wipe: firstboot.env removed' absent "$WK_DIR/firstboot.env"
check 'wipe: drop-in directory removed' absent "$WK_DIR"
check 'wipe: post-network unit removed' absent /etc/systemd/system/wukong-post-network.service
check 'locale: time zone' timezone_set
check 'locale: language generated and set' locale_set
check 'locale: keyboard' keyboard_set
check 'wifi: regulatory domain' wifi_country_set
check 'wifi: profile is private' wifi_profile_private
check 'wifi: only the derived key is stored' wifi_psk_is_derived
check 'wifi: a network that fails leaves Ethernet up' ethernet_up
check 'overlays: added, usbhost kept, spidev bus set' overlays_set
check 'ssh: password login as configured' sshd_value passwordauthentication yes
check 'ssh: root login as configured' sshd_value permitrootlogin no
check 'ssh: host keys were generated' host_keys_exist
check 'script: the own script ran' test -f /var/tmp/wukong-e2e-user-script
check 'order: firstboot.sh before NetworkManager' started_after_firstboot NetworkManager.service 'Starting '
check 'order: firstboot.sh before ssh' started_after_firstboot ssh.service 'Starting '
check 'order: firstboot.sh before the login prompt' started_after_firstboot serial-getty@ttyS0.service 'Started '
printf 'INFO order: firstboot done %s, NetworkManager %s, ssh %s, login prompt %s
' 	"$(first_seen armbian-firstrun.service 'Started ')" "$(first_seen NetworkManager.service 'Starting ')" 	"$(first_seen ssh.service 'Starting ')" "$(first_seen serial-getty@ttyS0.service 'Started ')"
check 'boot: no extra reboot' one_boot
check 'log: has the summary' log_has_summary
check 'log: no password, hash, key, SSID' log_has_no_secret
check 'log: the cause of the Wi-Fi failure' log_names_wifi_failure
check 'mdns: on, says the log' log_has_mdns
check 'mdns: systemd-resolved runs' systemctl is-active --quiet systemd-resolved
check 'mdns: on for the cable link' mdns_on_cable
check 'mdns: listens on 5353' mdns_listens
check 'mdns: DNS stays with NetworkManager' dns_unchanged
printf 'INFO mdns: own name over mDNS: %s\n' "$(resolvectl query -p mdns "$E2E_HOSTNAME.local" 2>&1 | head -1)"

# Without armbian-ramlog the log is on the card anyway, and the power cut below proves nothing about the copy.
if [[ $(findmnt -no TARGET /var/log) == /var/log ]]; then
	printf 'INFO log: /var/log is in RAM
'
else
	printf 'WARN log: armbian-ramlog is not active, /var/log is on the card
'
fi

printf 'INFO log follows\n'
sed 's/^/INFO | /' "$WK_LOG"
