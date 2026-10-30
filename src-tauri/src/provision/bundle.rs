// SPDX-License-Identifier: GPL-2.0-or-later

//! Settings as files: the drop-in, the device scripts and `firstboot.env`. A pure function, the image is not involved.

use super::config::AutoconfigConfig;
use super::secrets::is_wifi_psk;
use super::ProvisionError;

pub(super) const DROPIN_DIR: &str = "/etc/systemd/system/armbian-firstrun.service.d";
const FIRSTBOOT_SH: &str = include_str!("../../device/firstboot.sh");
const POST_NETWORK_SH: &str = include_str!("../../device/post-network.sh");
const USER_SCRIPT_MAX_BYTES: usize = 64 * 1024;
const HOSTNAME_MAX_BYTES: usize = 253;
const HOSTNAME_LABEL_MAX_BYTES: usize = 63;
const USER_NAME_MAX_BYTES: usize = 32;
const SSID_MAX_BYTES: usize = 32;
const MODE_DROPIN: u16 = 0o644;
const MODE_SCRIPT: u16 = 0o755;
const MODE_ENV: u16 = 0o600;
const MODE_USER_SCRIPT: u16 = 0o700;

/// A file to create on the card, owned by root.
pub struct BundleFile {
    pub path: String,
    pub mode: u16,
    pub data: Vec<u8>,
}

fn structural(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

fn flag(value: bool) -> String {
    String::from(if value { "1" } else { "0" })
}

/// The dotted mask as a prefix length; anything that is not one is kept for `check` to refuse.
fn prefix_of(mask: &str) -> String {
    let bits = mask
        .parse::<std::net::Ipv4Addr>()
        .ok()
        .map(u32::from)
        .filter(|bits| bits.leading_ones() + bits.trailing_zeros() == 32);
    bits.map_or(mask.to_string(), |bits| bits.leading_ones().to_string())
}

fn dns_list(value: &str) -> String {
    let servers: Vec<&str> = value
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .collect();
    servers.join(" ")
}

/// One key per line, without the blank lines and CRLF a pasted list brings along.
fn key_lines(value: &str) -> String {
    let lines: Vec<&str> = value
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    lines.join("\n")
}

/// What `firstboot.env` will hold, in file order; `ethernet_enabled` has no key, Ethernet is never touched.
fn assignments(cfg: &AutoconfigConfig) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let mut push = |key: &'static str, value: Option<String>| {
        if let Some(value) = value {
            out.push((key, value));
        }
    };

    push("WK_HOSTNAME", structural(&cfg.hostname));
    push("WK_TIMEZONE", structural(&cfg.timezone));
    push("WK_LOCALE", structural(&cfg.locale));
    push("WK_KEYBOARD", structural(&cfg.keyboard_layout));
    // Gated so that a switched-off Wi-Fi or static address leaves nothing stale behind.
    if cfg.wifi_enabled == Some(true) {
        push("WK_WIFI_ENABLED", Some(flag(true)));
        // Verbatim: spaces can be meaningful in an SSID.
        push(
            "WK_WIFI_SSID",
            cfg.wifi_ssid.clone().filter(|v| !v.is_empty()),
        );
        push("WK_WIFI_PSK", structural(&cfg.wifi_psk));
        push(
            "WK_WIFI_COUNTRY",
            structural(&cfg.wifi_country_code).map(|v| v.to_ascii_uppercase()),
        );
        push("WK_WIFI_HIDDEN", cfg.wifi_hidden.map(flag));
    }
    if cfg.use_static_ip == Some(true) {
        push("WK_STATIC_ENABLED", Some(flag(true)));
        push("WK_STATIC_IP", structural(&cfg.static_ip));
        push(
            "WK_STATIC_PREFIX",
            structural(&cfg.static_mask).map(|v| prefix_of(&v)),
        );
        push("WK_STATIC_GATEWAY", structural(&cfg.static_gateway));
        push(
            "WK_STATIC_DNS",
            structural(&cfg.static_dns).map(|v| dns_list(&v)),
        );
    }
    push("WK_USER_NAME", structural(&cfg.user_name));
    push("WK_USER_REALNAME", structural(&cfg.user_real_name));
    push("WK_USER_HASH", structural(&cfg.user_password_hash));
    push(
        "WK_USER_SHELL",
        cfg.user_shell.as_ref().map(|v| v.as_str().to_string()),
    );
    push("WK_ROOT_HASH", structural(&cfg.root_password_hash));
    push("WK_SSH_ENABLED", cfg.ssh_enabled.map(flag));
    push("WK_SSH_PASSWORD_AUTH", cfg.ssh_password_auth.map(flag));
    push("WK_SSH_ROOT_LOGIN", cfg.ssh_root_login.map(flag));
    push(
        "WK_SSH_KEYS",
        cfg.ssh_authorized_keys
            .as_deref()
            .map(key_lines)
            .filter(|v| !v.is_empty()),
    );
    let overlays: Vec<String> = cfg
        .overlays
        .iter()
        .flatten()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();
    push(
        "WK_OVERLAYS",
        (!overlays.is_empty()).then(|| overlays.join(" ")),
    );
    out
}

