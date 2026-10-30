// SPDX-License-Identifier: GPL-2.0-or-later

//! What the settings form needs before a flash: hashes, an image check, keys to put on the card.

use std::path::PathBuf;

use crate::decompress::needs_decompression;
use crate::provision::{self, ImageSupport};
use crate::ssh_keys::{self, SshKeySource};

/// A compressed image has to be decompressed before it can be probed (twin in errorUtils.ts).
pub const TAG_COMPRESSED: &str = "[PROBE_COMPRESSED]";

/// The typed password as a `$6$` hash; the plain text stays in the form.
#[tauri::command]
pub fn hash_password(plain: String) -> Result<String, String> {
    provision::hash_password(&plain).map_err(|e| e.to_string())
}

/// The WPA2 key for this network; a 64-hex value already is one.
#[tauri::command]
pub fn derive_wifi_psk(ssid: String, passphrase: String) -> Result<String, String> {
    provision::derive_wifi_psk(&ssid, &passphrase).map_err(|e| e.to_string())
}

/// Whether settings can go onto this raw image. Reads it only.
#[tauri::command]
pub async fn probe_custom_image(path: String) -> Result<ImageSupport, String> {
    let image = PathBuf::from(path);
    if needs_decompression(&image) {
        return Err(TAG_COMPRESSED.to_string());
    }
    tokio::task::spawn_blocking(move || provision::probe_image(&image))
        .await
        .map_err(|e| format!("Image check failed: {e}"))?
        .map_err(|e| e.to_string())
}

/// `authorized_keys` lines from a GitHub/GitLab username or an https link; errors are `[SSH_KEYS_*]` codes only.
#[tauri::command]
pub async fn fetch_ssh_keys(source: SshKeySource, value: String) -> Result<String, String> {
    ssh_keys::fetch_authorized_keys(source, &value).await
}
