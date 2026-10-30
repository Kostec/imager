#!/bin/bash
# Wukong Imager: first boot, the stage after the network.
#
# firstboot.sh starts this as wukong-post-network.service, and it stays enabled
# until it has finished, so a power cut repeats it on the next boot. It waits
# for a route, the clock and avahi-daemon, runs the user's script and wipes
# everything the imager left on the card, itself included.
#
# Never print a value from firstboot.conf, and no `set -x`.

WK_DIR=/etc/systemd/system/armbian-firstrun.service.d
WK_CONF=$WK_DIR/firstboot.conf
WK_LOG=/var/log/wukong-firstboot.log
WK_POST_UNIT=/etc/systemd/system/wukong-post-network.service
ROUTE_WAIT_SECS=120
CLOCK_WAIT_SECS=60
APT_TIMEOUT_SECS=300
APT_LOCK_WAIT_SECS=120
USER_SCRIPT_TIMEOUT_SECS=300

# apt and the user's script must leave files others can read, as under a login shell.
umask 022

log() {
	printf '%s post-network: %s\n' "$(date '+%F %T')" "$*" >>"$WK_LOG" 2>/dev/null
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

# The image has no avahi-daemon, and name.local needs it.
install_avahi() {
	if dpkg-query -W -f='${Status}' avahi-daemon 2>/dev/null | grep -q 'ok installed'; then
		log "OK avahi-daemon already installed"
		return 0
	fi
	if timeout "$APT_TIMEOUT_SECS" apt-get update -qq >>"$WK_LOG" 2>&1 &&
		DEBIAN_FRONTEND=noninteractive timeout "$APT_TIMEOUT_SECS" apt-get install -y -qq \
			--no-install-recommends -o "DPkg::Lock::Timeout=$APT_LOCK_WAIT_SECS" avahi-daemon >>"$WK_LOG" 2>&1; then
		log "OK avahi-daemon installed"
	else
		log "FAIL avahi-daemon not installed, name.local will not resolve"
	fi
}

run_user_script() {
	local status
	timeout "$USER_SCRIPT_TIMEOUT_SECS" /bin/bash "$WK_DIR/user-script.sh" >>"$WK_LOG" 2>&1
	status=$?
	if ((status == 0)); then
		log "OK user script"
	elif ((status == 124)); then
		log "FAIL user script stopped after ${USER_SCRIPT_TIMEOUT_SECS}s"
	else
		log "FAIL user script (exit $status)"
	fi
}

# Runs from the very file it overwrites: bash has parsed all of it by the time main starts.
wipe_everything() {
	local file
	systemctl disable wukong-post-network.service >/dev/null 2>&1
	for file in "$WK_DIR"/*; do
		shred -u -n 1 "$file" || rm -f "$file"
	done
	rmdir "$WK_DIR"
	rm -f "$WK_POST_UNIT"
	systemctl daemon-reload
	sync
	log "OK settings wiped"
}

main() {
	local online=
	if [[ -r $WK_CONF ]]; then
		# shellcheck source=/dev/null
		. "$WK_CONF"
	fi
	log "started"

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
	if [[ -f $WK_DIR/user-script.sh ]]; then
		run_user_script
	fi

	wipe_everything
	log "finished"
}

main "$@"; exit 0
