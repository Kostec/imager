#!/bin/bash
# shellcheck disable=SC2329 # step() calls the apply_ functions by name
# Wukong Imager: first boot, the stage before the network.
#
# Runs as ExecStartPre of armbian-firstrun.service, before NetworkManager, ssh
# and any login prompt, every time the board starts until armbian-firstrun has
# finished once. So every step can run twice, and none may stop the next one.
#
# Never print a hash, a Wi-Fi key or an authorized key, and never pass one in
# the arguments of a command: printf is a builtin, chpasswd reads stdin. No
# `set -x`. No `set -e` either: a failed step is logged and the rest goes on.

WK_DIR=/etc/systemd/system/armbian-firstrun.service.d
WK_ENV=$WK_DIR/firstboot.env
WK_LOG=/var/log/wukong-firstboot.log
WK_LOG_DISK=/var/log.hdd
WK_POST_UNIT=/etc/systemd/system/wukong-post-network.service
NM_DIR=/etc/NetworkManager/system-connections
USER_GROUPS="sudo netdev audio video disk tty users games dialout plugdev input bluetooth systemd-journal ssh"

# What actually worked. Root is locked and the first-login wizard dropped on these, not on what the profile asked for.
user_has_password=
user_has_keys=
user_can_log_in=

# Secrets live here, and in the profiles written below.
umask 077

log() {
	printf '%s firstboot: %s\n' "$(date '+%F %T')" "$*" >>"$WK_LOG" 2>/dev/null
}

