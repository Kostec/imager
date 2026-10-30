// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

//! The settings of one flash. Mirrors the TS `AutoconfigConfig` type; every field is optional and only what is set gets applied.

use serde::Deserialize;

/// Login shell choices offered for the first user.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserShell {
    Bash,
    Zsh,
}

impl UserShell {
    /// Value written to `WK_USER_SHELL`.
    pub fn as_str(&self) -> &'static str {
        match self {
            UserShell::Bash => "bash",
            UserShell::Zsh => "zsh",
        }
    }
}

/// Holds no secret in the clear: `wifi_psk` is the derived 64-hex key, the password fields are `$6$` hashes.
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoconfigConfig {
    /// Kept for profiles; Ethernet stays up whatever Wi-Fi does, so nothing is written for it.
    pub ethernet_enabled: Option<bool>,
    pub wifi_enabled: Option<bool>,
    pub wifi_ssid: Option<String>,
    pub wifi_psk: Option<String>,
    pub wifi_country_code: Option<String>,
    pub wifi_hidden: Option<bool>,
    pub use_static_ip: Option<bool>,
    pub static_ip: Option<String>,
    pub static_mask: Option<String>,
    pub static_gateway: Option<String>,
    pub static_dns: Option<String>,

    pub hostname: Option<String>,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    pub keyboard_layout: Option<String>,

    pub root_password_hash: Option<String>,

    pub user_name: Option<String>,
    pub user_password_hash: Option<String>,
    pub user_shell: Option<UserShell>,
    pub user_real_name: Option<String>,

    pub ssh_enabled: Option<bool>,
    pub ssh_password_auth: Option<bool>,
    pub ssh_root_login: Option<bool>,
    pub ssh_authorized_keys: Option<String>,

    pub overlays: Option<Vec<String>>,
    pub user_script: Option<String>,
}

// Hashes, the Wi-Fi key and even names must never reach a log: only show which fields are set.
impl std::fmt::Debug for AutoconfigConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let set = |value: &Option<String>| value.as_deref().is_some_and(|v| !v.is_empty());
        f.debug_struct("AutoconfigConfig")
            .field("ethernet_enabled", &self.ethernet_enabled)
            .field("wifi_psk_set", &set(&self.wifi_psk))
            .field("root_password_hash_set", &set(&self.root_password_hash))
            .field("user_password_hash_set", &set(&self.user_password_hash))
            .field("ssh_authorized_keys_set", &set(&self.ssh_authorized_keys))
            .field("user_script_set", &set(&self.user_script))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_follow_the_camel_case_of_the_ts_type() {
        let config: AutoconfigConfig = serde_json::from_str(
            r#"{ "wifiSsid": "net", "wifiPsk": "k", "rootPasswordHash": "r", "userPasswordHash": "u",
                 "sshAuthorizedKeys": "a", "keyboardLayout": "ru", "wifiHidden": true,
                 "sshEnabled": false, "sshPasswordAuth": true, "sshRootLogin": false,
                 "overlays": ["i2c0"], "userScript": "s", "hostname": "h", "userShell": "zsh" }"#,
        )
        .unwrap();
        assert_eq!(config.wifi_psk.as_deref(), Some("k"));
        assert_eq!(config.root_password_hash.as_deref(), Some("r"));
        assert_eq!(config.ssh_authorized_keys.as_deref(), Some("a"));
        assert_eq!(config.keyboard_layout.as_deref(), Some("ru"));
        assert_eq!(config.ssh_enabled, Some(false));
        assert_eq!(config.overlays.as_deref(), Some(&["i2c0".to_string()][..]));
    }

    #[test]
    fn a_profile_from_before_the_rename_still_loads() {
        let config: AutoconfigConfig =
            serde_json::from_str(r#"{ "wifiKey": "old", "applyNetwork": true }"#).unwrap();
        assert!(config.wifi_psk.is_none());
    }

    #[test]
    fn debug_output_never_shows_values() {
        let secret = |what: &str| Some(format!("SECRET-{what}"));
        let config = AutoconfigConfig {
            wifi_ssid: secret("SSID"),
            wifi_psk: secret("PSK"),
            root_password_hash: secret("ROOT"),
            user_password_hash: secret("USER"),
            user_name: secret("NAME"),
            hostname: secret("HOST"),
            ssh_authorized_keys: secret("KEYS"),
            user_script: secret("SCRIPT"),
            ..AutoconfigConfig::default()
        };
        let shown = format!("{config:?}");
        assert!(!shown.contains("SECRET"), "{shown}");
        assert!(shown.contains("wifi_psk_set: true"), "{shown}");
    }
}
