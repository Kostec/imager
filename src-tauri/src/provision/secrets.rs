// SPDX-License-Identifier: GPL-2.0-or-later

//! One-way forms of what the user types: a crypt(3) hash for accounts, a WPA2 key for Wi-Fi.
//! Only these reach a profile, a working copy or the card.

use super::ProvisionError;

const PSK_ROUNDS: u32 = 4096;
const PSK_BYTES: usize = 32;
const PSK_HEX_LEN: usize = 64;
const PASSPHRASE_CHARS: std::ops::RangeInclusive<usize> = 8..=63;
const SSID_BYTES: std::ops::RangeInclusive<usize> = 1..=32;

/// SHA-512 crypt (`$6$`), which Bullseye and Jammy both understand. Default rounds, so `chpasswd -e` takes it as is.
pub fn hash_password(plain: &str) -> Result<String, ProvisionError> {
    if plain.is_empty() {
        return Err(ProvisionError::InvalidConfig(
            "the password is empty".to_string(),
        ));
    }
    sha_crypt::sha512_simple(plain, &sha_crypt::Sha512Params::default())
        .map_err(|_| ProvisionError::Hash("SHA-512 crypt failed".to_string()))
}

pub fn is_wifi_psk(value: &str) -> bool {
    value.len() == PSK_HEX_LEN && value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// WPA2-PSK key: 4096 rounds of PBKDF2-HMAC-SHA1 over the passphrase, salted with the SSID. A 64-hex input already is the key.
pub fn derive_wifi_psk(ssid: &str, passphrase: &str) -> Result<String, ProvisionError> {
    if is_wifi_psk(passphrase) {
        return Ok(passphrase.to_string());
    }
    if !SSID_BYTES.contains(&ssid.len()) {
        return Err(ProvisionError::InvalidConfig(
            "the SSID must be 1 to 32 bytes".to_string(),
        ));
    }
    if !PASSPHRASE_CHARS.contains(&passphrase.len())
        || !passphrase.bytes().all(|b| (0x20..0x7f).contains(&b))
    {
        return Err(ProvisionError::InvalidConfig(
            "the Wi-Fi password must be 8 to 63 ASCII characters or 64 hex digits".to_string(),
        ));
    }
    let mut key = [0u8; PSK_BYTES];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(passphrase.as_bytes(), ssid.as_bytes(), PSK_ROUNDS, &mut key);
    Ok(hex::encode(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hashed by glibc crypt(3): `openssl passwd -6 -salt saltstring 'Hello world!'` prints the same.
    const GLIBC_VECTOR: &str = "$6$saltstring$svn8UoSVapNtMuq1ukKS4tPQd8iKwSMHWjl/O817G3uBnIFNjnQJuesI68u4OTLiBFdcbYEdFCoEOfaS35inz1";

    fn salt_of(hash: &str) -> &str {
        hash.split('$').nth(2).unwrap()
    }

    #[test]
    fn password_hash_is_sha512_crypt_and_salted() {
        let first = hash_password("Hello world!").unwrap();
        let second = hash_password("Hello world!").unwrap();
        assert!(first.starts_with("$6$"), "{first}");
        assert_ne!(first, second, "every hash gets its own salt");
        assert_eq!(first.split('$').nth(3).unwrap().len(), 86);
        assert!(sha_crypt::sha512_check("Hello world!", &first).is_ok());
        assert!(sha_crypt::sha512_check("hello world!", &first).is_err());
        assert!(!first.contains("Hello"));
        assert!(!salt_of(&first).is_empty());
    }

    #[test]
    fn the_checker_agrees_with_glibc() {
        assert!(sha_crypt::sha512_check("Hello world!", GLIBC_VECTOR).is_ok());
    }

    // The board checks these with glibc, so ask a third party when one is around.
    #[cfg(unix)]
    #[test]
    fn openssl_reproduces_the_hash() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let hash = hash_password("pa$$ wörd'").unwrap();
        let child = Command::new("openssl")
            .args(["passwd", "-6", "-salt", salt_of(&hash), "-stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn();
        let Ok(mut child) = child else {
            eprintln!("SKIP: no openssl here");
            return;
        };
        child
            .stdin
            .take()
            .unwrap()
            .write_all("pa$$ wörd'\n".as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), hash);
    }

    #[test]
    fn an_empty_password_is_refused() {
        assert!(matches!(
            hash_password(""),
            Err(ProvisionError::InvalidConfig(_))
        ));
    }

    // IEEE 802.11i-2004, annex H.4.
    #[test]
    fn psk_matches_the_ieee_vectors() {
        assert_eq!(
            derive_wifi_psk("IEEE", "password").unwrap(),
            "f42c6fc52df0ebef9ebb4b90b38a5f902e83fe1b135a70e23aed762e9710a12e"
        );
        assert_eq!(
            derive_wifi_psk("ThisIsASSID", "ThisIsAPassword").unwrap(),
            "0dc0d6eb90555ed6419756b9a15ec3e3209b63df707dd508d14581f8982721af"
        );
    }

    #[test]
    fn a_ready_made_key_passes_through() {
        let key = "F42c6fc52df0ebef9ebb4b90b38a5f902e83fe1b135a70e23aed762e9710a12e";
        assert_eq!(derive_wifi_psk("any", key).unwrap(), key);
    }

    #[test]
    fn psk_takes_the_ssid_as_utf8_bytes() {
        // From Python's hashlib.pbkdf2_hmac('sha1', b'12345678', 'Сеть'.encode(), 4096, 32).
        assert_eq!(
            derive_wifi_psk("Сеть", "12345678").unwrap(),
            "70490c94b589a839af77984051fe7280ed0052ad388ceb8dde62652d14d2b736"
        );
    }

    #[test]
    fn a_bad_passphrase_or_ssid_is_refused_without_echoing_it() {
        for (ssid, pass) in [
            ("net", "short12"),
            ("net", &"x".repeat(64)),
            ("net", "кириллица-пароль"),
            ("", "password"),
            (&"s".repeat(33), "password"),
        ] {
            let err = derive_wifi_psk(ssid, pass).unwrap_err().to_string();
            assert!(!err.contains(pass), "{err}");
        }
    }
}
