// SPDX-License-Identifier: GPL-2.0-or-later

//! Turns the settings of a profile into files on the card, plus the device scripts that apply them on first boot.
//! See `interfaces.md`, section 2. This module uses nothing from the rest of the app, so `examples/provision.rs` can include it.

mod bundle;
mod config;
mod image;
mod secrets;

// The app itself does not name every type of the interface.
#[allow(unused_imports)]
pub use bundle::{is_empty, render_bundle, BundleFile};
#[allow(unused_imports)]
pub use config::{AutoconfigConfig, UserShell};
pub use image::{probe_image, provision_image, ImageSupport};
pub use secrets::{derive_wifi_psk, hash_password};

/// Why settings could not be prepared. Messages never carry a value from the profile.
#[derive(Debug)]
pub enum ProvisionError {
    InvalidConfig(String),
    Hash(String),
    Image(String),
}

impl std::fmt::Display for ProvisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(what) => write!(f, "invalid settings: {what}"),
            Self::Hash(what) => write!(f, "hashing failed: {what}"),
            Self::Image(what) => write!(f, "image error: {what}"),
        }
    }
}

impl std::error::Error for ProvisionError {}

impl From<classic_ext4::Ext4Error> for ProvisionError {
    fn from(e: classic_ext4::Ext4Error) -> Self {
        Self::Image(e.to_string())
    }
}
