#!/bin/bash
# Wukong Imager: first boot, the stage after the network.
#
# firstboot.sh starts this as wukong-post-network.service, and it stays enabled
# until it has finished, so a power cut repeats it on the next boot. It waits
# for a route, the clock, Wi-Fi and avahi-daemon, wipes everything the imager
# left on the card, itself included, and then runs the user's script once from
# RAM. If the wipe fails, the unit stays for a retry on the next boot.
#
# Never print a value from firstboot.env, and no `set -x`.

WK_DIR=/etc/systemd/system/armbian-firstrun.service.d
WK_ENV=$WK_DIR/firstboot.env
WK_LOG=/var/log/wukong-firstboot.log
WK_LOG_DISK=/var/log.hdd
WK_POST_UNIT=/etc/systemd/system/wukong-post-network.service
WIFI_PROFILE=wukong-wifi
ROUTE_WAIT_SECS=120
WIFI_WAIT_SECS=120
CLOCK_WAIT_SECS=60
APT_TIMEOUT_SECS=900
# Far past any real install: dpkg must finish, a stop halfway leaves the packages unconfigured.
DPKG_TIMEOUT_SECS=3600
APT_LOCK_WAIT_SECS=120
APT_LIST=/run/wukong-sources.list
USER_SCRIPT_TIMEOUT_SECS=300
USER_SCRIPT_KILL_SECS=10
USER_SCRIPT_RUN=/run/wukong-user-script.sh
USER_SCRIPT_RAN=/var/lib/wukong-user-script-ran

# apt and the user's script must leave files others can read, as under a login shell.
umask 022

log() {
	printf '%s post-network: %s\n' "$(date '+%F %T')" "$*" >>"$WK_LOG" 2>/dev/null
}