# /var/log is in RAM on Armbian. What is copied here comes back at the next boot, even after a power cut.
save_log() {
	# With armbian-ramlog down, /var/log.hdd is a bind mount of the card's own /var/log and this is the same file.
	if [[ -d $WK_LOG_DISK && ! $WK_LOG -ef $WK_LOG_DISK/${WK_LOG##*/} ]]; then
		cp -p "$WK_LOG" "$WK_LOG_DISK/" 2>/dev/null || log "FAIL log not copied to $WK_LOG_DISK"
	fi
	sync
}

# step NAME FUNCTION: one line of result per step, whatever the function does.
step() {
	local name=$1 fn=$2
	if "$fn" >>"$WK_LOG" 2>&1; then
		log "OK $name"
	else
		log "FAIL $name (exit $?)"
	fi
}

apply_hostname() {
	local old
	old=$(cat /etc/hostname 2>/dev/null)
	# /etc/hosts first: a power cut in between must leave the old name findable here.
	if [[ -n $old && $old != "$WK_HOSTNAME" ]]; then
		sed -i "s/\b${old//./\\.}\b/$WK_HOSTNAME/g" /etc/hosts || return 1
	fi
	printf '%s\n' "$WK_HOSTNAME" >/etc/hostname || return 1
	chmod 644 /etc/hostname
	hostname "$WK_HOSTNAME" || true
}

# timedatectl needs dbus, which may not be up yet; these two files are all it would write.
apply_timezone() {
	[[ -f /usr/share/zoneinfo/$WK_TIMEZONE ]] || return 1
	ln -sf "/usr/share/zoneinfo/$WK_TIMEZONE" /etc/localtime || return 1
	printf '%s\n' "$WK_TIMEZONE" >/etc/timezone || return 1
	chmod 644 /etc/timezone
}

apply_locale() {
	local escaped=${WK_LOCALE//./\\.}
	grep -q "^$escaped " /usr/share/i18n/SUPPORTED || return 1
	sed -i "s/^# *\($escaped \)/\1/" /etc/locale.gen || return 1
	if ! grep -q "^$escaped " /etc/locale.gen; then
		grep "^$escaped " /usr/share/i18n/SUPPORTED >>/etc/locale.gen || return 1
	fi
	# The archive must stay readable by everyone, whatever this script's umask.
	(umask 022 && locale-gen >/dev/null) || return 1
	printf 'LANG=%s\nLC_ALL=%s\nLANGUAGE=%s\n' "$WK_LOCALE" "$WK_LOCALE" "$WK_LOCALE" >/etc/default/locale || return 1
	chmod 644 /etc/default/locale
}

# armbian-firstrun runs setupcon right after this script.
apply_keyboard() {
	sed -i "s/^XKBLAYOUT=.*/XKBLAYOUT=\"$WK_KEYBOARD\"/" /etc/default/keyboard
}

apply_wifi_country() {
	if grep -q '^REGDOMAIN=' /etc/default/crda; then
		sed -i "s/^REGDOMAIN=.*/REGDOMAIN=$WK_WIFI_COUNTRY/" /etc/default/crda || return 1
	else
		printf 'REGDOMAIN=%s\n' "$WK_WIFI_COUNTRY" >>/etc/default/crda || return 1
	fi
	# No radio driver loaded yet is normal here: udev applies the file when it loads.
	iw reg set "$WK_WIFI_COUNTRY" || log "iw reg set did not apply yet"
}

# An SSID is any 1..32 bytes; NetworkManager reads a list of byte values whatever they spell.
ssid_as_bytes() {
	printf '%s' "$1" | od -An -v -tu1 | tr -s ' \n' ';' | sed 's/^;//'
}

ipv4_section() {
	local dns
	printf '\n[ipv4]\n'
	if [[ ${WK_STATIC_ENABLED:-} == 1 ]]; then
		printf 'method=manual\n'
		printf 'address1=%s/%s%s\n' "$WK_STATIC_IP" "$WK_STATIC_PREFIX" "${WK_STATIC_GATEWAY:+,$WK_STATIC_GATEWAY}"
		if [[ -n ${WK_STATIC_DNS:-} ]]; then
			dns=${WK_STATIC_DNS// /;}
			printf 'dns=%s;\n' "$dns"
		fi
	else
		printf 'method=auto\n'
	fi
	printf '\n[ipv6]\nmethod=auto\n'
}

# NetworkManager is not running yet, so nmcli has nothing to talk to: the profile goes straight to its directory.
write_profile() {
	local file=$NM_DIR/$1.nmconnection
	cat >"$file" || return 1
	chown root:root "$file"
	chmod 600 "$file"
}

apply_wifi() {
	{
		printf '[connection]\nid=wukong-wifi\nuuid=%s\ntype=wifi\nautoconnect=true\n' "$(cat /proc/sys/kernel/random/uuid)"
		printf '\n[wifi]\nmode=infrastructure\nssid=%s\n' "$(ssid_as_bytes "$WK_WIFI_SSID")"
		if [[ ${WK_WIFI_HIDDEN:-} == 1 ]]; then
			printf 'hidden=true\n'
		fi
		if [[ -n ${WK_WIFI_PSK:-} ]]; then
			printf '\n[wifi-security]\nkey-mgmt=wpa-psk\npsk=%s\n' "$WK_WIFI_PSK"
		fi
		ipv4_section
	} | write_profile wukong-wifi
}

# Ethernet needs a profile only for a static address: otherwise NetworkManager's own DHCP profile stays, Wi-Fi or not.
apply_ethernet() {
	{
		printf '[connection]\nid=wukong-ethernet\nuuid=%s\ntype=ethernet\nautoconnect=true\nautoconnect-priority=10\n' "$(cat /proc/sys/kernel/random/uuid)"
		printf '\n[ethernet]\n'
		ipv4_section
	} | write_profile wukong-ethernet
}

apply_user() {
	local name=$WK_USER_NAME shell=/bin/bash real_name group
	if [[ ${WK_USER_SHELL:-} == zsh ]]; then
		shell=$(command -v zsh || printf '/bin/bash')
	fi
	real_name=$(printf '%s' "${WK_USER_REALNAME:-}" | tr -d '[:cntrl:]:,=')
	if id -u "$name" >/dev/null 2>&1; then
		# A second run finds its own user; a system account is not ours to touch.
		(($(id -u "$name") >= 1000)) || return 1
		usermod --shell "$shell" --comment "$real_name" "$name" || return 1
	else
		useradd --create-home --shell "$shell" --comment "$real_name" "$name" || return 1
	fi
	for group in $USER_GROUPS; do
		if getent group "$group" >/dev/null; then
			usermod -aG "$group" "$name" || log "group $group: not added"
		fi
	done
	if [[ -n ${WK_USER_HASH:-} ]]; then
		printf '%s:%s\n' "$name" "$WK_USER_HASH" | chpasswd -e 2>/dev/null || return 1
		user_has_password=1
	fi
	touch "/home/$name/.Xauthority"
	chown "$name:" "/home/$name/.Xauthority"
}

# Keys open nothing while ssh is masked.
decide_login() {
	if [[ -n $user_has_password ]]; then
		user_can_log_in=1
	elif [[ -n $user_has_keys && ${WK_SSH_ENABLED:-} != 0 ]]; then
		user_can_log_in=1
	fi
}

user_has_sudo() {
	[[ -n $user_has_password && " $(id -nG "$WK_USER_NAME") " == *" sudo "* ]]
}

apply_root() {
	if [[ -n ${WK_ROOT_HASH:-} ]]; then
		printf 'root:%s\n' "$WK_ROOT_HASH" | chpasswd -e 2>/dev/null
	elif user_has_sudo; then
		passwd -l root >/dev/null || return 1
		# The image marks root's password as expired for the wizard, and PAM then refuses root's cron jobs.
		chage -d "$(date +%F)" root
	fi
}

install_keys() {
	local owner=$1 home=$2
	mkdir -p "$home/.ssh" || return 1
	printf '%s\n' "$WK_SSH_KEYS" >"$home/.ssh/authorized_keys" || return 1
	chmod 700 "$home/.ssh"
	chmod 600 "$home/.ssh/authorized_keys"
	chown -R "$owner:" "$home/.ssh"
}

# The keys belong to the new user; root gets them when there is no user or when root may log in.
apply_ssh_keys() {
	local status=0
	if [[ -n ${WK_USER_NAME:-} ]]; then
		if id -u "$WK_USER_NAME" >/dev/null 2>&1 && install_keys "$WK_USER_NAME" "/home/$WK_USER_NAME"; then
			user_has_keys=1
		else
			status=1
		fi
	fi
	if [[ -z ${WK_USER_NAME:-} || ${WK_SSH_ROOT_LOGIN:-} == 1 ]]; then
		install_keys root /root || status=1
	fi
	return "$status"
}

yes_no() {
	if [[ $1 == 1 ]]; then printf 'yes'; else printf 'no'; fi
}

# ssh.service would stop at /run/sshd without its RuntimeDirectory, and armbian-firstrun may be regenerating host keys.
sshd_config_is_valid() {
	local tmp status=1
	tmp=$(mktemp -d) || return 1
	mkdir -p /run/sshd
	if ssh-keygen -q -t ed25519 -N '' -f "$tmp/key" >/dev/null; then
		/usr/sbin/sshd -t -h "$tmp/key"
		status=$?
	fi
	rm -rf "$tmp"
	return "$status"
}

sshd_set_option() {
	local option=$1 value=$2
	if grep -qE "^#?$option " /etc/ssh/sshd_config; then
		sed -i -E "s/^#?$option .*/$option $value/" /etc/ssh/sshd_config
	else
		printf '%s %s\n' "$option" "$value" >>/etc/ssh/sshd_config
	fi
}

apply_sshd() {
	local dropin=/etc/ssh/sshd_config.d/10-wukong.conf backup=/etc/ssh/sshd_config.wukong-backup
	local lines=()
	if [[ -n ${WK_SSH_PASSWORD_AUTH:-} ]]; then
		lines+=("PasswordAuthentication $(yes_no "$WK_SSH_PASSWORD_AUTH")")
	fi
	if [[ -n ${WK_SSH_ROOT_LOGIN:-} ]]; then
		lines+=("PermitRootLogin $(yes_no "$WK_SSH_ROOT_LOGIN")")
	fi

	# The first value wins, and the Include sits at the top of sshd_config.
	if grep -q '^Include /etc/ssh/sshd_config.d/' /etc/ssh/sshd_config; then
		printf '%s\n' "${lines[@]}" >"$dropin" || return 1
		chmod 644 "$dropin"
		sshd_config_is_valid && return 0
		rm -f "$dropin"
		return 1
	fi

	cp -p /etc/ssh/sshd_config "$backup" || return 1
	local line
	for line in "${lines[@]}"; do
		sshd_set_option "${line%% *}" "${line#* }"
	done
	if sshd_config_is_valid; then
		rm -f "$backup"
		return 0
	fi
	mv -f "$backup" /etc/ssh/sshd_config
	return 1
}

# mask, not disable: armbian-firstrun runs `service sshd restart`, and that would start a disabled service.
apply_ssh_service() {
	if [[ $WK_SSH_ENABLED == 1 ]]; then
		systemctl unmask ssh.service
		systemctl enable ssh.service
	else
		systemctl mask ssh.service
	fi
}

apply_overlays() {
	local env=/boot/armbianEnv.txt merged overlay
	merged=$(sed -n 's/^overlays=//p' "$env" | head -n 1)
	for overlay in $WK_OVERLAYS; do
		if [[ " $merged " != *" $overlay "* ]]; then
			merged="${merged:+$merged }$overlay"
		fi
	done
	if grep -q '^overlays=' "$env"; then
		sed -i "s/^overlays=.*/overlays=$merged/" "$env" || return 1
	else
		printf 'overlays=%s\n' "$merged" >>"$env" || return 1
	fi
	if [[ " $merged " == *" spi-spidev "* ]] && ! grep -q '^param_spidev_spi_bus=' "$env"; then
		printf 'param_spidev_spi_bus=0\n' >>"$env" || return 1
	fi
}

# armbian-firstlogin is what normally removes these; without it root keeps its console autologin for good.
skip_wizard() {
	rm -f /root/.not_logged_in_yet
	rm -f /etc/systemd/system/getty@.service.d/override.conf
	rm -f /etc/systemd/system/serial-getty@.service.d/override.conf
	chmod +x /etc/update-motd.d/* 2>/dev/null
	systemctl daemon-reload
}

# armbian-ramlog has 30 s by default, and rsync copies by name: on a slow start it is killed with /var/log already in RAM
# and this log, near the end of the list, not back yet. It stays on the card, but /var/log no longer shows it.
apply_ramlog_timeout() {
	local dir=/etc/systemd/system/armbian-ramlog.service.d
	mkdir -p "$dir" || return 1
	chmod 755 "$dir"
	printf '[Service]\nTimeoutStartSec=5min\n' >"$dir/10-wukong.conf" || return 1
	chmod 644 "$dir/10-wukong.conf"
}

# The boot transaction is already computed, so enable alone would only take effect on the next boot.
start_post_network() {
	cat >"$WK_POST_UNIT" <<EOF || return 1
[Unit]
Description=Wukong first boot, stage after the network
After=network.target armbian-firstrun.service
ConditionPathExists=$WK_DIR/post-network.sh

[Service]
Type=simple
ExecStart=/bin/bash $WK_DIR/post-network.sh

[Install]
WantedBy=multi-user.target
EOF
	chmod 644 "$WK_POST_UNIT"
	systemctl daemon-reload
	systemctl enable wukong-post-network.service || return 1
	systemctl --no-block start wukong-post-network.service
}

main() {
	if [[ ! -r $WK_ENV ]]; then
		log "no settings file, nothing to do"
		return 0
	fi
	# shellcheck source=/dev/null
	. "$WK_ENV"
	log "started"

	[[ -n ${WK_HOSTNAME:-} ]] && step hostname apply_hostname
	[[ -n ${WK_TIMEZONE:-} ]] && step timezone apply_timezone
	[[ -n ${WK_LOCALE:-} ]] && step locale apply_locale
	[[ -n ${WK_KEYBOARD:-} ]] && step keyboard apply_keyboard
	[[ -n ${WK_WIFI_COUNTRY:-} ]] && step wifi-country apply_wifi_country
	if [[ ${WK_WIFI_ENABLED:-} == 1 ]]; then
		step wifi apply_wifi
	elif [[ ${WK_STATIC_ENABLED:-} == 1 ]]; then
		step ethernet apply_ethernet
	fi
	[[ -n ${WK_USER_NAME:-} ]] && step user apply_user
	[[ -n ${WK_SSH_KEYS:-} ]] && step ssh-keys apply_ssh_keys
	[[ -n ${WK_SSH_PASSWORD_AUTH:-}${WK_SSH_ROOT_LOGIN:-} ]] && step sshd apply_sshd
	[[ -n ${WK_SSH_ENABLED:-} ]] && step ssh-service apply_ssh_service
	decide_login
	[[ -n ${WK_ROOT_HASH:-} ]] || user_has_sudo && step root apply_root
	[[ -n ${WK_OVERLAYS:-} ]] && step overlays apply_overlays
	[[ -n $user_can_log_in ]] && step skip-wizard skip_wizard
	step ramlog-timeout apply_ramlog_timeout
	step post-network start_post_network
	log "finished"
	save_log
	return 0
}

main "$@"
exit 0
