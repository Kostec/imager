// SPDX-License-Identifier: GPL-2.0-or-later

//! `catalog.json`, version 1: the file an external catalog source serves. Parsing checks every
//! field and names the failing place (`images[2].sha256`), so a broken file is easy to fix.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use url::Url;

use super::{Catalog, CatalogError};
use crate::images::{ApiBoardSummary, ApiDownloadInfo, ApiImage, ApiVendor};

/// Where a catalog file lives; relative locations in it resolve against this.
#[derive(Debug, Clone)]
pub(super) enum Base {
    /// The URL of `catalog.json` itself
    Url(Url),
    /// The folder holding `catalog.json`
    Folder(PathBuf),
}

/// What one external file adds to the built-in catalog.
#[derive(Debug, Default)]
pub(super) struct Parsed {
    pub vendors: Vec<ApiVendor>,
    pub boards: Vec<ApiBoardSummary>,
    pub images: Vec<ApiImage>,
    /// Things worth telling the user that do not stop the catalog from loading
    pub warnings: Vec<String>,
}

#[derive(Deserialize)]
struct RawCatalog {
    version: u32,
    #[serde(default)]
    vendors: Vec<RawVendor>,
    #[serde(default)]
    boards: Vec<RawBoard>,
    #[serde(default)]
    images: Vec<RawImage>,
}

#[derive(Deserialize)]
struct RawVendor {
    slug: String,
    name: String,
    logo: Option<String>,
    website: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct RawBoard {
    slug: String,
    name: String,
    vendor: String,
    soc: Option<String>,
    architecture: Option<String>,
    summary: Option<String>,
    image: Option<String>,
}

#[derive(Deserialize)]
struct RawImage {
    board: String,
    name: String,
    #[serde(default)]
    release: String,
    #[serde(default)]
    kernel: String,
    url: Option<String>,
    file: Option<String>,
    sha256: Option<String>,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    supports_settings: bool,
}

fn invalid(place: impl Into<String>, reason: impl Into<String>) -> CatalogError {
    CatalogError::Invalid {
        place: place.into(),
        reason: reason.into(),
    }
}

/// A slug names a board or a vendor and ends up in file names, so keep it plain.
fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 64
        && slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !slug.starts_with('.')
}

/// `file` as a path below the catalog folder, or why it is not one. Lexical: no `..`, no root,
/// no drive; both slash kinds count as separators on every platform.
pub(super) fn safe_relative(file: &str) -> Result<PathBuf, &'static str> {
    if file.is_empty() {
        return Err("is empty");
    }
    if file.starts_with(['/', '\\']) || Path::new(file).has_root() {
        return Err("must be relative to the catalog folder");
    }
    let mut path = PathBuf::new();
    for part in file.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return Err("must stay inside the catalog folder"),
            // A drive letter ("C:x") is a prefix on Windows; refuse a colon everywhere.
            part if part.contains(':') => return Err("must be relative to the catalog folder"),
            part => path.push(part),
        }
    }
    if path.as_os_str().is_empty() {
        return Err("is empty");
    }
    Ok(path)
}

fn sha256_of(raw: Option<&str>, place: &str) -> Result<Option<String>, CatalogError> {
    let Some(raw) = raw else { return Ok(None) };
    let hex = raw.trim().to_ascii_lowercase();
    if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(Some(hex))
    } else {
        Err(invalid(place, "must be 64 hexadecimal characters"))
    }
}

/// Resolve a link in the file against the catalog's own location.
fn resolve_url(link: &str, base: &Base, place: &str) -> Result<String, CatalogError> {
    let url = match base {
        Base::Url(base) => base
            .join(link)
            .map_err(|e| invalid(place, format!("not a valid URL: {e}")))?,
        Base::Folder(_) => Url::parse(link).map_err(|_| {
            invalid(
                place,
                "must be an absolute http(s) URL in a folder catalog; use `file` for a local file",
            )
        })?,
    };
    if !matches!(url.scheme(), "http" | "https") {
        return Err(invalid(place, "only http and https URLs are supported"));
    }
    Ok(url.to_string())
}