fn user_script(cfg: &AutoconfigConfig) -> Option<String> {
    cfg.user_script
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.replace("\r\n", "\n"))
}

/// Nothing to apply: the image is written as it is, without a working copy.
pub fn is_empty(cfg: &AutoconfigConfig) -> bool {
    assignments(cfg).is_empty() && user_script(cfg).is_none()
}

fn invalid<T>(what: &str) -> Result<T, ProvisionError> {
    Err(ProvisionError::InvalidConfig(what.to_string()))
}

fn is_hostname(value: &str) -> bool {
    value.len() <= HOSTNAME_MAX_BYTES
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= HOSTNAME_LABEL_MAX_BYTES
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
}

/// `^[a-z_][a-z0-9_-]{0,31}$`, and not root, which has its own hash field.
fn is_user_name(value: &str) -> bool {
    value.len() <= USER_NAME_MAX_BYTES
        && value != "root"
        && value
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b == b'_')
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
}

/// `$6$[rounds=N$]salt$hash`: nothing else may reach `chpasswd -e`, a plain password least of all.
fn is_sha512_hash(value: &str) -> bool {
    value.strip_prefix("$6$").is_some_and(|rest| {
        rest.contains('$')
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'/' | b'$' | b'='))
    })
}

fn only(value: &str, allowed: impl Fn(u8) -> bool) -> bool {
    !value.is_empty() && value.bytes().all(allowed)
}

fn is_ipv4(value: &str) -> bool {
    value.parse::<std::net::Ipv4Addr>().is_ok()
}

fn has_no_control(value: &str) -> bool {
    !value.chars().any(char::is_control)
}

/// Scripts run these as root and some go through sed, so each must be exactly what its key expects.
fn check(key: &str, value: &str) -> Result<(), ProvisionError> {
    let ok = match key {
        "WK_HOSTNAME" => is_hostname(value),
        "WK_TIMEZONE" => {
            !value.starts_with('/')
                && !value.contains("..")
                && only(value, |b| {
                    b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'+' | b'-')
                })
        }
        "WK_LOCALE" => only(value, |b| {
            b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'@' | b'-')
        }),
        "WK_KEYBOARD" => only(value, |b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b',' | b'-')
        }),
        "WK_WIFI_SSID" => value.len() <= SSID_MAX_BYTES && has_no_control(value),
        "WK_WIFI_PSK" => is_wifi_psk(value),
        "WK_WIFI_COUNTRY" => value.len() == 2 && only(value, |b| b.is_ascii_uppercase()),
        "WK_STATIC_IP" | "WK_STATIC_GATEWAY" => is_ipv4(value),
        "WK_STATIC_PREFIX" => value.parse::<u8>().is_ok_and(|prefix| prefix <= 32),
        "WK_STATIC_DNS" => value.split(' ').all(is_ipv4),
        "WK_USER_NAME" => is_user_name(value),
        "WK_USER_REALNAME" => has_no_control(value),
        "WK_USER_HASH" | "WK_ROOT_HASH" => is_sha512_hash(value),
        "WK_SSH_KEYS" => value.lines().all(has_no_control),
        "WK_OVERLAYS" => value.split(' ').all(|overlay| {
            only(overlay, |b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'
            })
        }),
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        // The key, never the value: this ends up in a log and on screen.
        invalid(key)
    }
}