# /var/log is in RAM on Armbian. What is copied here comes back at the next boot, even after a power cut.
save_log() {
	# With armbian-ramlog down, /var/log.hdd is a bind mount of the card's own /var/log and this is the same file.
	if [[ -d $WK_LOG_DISK && ! $WK_LOG -ef $WK_LOG_DISK/${WK_LOG##*/} ]]; then
		cp -p "$WK_LOG" "$WK_LOG_DISK/" 2>/dev/null || log "FAIL log not copied to $WK_LOG_DISK"
	fi
	sync
}

# NetworkManager-wait-online is disabled on this image, so nothing else tells us.
wait_for_route() {
	local waited=0
	while ((waited < ROUTE_WAIT_SECS)); do
		if [[ -n $(ip route show default 2>/dev/null) ]]; then
			log "OK route after ${waited}s"
			return 0
		fi
		sleep 2
		waited=$((waited + 2))
	done
	log "FAIL no default route after ${ROUTE_WAIT_SECS}s"
	return 1
}

# No RTC on the board: apt refuses repository metadata from the "future" until the clock is set.
wait_for_clock() {
	if timeout "$CLOCK_WAIT_SECS" chronyc waitsync 12 0 0 5 >/dev/null 2>&1; then
		log "OK clock synchronized"
	else
		log "FAIL clock not synchronized after ${CLOCK_WAIT_SECS}s"
	fi
}

nm() {
	LC_ALL=C nmcli "$@" 2>/dev/null
}

# What NetworkManager says now, in its own words. Never the SSID: only the fixed profile name and the device.
wifi_failure() {
	local device state reason profile
	if ! nm -t general status >/dev/null; then
		printf 'NetworkManager is not running'
		return
	fi
	if profile=$(nm -g GENERAL.STATE connection show "$WIFI_PROFILE"); then
		profile=${profile:-inactive}
	else
		profile='not loaded'
	fi
	device=$(nm -t -f DEVICE,TYPE device | awk -F: '$2 == "wifi" { print $1; exit }')
	if [[ -z $device ]]; then
		printf 'no Wi-Fi device in NetworkManager, connection %s' "$profile"
		return
	fi
	{
		read -r state
		read -r reason
	} < <(nm -g GENERAL.STATE,GENERAL.REASON device show "$device")
	printf 'device %s %s, reason %s, connection %s' "$device" "$state" "$reason" "$profile"
}

# Runs beside the rest: a network that never comes must not hold up what the cable can do.
wait_for_wifi() {
	local waited=0
	while ((waited < WIFI_WAIT_SECS)); do
		if [[ $(nm -g GENERAL.STATE connection show "$WIFI_PROFILE") == activated ]]; then
			log "OK wifi"
			return 0
		fi
		sleep 2
		waited=$((waited + 2))
	done
	log "FAIL wifi: $(wifi_failure)"
}

# bullseye-backports is gone from the mirrors, and the armhf packages of bullseye-security answer 404.
write_apt_list() {
	grep -v -e bullseye-backports -e security /etc/apt/sources.list >"$APT_LIST"
}

# Fetching may be cut off at any moment. dpkg may not: --foreground keeps it out of the signal even at the limit,
# and the limit is there only so that a hung package cannot leave the settings on the card for good.
install_avahi() {
	local quiet=(env DEBIAN_FRONTEND=noninteractive)
	local apt=("${quiet[@]}" apt-get -qq -o "Dir::Etc::sourcelist=$APT_LIST" -o "DPkg::Lock::Timeout=$APT_LOCK_WAIT_SECS")
	local packages=(--no-install-recommends avahi-daemon)
	if dpkg-query -W -f='${Status}' avahi-daemon 2>/dev/null | grep -q 'ok installed'; then
		log "OK avahi-daemon already installed"
		return 0
	fi
	if ! write_apt_list; then
		log "FAIL avahi-daemon not installed, no apt list"
		return 1
	fi
	# After a power cut inside dpkg, apt-get refuses everything until this has run.
	timeout --foreground "$DPKG_TIMEOUT_SECS" "${quiet[@]}" dpkg --configure -a >>"$WK_LOG" 2>&1
	# An error here is about one list, and the package may well be in the others: install decides.
	timeout "$APT_TIMEOUT_SECS" "${apt[@]}" update >>"$WK_LOG" 2>&1 || log "apt-get update reported errors"
	if timeout "$APT_TIMEOUT_SECS" "${apt[@]}" install -y --download-only "${packages[@]}" >>"$WK_LOG" 2>&1 &&
		timeout --foreground "$DPKG_TIMEOUT_SECS" "${apt[@]}" install -y --no-download "${packages[@]}" >>"$WK_LOG" 2>&1; then
		log "OK avahi-daemon installed"
	else
		log "FAIL avahi-daemon not installed, name.local will not resolve"
	fi
}

# The script runs from RAM, after the wipe: a reboot or a kill inside it can neither leave settings on the card nor repeat it.
stage_user_script() {
	if [[ -f $WK_DIR/user-script.sh && ! -e $USER_SCRIPT_RAN ]]; then
		install -m 700 "$WK_DIR/user-script.sh" "$USER_SCRIPT_RUN" >>"$WK_LOG" 2>&1 || log "FAIL user script not staged"
	fi
}

run_user_script() {
	local status
	timeout --kill-after="$USER_SCRIPT_KILL_SECS" "$USER_SCRIPT_TIMEOUT_SECS" /bin/bash "$USER_SCRIPT_RUN" >>"$WK_LOG" 2>&1
	status=$?
	rm -f "$USER_SCRIPT_RUN"
	if ((status == 0)); then
		log "OK user script"
	elif ((status == 124 || status == 137)); then
		log "FAIL user script stopped after ${USER_SCRIPT_TIMEOUT_SECS}s"
	else
		log "FAIL user script (exit $status)"
	fi
}

wipe_file() {
	shred -u -n 1 "$1" || rm -f "$1"
	sync
}

# Settings first, then whatever a repeat does not need. This script goes last, and only when nothing else is left:
# the unit starts only while it exists, so a power cut or a file that will not go runs the rest on the next boot.
# Bash has parsed all of it by the time main starts.
wipe_everything() {
	local file self=$WK_DIR/post-network.sh
	[[ -e $WK_ENV ]] && wipe_file "$WK_ENV"
	for file in "$WK_DIR"/* "$WK_DIR"/.[!.]*; do
		if [[ -e $file && $file != "$self" ]]; then
			wipe_file "$file"
		fi
	done
	if [[ -n $(find "$WK_DIR" -mindepth 1 ! -path "$self" -print -quit 2>/dev/null) ]]; then
		log "FAIL settings not wiped"
		return 1
	fi
	# Renamed first: the unit must never start a half-overwritten file.
	if [[ -e $self ]]; then
		mv -f "$self" "$self.wiping"
		sync
	fi
	[[ -e $self.wiping ]] && wipe_file "$self.wiping"
	rmdir "$WK_DIR"
	if [[ -e $WK_DIR ]]; then
		log "FAIL settings not wiped"
		return 1
	fi
	systemctl disable wukong-post-network.service >/dev/null 2>&1
	rm -f "$WK_POST_UNIT" "$USER_SCRIPT_RAN"
	systemctl daemon-reload
	sync
	log "OK settings wiped"
}

main() {
	local online='' wifi_wait='' wiped=1
	if [[ -r $WK_ENV ]]; then
		# shellcheck source=/dev/null
		. "$WK_ENV"
	fi
	log "started"

	if [[ ${WK_WIFI_ENABLED:-} == 1 ]]; then
		wait_for_wifi &
		wifi_wait=$!
	fi
	if wait_for_route; then
		online=1
		wait_for_clock
	fi
	if [[ -n ${WK_HOSTNAME:-} ]]; then
		if [[ -n $online ]]; then
			install_avahi
		else
			log "FAIL avahi-daemon skipped, no network"
		fi
	fi
	[[ -n $wifi_wait ]] && wait "$wifi_wait"

	stage_user_script
	wipe_everything || wiped=
	save_log
	if [[ -f $USER_SCRIPT_RUN ]]; then
		# The stage repeats on the next boot, and the script must not come back with it.
		if [[ -z $wiped ]]; then
			touch "$USER_SCRIPT_RAN"
			sync
		fi
		run_user_script
	fi
	log "finished"
	save_log
}

main "$@"; exit 0