/// The `file` entry as a `file://` URL, checked to exist and to stay inside the folder.
fn resolve_file(file: &str, dir: &Path, place: &str) -> Result<String, CatalogError> {
    let relative =
        safe_relative(file).map_err(|reason| invalid(place, format!("{file:?} {reason}")))?;
    let missing = || {
        invalid(
            place,
            format!("{file:?} was not found in the catalog folder"),
        )
    };
    let real = dir.join(relative).canonicalize().map_err(|_| missing())?;
    let root = dir
        .canonicalize()
        .map_err(|e| invalid(place, format!("catalog folder is not readable: {e}")))?;
    if !real.starts_with(&root) {
        return Err(invalid(
            place,
            format!("{file:?} must stay inside the catalog folder"),
        ));
    }
    if !real.is_file() {
        return Err(missing());
    }
    Url::from_file_path(&real)
        .map(|url| url.to_string())
        .map_err(|_| invalid(place, format!("{file:?} is not a usable path")))
}

pub(super) fn parse(text: &str, base: &Base, builtin: &Catalog) -> Result<Parsed, CatalogError> {
    let raw: RawCatalog = serde_json::from_str(text).map_err(|e| {
        invalid(
            format!("line {}, column {}", e.line(), e.column()),
            e.to_string(),
        )
    })?;
    if raw.version != 1 {
        return Err(invalid(
            "version",
            format!("expected 1, found {}", raw.version),
        ));
    }

    let mut parsed = Parsed::default();
    let mut vendor_names: Vec<(String, String)> = builtin
        .vendors
        .iter()
        .map(|v| (v.slug.clone(), v.name.clone()))
        .collect();
    let mut board_slugs: HashSet<String> = builtin.boards.iter().map(|b| b.slug.clone()).collect();
    const SLUG_HINT: &str = "use letters, digits, '-', '_' and '.'";

    for (i, vendor) in raw.vendors.into_iter().enumerate() {
        if !valid_slug(&vendor.slug) {
            return Err(invalid(format!("vendors[{i}].slug"), SLUG_HINT));
        }
        if vendor_names.iter().any(|(slug, _)| *slug == vendor.slug) {
            return Err(invalid(
                format!("vendors[{i}].slug"),
                format!("{:?} is already defined", vendor.slug),
            ));
        }
        if vendor.name.trim().is_empty() {
            return Err(invalid(format!("vendors[{i}].name"), "is empty"));
        }
        vendor_names.push((vendor.slug.clone(), vendor.name.clone()));
        parsed.vendors.push(ApiVendor {
            slug: vendor.slug,
            name: vendor.name,
            logo_url: vendor.logo,
            website: vendor.website,
            description: vendor.description,
            board_count: 0,
            partner_tier: None,
        });
    }

    for (i, board) in raw.boards.into_iter().enumerate() {
        if !valid_slug(&board.slug) {
            return Err(invalid(format!("boards[{i}].slug"), SLUG_HINT));
        }
        if !board_slugs.insert(board.slug.clone()) {
            return Err(invalid(
                format!("boards[{i}].slug"),
                format!("{:?} is already defined", board.slug),
            ));
        }
        if board.name.trim().is_empty() {
            return Err(invalid(format!("boards[{i}].name"), "is empty"));
        }
        let Some((_, vendor_name)) = vendor_names.iter().find(|(slug, _)| *slug == board.vendor)
        else {
            return Err(invalid(
                format!("boards[{i}].vendor"),
                format!("unknown vendor {:?}", board.vendor),
            ));
        };
        parsed.boards.push(ApiBoardSummary {
            slug: board.slug,
            name: board.name,
            vendor_slug: board.vendor,
            vendor_name: vendor_name.clone(),
            support_tier: "standard".to_string(),
            image_count: 0,
            has_desktop: false,
            promoted: false,
            image_url: board.image,
            soc: board.soc,
            architecture: board.architecture,
            summary: board.summary,
            qdl: None,
        });
    }

    for (i, image) in raw.images.into_iter().enumerate() {
        if !board_slugs.contains(&image.board) {
            return Err(invalid(
                format!("images[{i}].board"),
                format!("unknown board {:?}", image.board),
            ));
        }
        if image.name.trim().is_empty() {
            return Err(invalid(format!("images[{i}].name"), "is empty"));
        }
        let sha256 = sha256_of(image.sha256.as_deref(), &format!("images[{i}].sha256"))?;
        let location = match (&image.url, &image.file) {
            (Some(_), Some(_)) | (None, None) => {
                return Err(invalid(
                    format!("images[{i}]"),
                    "give exactly one of `url` and `file`",
                ));
            }
            (Some(url), None) => resolve_url(url, base, &format!("images[{i}].url"))?,
            (None, Some(file)) => match base {
                Base::Folder(dir) => resolve_file(file, dir, &format!("images[{i}].file"))?,
                Base::Url(_) => {
                    return Err(invalid(
                        format!("images[{i}].file"),
                        "local files are only available in a folder catalog; use `url`",
                    ));
                }
            },
        };
        if sha256.is_none() {
            parsed.warnings.push(format!(
                "images[{i}] ({}): no sha256, the download will not be verified",
                image.name
            ));
        }
        parsed.images.push(ApiImage {
            id: format!("{}/{i}", image.board),
            board_slug: image.board,
            variant: "minimal".to_string(),
            distribution: image.release,
            release: image.name,
            kernel_branch: String::new(),
            kernel_version: image.kernel,
            application: None,
            promoted: false,
            stability: "stable".to_string(),
            format: "block".to_string(),
            storage: None,
            companions: Vec::new(),
            display_variants: Vec::new(),
            download: ApiDownloadInfo {
                file_url: location.clone(),
                direct_url: location,
                sha_url: sha256.map(|hex| format!("sha256:{hex}")),
                asc_url: None,
                torrent_url: None,
                size_bytes: image.size,
                updated_at: None,
            },
            supports_settings: image.supports_settings,
        });
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builtin() -> Catalog {
        super::super::built_in()
    }

    fn folder_base(dir: &Path) -> Base {
        Base::Folder(dir.to_path_buf())
    }

    fn url_base() -> Base {
        Base::Url(Url::parse("http://catalog.test/dir/catalog.json").unwrap())
    }

    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn place(result: Result<Parsed, CatalogError>) -> (String, String) {
        match result {
            Err(CatalogError::Invalid { place, reason }) => (place, reason),
            Err(other) => panic!("expected Invalid, got {other:?}"),
            Ok(_) => panic!("expected an error, the file was accepted"),
        }
    }

    #[test]
    fn a_full_catalog_maps_to_the_types_the_app_uses() {
        let text = format!(
            r#"{{ "version": 1,
              "vendors": [{{ "slug": "acme", "name": "Acme" }}],
              "boards": [{{ "slug": "acme-one", "name": "Acme One", "vendor": "acme", "soc": "H3" }}],
              "images": [{{ "board": "acme-one", "name": "Bullseye 5.15", "release": "bullseye",
                            "kernel": "5.15.93", "url": "https://cdn.test/a.img.xz",
                            "sha256": "{SHA}", "size": 123, "supports_settings": true }}] }}"#
        );

        let parsed = parse(&text, &url_base(), &builtin()).unwrap();

        assert_eq!(parsed.vendors[0].slug, "acme");
        assert_eq!(parsed.vendors[0].name, "Acme");
        assert_eq!(parsed.boards[0].slug, "acme-one");
        assert_eq!(parsed.boards[0].vendor_slug, "acme");
        assert_eq!(parsed.boards[0].vendor_name, "Acme");
        assert_eq!(parsed.boards[0].soc.as_deref(), Some("H3"));
        let image = &parsed.images[0];
        assert_eq!(image.board_slug, "acme-one");
        assert_eq!(image.download.direct_url, "https://cdn.test/a.img.xz");
        assert_eq!(image.download.size_bytes, 123);
        assert_eq!(
            image.download.sha_url.as_deref(),
            Some(format!("sha256:{SHA}").as_str())
        );
        assert_eq!(image.kernel_version, "5.15.93");
        assert!(image.supports_settings);
        assert!(parsed.warnings.is_empty());
    }

    #[test]
    fn an_image_may_extend_the_built_in_board() {
        let text = r#"{ "version": 1, "images": [{ "board": "wukongpi", "name": "x",
                         "url": "https://cdn.test/w.img", "sha256": null }] }"#;
        let parsed = parse(text, &url_base(), &builtin()).unwrap();
        assert_eq!(parsed.images[0].board_slug, "wukongpi");
    }

    #[test]
    fn a_relative_url_resolves_against_the_catalog_url() {
        let text = r#"{ "version": 1, "images": [{ "board": "wukongpi", "name": "x",
                         "url": "images/w.img.xz" }] }"#;
        let parsed = parse(text, &url_base(), &builtin()).unwrap();
        assert_eq!(
            parsed.images[0].download.direct_url,
            "http://catalog.test/dir/images/w.img.xz"
        );
    }

    #[test]
    fn an_image_without_sha256_loads_with_a_warning() {
        let text = r#"{ "version": 1, "images": [{ "board": "wukongpi", "name": "Plain",
                         "url": "https://cdn.test/w.img" }] }"#;
        let parsed = parse(text, &url_base(), &builtin()).unwrap();
        assert!(parsed.images[0].download.sha_url.is_none());
        assert_eq!(parsed.warnings.len(), 1);
        assert!(
            parsed.warnings[0].contains("images[0]"),
            "{:?}",
            parsed.warnings
        );
        assert!(
            parsed.warnings[0].contains("sha256"),
            "{:?}",
            parsed.warnings
        );
    }

    #[test]
    fn syntax_errors_name_the_line() {
        let (place, _) = place(parse(
            "{\n  \"version\": 1,\n  oops\n}",
            &url_base(),
            &builtin(),
        ));
        assert!(place.contains("line 3"), "{place}");
    }

    #[test]
    fn only_version_one_is_read() {
        let (place, reason) = place(parse(r#"{ "version": 2 }"#, &url_base(), &builtin()));
        assert_eq!(place, "version");
        assert!(reason.contains('2'), "{reason}");
    }

    #[test]
    fn a_bad_sha256_names_the_image() {
        let text = r#"{ "version": 1, "images": [
            { "board": "wukongpi", "name": "ok", "url": "https://cdn.test/a.img" },
            { "board": "wukongpi", "name": "bad", "url": "https://cdn.test/b.img", "sha256": "abc" }] }"#;
        let (place, _) = place(parse(text, &url_base(), &builtin()));
        assert_eq!(place, "images[1].sha256");
    }

    #[test]
    fn an_image_of_an_unknown_board_is_refused() {
        let text = r#"{ "version": 1, "images": [{ "board": "ghost", "name": "x",
                         "url": "https://cdn.test/a.img" }] }"#;
        let (place, reason) = place(parse(text, &url_base(), &builtin()));
        assert_eq!(place, "images[0].board");
        assert!(reason.contains("ghost"), "{reason}");
    }

    #[test]
    fn a_board_of_an_unknown_vendor_is_refused() {
        let text =
            r#"{ "version": 1, "boards": [{ "slug": "b", "name": "B", "vendor": "nobody" }] }"#;
        let (place, _) = place(parse(text, &url_base(), &builtin()));
        assert_eq!(place, "boards[0].vendor");
    }

    #[test]
    fn a_board_cannot_replace_a_built_in_one() {
        let text = r#"{ "version": 1, "boards": [{ "slug": "wukongpi", "name": "B", "vendor": "wukong" }] }"#;
        let (place, _) = place(parse(text, &url_base(), &builtin()));
        assert_eq!(place, "boards[0].slug");
    }

    #[test]
    fn url_and_file_are_mutually_exclusive() {
        let both = r#"{ "version": 1, "images": [{ "board": "wukongpi", "name": "x",
                         "url": "https://cdn.test/a.img", "file": "a.img" }] }"#;
        let (p, _) = place(parse(both, &url_base(), &builtin()));
        assert_eq!(p, "images[0]");
        let neither = r#"{ "version": 1, "images": [{ "board": "wukongpi", "name": "x" }] }"#;
        let (p, _) = place(parse(neither, &url_base(), &builtin()));
        assert_eq!(p, "images[0]");
    }

    #[test]
    fn a_url_catalog_cannot_point_at_local_files() {
        let text = r#"{ "version": 1, "images": [{ "board": "wukongpi", "name": "x", "file": "a.img" }] }"#;
        let (p, _) = place(parse(text, &url_base(), &builtin()));
        assert_eq!(p, "images[0].file");
    }

    #[test]
    fn a_folder_file_becomes_a_file_url_inside_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("w.img"), b"data").unwrap();
        let text = r#"{ "version": 1, "images": [{ "board": "wukongpi", "name": "x", "file": "w.img" }] }"#;

        let parsed = parse(text, &folder_base(dir.path()), &builtin()).unwrap();

        let url = Url::parse(&parsed.images[0].download.direct_url).unwrap();
        assert_eq!(url.scheme(), "file");
        let path = url.to_file_path().unwrap();
        assert_eq!(path.file_name().unwrap(), "w.img");
        assert!(path.starts_with(dir.path().canonicalize().unwrap()));
    }

    #[test]
    fn a_missing_folder_file_is_named() {
        let dir = tempfile::tempdir().unwrap();
        let text = r#"{ "version": 1, "images": [{ "board": "wukongpi", "name": "x", "file": "gone.img" }] }"#;
        let (p, reason) = place(parse(text, &folder_base(dir.path()), &builtin()));
        assert_eq!(p, "images[0].file");
        assert!(reason.contains("gone.img"), "{reason}");
    }

    #[test]
    fn file_cannot_leave_the_catalog_folder() {
        let dir = tempfile::tempdir().unwrap();
        let outside = if cfg!(windows) {
            "C:\\\\Windows\\\\win.ini"
        } else {
            "/etc/passwd"
        };
        for file in [
            "../secret.img",
            "a/../../secret.img",
            "..",
            "..\\\\secret.img",
            outside,
        ] {
            let text = format!(
                r#"{{ "version": 1, "images": [{{ "board": "wukongpi", "name": "x", "file": "{file}" }}] }}"#
            );
            let (p, _) = place(parse(&text, &folder_base(dir.path()), &builtin()));
            assert_eq!(p, "images[0].file", "accepted {file}");
        }
    }

    #[test]
    fn safe_relative_accepts_plain_paths_and_refuses_the_rest() {
        assert_eq!(safe_relative("a.img").unwrap(), PathBuf::from("a.img"));
        assert_eq!(
            safe_relative("sub/a.img").unwrap(),
            PathBuf::from("sub").join("a.img")
        );
        for bad in [
            "",
            "/a.img",
            "\\a.img",
            "../a.img",
            "a/../a.img",
            "C:/a.img",
            "C:a.img",
            "a/./../../b",
        ] {
            assert!(safe_relative(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_folder_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.img"), b"x").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.img"),
            dir.path().join("link.img"),
        )
        .unwrap();
        let text = r#"{ "version": 1, "images": [{ "board": "wukongpi", "name": "x", "file": "link.img" }] }"#;
        let (p, _) = place(parse(text, &folder_base(dir.path()), &builtin()));
        assert_eq!(p, "images[0].file");
    }
}