fn check_required(lines: &[(&'static str, String)]) -> Result<(), ProvisionError> {
    let has = |key: &str| lines.iter().any(|(k, _)| *k == key);
    if has("WK_WIFI_ENABLED") && !has("WK_WIFI_SSID") {
        return invalid("WK_WIFI_SSID");
    }
    // firstboot.sh locks root for a user with sudo, and sudo needs that user's password: without either hash, no way to root.
    if has("WK_USER_NAME") && !has("WK_USER_HASH") && !has("WK_ROOT_HASH") {
        return invalid("WK_USER_HASH");
    }
    if has("WK_STATIC_ENABLED") {
        for key in ["WK_STATIC_IP", "WK_STATIC_PREFIX"] {
            if !has(key) {
                return invalid(key);
            }
        }
    }
    Ok(())
}

/// `KEY='value'`, safe to source: inside single quotes only the quote itself needs care.
fn render_env(lines: &[(&'static str, String)]) -> String {
    let mut out = String::new();
    for (key, value) in lines {
        out.push_str(key);
        out.push_str("='");
        out.push_str(&value.replace('\'', "'\\''"));
        out.push_str("'\n");
    }
    out
}

/// The orderings go beyond the unit's own `Before=getty.target`: serial getties, the console of this board, only
/// wait for systemd-user-sessions, and armbian-firstrun has no order at all against NetworkManager or ssh.
fn render_dropin() -> String {
    format!(
        "[Unit]\n\
         Before=NetworkManager.service ssh.service systemd-user-sessions.service\n\
         \n\
         [Service]\n\
         ExecStartPre=-/bin/bash {DROPIN_DIR}/firstboot.sh\n\
         TimeoutStartSec=15min\n"
    )
}

/// Every file for the card; empty when `is_empty`. Refuses a value that could not be applied or does not belong on it.
pub fn render_bundle(cfg: &AutoconfigConfig) -> Result<Vec<BundleFile>, ProvisionError> {
    if is_empty(cfg) {
        return Ok(Vec::new());
    }
    let lines = assignments(cfg);
    check_required(&lines)?;
    for (key, value) in &lines {
        check(key, value)?;
    }
    let script = user_script(cfg);
    if script
        .as_ref()
        .is_some_and(|s| s.len() > USER_SCRIPT_MAX_BYTES)
    {
        return invalid("the first-boot script is over 64 KiB");
    }

    let file = |name: &str, mode: u16, data: Vec<u8>| BundleFile {
        path: format!("{DROPIN_DIR}/{name}"),
        mode,
        data,
    };
    let mut files = vec![
        file("10-wukong.conf", MODE_DROPIN, render_dropin().into_bytes()),
        file(
            "firstboot.sh",
            MODE_SCRIPT,
            FIRSTBOOT_SH.as_bytes().to_vec(),
        ),
        file(
            "post-network.sh",
            MODE_SCRIPT,
            POST_NETWORK_SH.as_bytes().to_vec(),
        ),
        file("firstboot.env", MODE_ENV, render_env(&lines).into_bytes()),
    ];
    if let Some(script) = script {
        files.push(file(
            "user-script.sh",
            MODE_USER_SCRIPT,
            script.into_bytes(),
        ));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::super::{hash_password, UserShell};
    use super::*;

    const DIR: &str = "/etc/systemd/system/armbian-firstrun.service.d";
    const PSK: &str = "f42c6fc52df0ebef9ebb4b90b38a5f902e83fe1b135a70e23aed762e9710a12e";
    const HASH: &str = "$6$saltstring$svn8UoSVapNtMuq1ukKS4tPQd8iKwSMHWjl/O817G3uBnIFNjnQJuesI68u4OTLiBFdcbYEdFCoEOfaS35inz1";

    fn s(value: &str) -> Option<String> {
        Some(value.to_string())
    }

    fn wifi() -> AutoconfigConfig {
        AutoconfigConfig {
            wifi_enabled: Some(true),
            wifi_ssid: s("home"),
            wifi_psk: s(PSK),
            wifi_country_code: s("de"),
            ..AutoconfigConfig::default()
        }
    }

    fn full() -> AutoconfigConfig {
        AutoconfigConfig {
            wifi_hidden: Some(true),
            use_static_ip: Some(true),
            static_ip: s("192.168.1.50 "),
            static_mask: s(" 255.255.255.0"),
            static_gateway: s("192.168.1.1"),
            static_dns: s(" 8.8.8.8, 1.1.1.1 "),
            hostname: s("wukong"),
            locale: s("ru_RU.UTF-8"),
            timezone: s("Europe/Moscow"),
            keyboard_layout: s("ru"),
            root_password_hash: s(HASH),
            user_name: s("anna"),
            user_password_hash: s(HASH),
            user_shell: Some(UserShell::Zsh),
            user_real_name: s("Анна"),
            ssh_enabled: Some(true),
            ssh_password_auth: Some(false),
            ssh_root_login: Some(false),
            ssh_authorized_keys: s("ssh-ed25519 AAAA one\r\n\r\nssh-rsa BBBB two\r\n"),
            overlays: Some(vec!["i2c0".to_string(), "spi-spidev".to_string()]),
            user_script: s("echo hello\r\n"),
            ..wifi()
        }
    }

    fn file<'a>(files: &'a [BundleFile], name: &str) -> &'a BundleFile {
        let path = format!("{DIR}/{name}");
        files
            .iter()
            .find(|f| f.path == path)
            .unwrap_or_else(|| panic!("no {path}"))
    }

    fn env(config: &AutoconfigConfig) -> String {
        let files = render_bundle(config).unwrap();
        String::from_utf8(file(&files, "firstboot.env").data.clone()).unwrap()
    }

    fn assert_refused(config: AutoconfigConfig, secret: &str) {
        match render_bundle(&config) {
            Err(ProvisionError::InvalidConfig(why)) => {
                assert!(!why.contains(secret), "{why}");
            }
            Err(other) => panic!("wrong error: {other}"),
            Ok(_) => panic!("a bad value went through"),
        }
    }

    #[test]
    fn nothing_set_means_nothing_to_write() {
        let cases = [
            AutoconfigConfig::default(),
            AutoconfigConfig {
                ethernet_enabled: Some(true),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                wifi_enabled: Some(false),
                wifi_ssid: s("left over"),
                wifi_psk: s(PSK),
                use_static_ip: Some(false),
                static_ip: s("10.0.0.1"),
                hostname: s("   "),
                user_script: s(" \n "),
                overlays: Some(vec![String::new()]),
                ..AutoconfigConfig::default()
            },
        ];
        for config in cases {
            assert!(is_empty(&config));
            assert!(render_bundle(&config).unwrap().is_empty());
        }
    }

    #[test]
    fn one_setting_is_enough_to_write_the_bundle() {
        let config = AutoconfigConfig {
            hostname: s("wukong"),
            ..AutoconfigConfig::default()
        };
        assert!(!is_empty(&config));
        assert_eq!(env(&config), "WK_HOSTNAME='wukong'\n");
    }

    #[test]
    fn the_bundle_has_the_files_and_modes_of_the_interface() {
        let files = render_bundle(&full()).unwrap();
        let modes: Vec<(&str, u16)> = files
            .iter()
            .map(|f| (f.path.rsplit('/').next().unwrap(), f.mode))
            .collect();
        assert_eq!(
            modes,
            [
                ("10-wukong.conf", 0o644),
                ("firstboot.sh", 0o755),
                ("post-network.sh", 0o755),
                ("firstboot.env", 0o600),
                ("user-script.sh", 0o700),
            ]
        );
        assert!(files.iter().all(|f| f.path.starts_with(DIR)));
    }

    #[test]
    fn a_user_without_a_password_needs_a_root_password() {
        let keys_only = || AutoconfigConfig {
            user_name: s("anna"),
            ssh_authorized_keys: s("ssh-ed25519 AAAA one"),
            ..AutoconfigConfig::default()
        };
        // Nothing could become root: sudo asks for a password and root gets locked.
        assert_refused(keys_only(), "no value to look for");
        assert!(render_bundle(&AutoconfigConfig {
            root_password_hash: s(HASH),
            ..keys_only()
        })
        .is_ok());
        assert!(render_bundle(&AutoconfigConfig {
            user_password_hash: s(HASH),
            ..keys_only()
        })
        .is_ok());
    }

    #[test]
    fn systemd_reads_only_the_dropin_from_this_directory() {
        let conf_files: Vec<String> = render_bundle(&full())
            .unwrap()
            .into_iter()
            .map(|f| f.path)
            .filter(|p| p.ends_with(".conf"))
            .collect();
        assert_eq!(conf_files, [format!("{DIR}/10-wukong.conf")]);
    }

    #[test]
    fn no_user_script_no_file() {
        let files = render_bundle(&wifi()).unwrap();
        assert_eq!(files.len(), 4);
    }

    #[test]
    fn the_dropin_orders_the_scripts_before_network_ssh_and_logins() {
        let files = render_bundle(&wifi()).unwrap();
        let dropin = String::from_utf8(file(&files, "10-wukong.conf").data.clone()).unwrap();
        assert_eq!(
            dropin,
            format!(
                "[Unit]\n\
                 Before=NetworkManager.service ssh.service systemd-user-sessions.service\n\
                 \n\
                 [Service]\n\
                 ExecStartPre=-/bin/bash {DIR}/firstboot.sh\n\
                 TimeoutStartSec=15min\n"
            )
        );
    }

    #[test]
    fn a_full_profile_renders_every_key_once() {
        assert_eq!(
            env(&full()),
            format!(
                "WK_HOSTNAME='wukong'\n\
                 WK_TIMEZONE='Europe/Moscow'\n\
                 WK_LOCALE='ru_RU.UTF-8'\n\
                 WK_KEYBOARD='ru'\n\
                 WK_WIFI_ENABLED='1'\n\
                 WK_WIFI_SSID='home'\n\
                 WK_WIFI_PSK='{PSK}'\n\
                 WK_WIFI_COUNTRY='DE'\n\
                 WK_WIFI_HIDDEN='1'\n\
                 WK_STATIC_ENABLED='1'\n\
                 WK_STATIC_IP='192.168.1.50'\n\
                 WK_STATIC_PREFIX='24'\n\
                 WK_STATIC_GATEWAY='192.168.1.1'\n\
                 WK_STATIC_DNS='8.8.8.8 1.1.1.1'\n\
                 WK_USER_NAME='anna'\n\
                 WK_USER_REALNAME='Анна'\n\
                 WK_USER_HASH='{HASH}'\n\
                 WK_USER_SHELL='zsh'\n\
                 WK_ROOT_HASH='{HASH}'\n\
                 WK_SSH_ENABLED='1'\n\
                 WK_SSH_PASSWORD_AUTH='0'\n\
                 WK_SSH_ROOT_LOGIN='0'\n\
                 WK_SSH_KEYS='ssh-ed25519 AAAA one\n\
                 ssh-rsa BBBB two'\n\
                 WK_OVERLAYS='i2c0 spi-spidev'\n"
            )
        );
    }

    #[test]
    fn single_quotes_are_escaped_and_the_rest_stays_verbatim() {
        let config = AutoconfigConfig {
            wifi_ssid: s(" Кафе's $HOME `x` \"y\" \\ "),
            ..wifi()
        };
        assert!(env(&config).contains("WK_WIFI_SSID=' Кафе'\\''s $HOME `x` \"y\" \\ '\n"));
    }

    #[cfg(unix)]
    #[test]
    fn bash_reads_back_exactly_what_was_set() {
        use std::process::Command;
        let ssid = " it's $HOME `id` \"q\" \\ Кафе ";
        let keys = "ssh-ed25519 AAAA it's\nssh-rsa BBBB $x";
        let config = AutoconfigConfig {
            wifi_ssid: s(ssid),
            ssh_authorized_keys: s(keys),
            ..wifi()
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("firstboot.env");
        std::fs::write(&path, env(&config)).unwrap();
        let read = |name: &str| {
            let script = format!(". '{}'; printf %s \"${name}\"", path.display());
            let out = Command::new("bash").args(["-c", &script]).output().unwrap();
            assert!(out.stderr.is_empty(), "{out:?}");
            String::from_utf8(out.stdout).unwrap()
        };
        assert_eq!(read("WK_WIFI_SSID"), ssid);
        assert_eq!(read("WK_SSH_KEYS"), keys);
    }

    #[test]
    fn disabled_wifi_and_static_ip_leave_no_stale_keys() {
        let config = AutoconfigConfig {
            wifi_enabled: Some(false),
            use_static_ip: Some(false),
            ..full()
        };
        let text = env(&config);
        assert!(!text.contains("WK_WIFI"), "{text}");
        assert!(!text.contains("WK_STATIC"), "{text}");
    }

    #[test]
    fn an_open_network_has_no_key() {
        let config = AutoconfigConfig {
            wifi_psk: None,
            ..wifi()
        };
        assert!(!env(&config).contains("WK_WIFI_PSK"));
    }

    #[test]
    fn the_mask_may_be_dotted_or_a_prefix() {
        let with_mask = |mask: &str| AutoconfigConfig {
            hostname: s("wukong"),
            use_static_ip: Some(true),
            static_ip: s("10.1.2.3"),
            static_mask: s(mask),
            ..AutoconfigConfig::default()
        };
        assert!(env(&with_mask("255.255.0.0")).contains("WK_STATIC_PREFIX='16'\n"));
        assert!(env(&with_mask("22")).contains("WK_STATIC_PREFIX='22'\n"));
        for bad in ["255.0.255.0", "33", "mask", "255.255.255.0.0"] {
            assert_refused(with_mask(bad), bad);
        }
    }

    #[test]
    fn plain_text_in_a_secret_field_never_reaches_the_card() {
        let plain = "my plain password";
        for config in [
            AutoconfigConfig {
                user_password_hash: s(plain),
                ..full()
            },
            AutoconfigConfig {
                root_password_hash: s(plain),
                ..full()
            },
            AutoconfigConfig {
                wifi_psk: s(plain),
                ..full()
            },
            AutoconfigConfig {
                user_password_hash: s("$6$ab:cd$xx"),
                ..full()
            },
        ] {
            assert_refused(config, plain);
        }
    }

    #[test]
    fn the_typed_password_appears_in_no_file() {
        let plain = "s3cret-Pass";
        let config = AutoconfigConfig {
            user_password_hash: Some(hash_password(plain).unwrap()),
            root_password_hash: Some(hash_password(plain).unwrap()),
            ..full()
        };
        for file in render_bundle(&config).unwrap() {
            let text = String::from_utf8_lossy(&file.data);
            assert!(!text.contains(plain), "{}", file.path);
        }
    }

    #[test]
    fn values_that_could_break_out_of_their_line_are_refused() {
        let cases = [
            AutoconfigConfig {
                hostname: s("bad name"),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                hostname: s("-lead"),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                hostname: s(&"a".repeat(64)),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                user_name: s("Bad"),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                user_name: s("root"),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                user_name: s("a;b"),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                timezone: s("../../etc/passwd"),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                locale: s("en_US/UTF-8"),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                keyboard_layout: s("us\"; reboot"),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                overlays: Some(vec!["i2c0; reboot".to_string()]),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                wifi_country_code: s("DEU"),
                ..wifi()
            },
            AutoconfigConfig {
                wifi_ssid: s("line\n[Service]"),
                ..wifi()
            },
            AutoconfigConfig {
                wifi_ssid: None,
                ..wifi()
            },
            AutoconfigConfig {
                user_real_name: s("x\ny"),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                use_static_ip: Some(true),
                static_ip: s("300.1.1.1"),
                static_mask: s("24"),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                use_static_ip: Some(true),
                static_mask: s("24"),
                ..AutoconfigConfig::default()
            },
            AutoconfigConfig {
                use_static_ip: Some(true),
                static_ip: s("10.0.0.1"),
                static_mask: s("24"),
                static_dns: s("not-an-ip"),
                ..AutoconfigConfig::default()
            },
        ];
        for config in cases {
            assert_refused(config, "no value to look for");
        }
    }

    #[test]
    fn the_user_script_is_stored_with_unix_line_ends() {
        let files = render_bundle(&full()).unwrap();
        assert_eq!(file(&files, "user-script.sh").data, b"echo hello\n");
    }

    #[test]
    fn a_user_script_over_64_kib_is_refused() {
        let limit = 64 * 1024;
        let at_limit = AutoconfigConfig {
            user_script: Some("x".repeat(limit)),
            ..AutoconfigConfig::default()
        };
        assert!(render_bundle(&at_limit).is_ok());
        let over = AutoconfigConfig {
            user_script: Some("x".repeat(limit + 1)),
            ..AutoconfigConfig::default()
        };
        assert_refused(over, "xxxx");
    }

    #[test]
    fn the_device_scripts_ship_as_written_with_unix_line_ends() {
        let files = render_bundle(&wifi()).unwrap();
        for name in ["firstboot.sh", "post-network.sh"] {
            let data = &file(&files, name).data;
            assert!(data.starts_with(b"#!/bin/bash\n"), "{name}");
            assert!(!data.contains(&b'\r'), "{name} has CRLF");
            let text = String::from_utf8_lossy(data);
            assert!(!text.contains("\nset -x"), "{name}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn bash_parses_the_device_scripts() {
        use std::process::{Command, Stdio};
        let dir = tempfile::tempdir().unwrap();
        for file in render_bundle(&wifi()).unwrap() {
            if file.path.ends_with(".sh") {
                let path = dir.path().join("script.sh");
                std::fs::write(&path, &file.data).unwrap();
                let status = Command::new("bash")
                    .arg("-n")
                    .arg(&path)
                    .stderr(Stdio::inherit())
                    .status()
                    .unwrap();
                assert!(status.success(), "{}", file.path);
            }
        }
    }
}
