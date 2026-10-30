// SPDX-License-Identifier: GPL-2.0-or-later

//! Where boards, vendors and images come from. The built-in catalog is compiled in and
//! always present, so the app works offline; it names the board but ships no image. An
//! external source (a URL or a folder holding `catalog.json`) adds to it and never replaces it.

mod external;

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use serde::Serialize;
use url::Url;

use crate::config;
use crate::images::{ApiBoardSummary, ApiImage, ApiVendor};
use crate::{log_info, log_warn};

const MODULE: &str = "catalog";

/// Settings-store key of the source, shared with the settings page.
pub const SETTING_KEY: &str = "catalog_source";

/// `catalog.json` is a list of names and links; anything bigger is not one.
const MAX_CATALOG_BYTES: u64 = 8 * 1024 * 1024;

/// An external catalog is fetched at most this often; the three queries of one screen share a read.
const CACHE_TTL: Duration = Duration::from_secs(30);

/// Catalog origin, picked by the `catalog_source` setting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogSource {
    BuiltIn,
    Url(String),
    Folder(PathBuf),
}

impl CatalogSource {
    /// The setting's text as a source: empty is the built-in catalog, `http(s)://` a URL,
    /// anything else a folder (or the `catalog.json` file itself).
    pub fn parse(value: &str) -> CatalogSource {
        let value = value.trim();
        let lower = value.to_ascii_lowercase();
        if value.is_empty() {
            CatalogSource::BuiltIn
        } else if lower.starts_with("http://") || lower.starts_with("https://") {
            CatalogSource::Url(value.to_string())
        } else {
            CatalogSource::Folder(PathBuf::from(value))
        }
    }

    /// The text the setting stores; `parse` reads it back.
    pub fn to_setting(&self) -> String {
        match self {
            CatalogSource::BuiltIn => String::new(),
            CatalogSource::Url(url) => url.clone(),
            CatalogSource::Folder(path) => path.display().to_string(),
        }
    }
}

/// Everything one source offers, in the types the rest of the app already consumes.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub vendors: Vec<ApiVendor>,
    pub boards: Vec<ApiBoardSummary>,
    /// Images per board slug
    pub images: HashMap<String, Vec<ApiImage>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    /// The source could not be read at all (no network, no such folder, HTTP error)
    Unreachable { source: String, reason: String },
    /// `catalog.json` was read but is not a valid version 1 catalog; `place` says where
    Invalid { place: String, reason: String },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogError::Unreachable { source, reason } => {
                write!(f, "Cannot read the catalog at {source}: {reason}")
            }
            CatalogError::Invalid { place, reason } => {
                write!(f, "Invalid catalog.json at {place}: {reason}")
            }
        }
    }
}

/// What the "check" button shows: counts when the source loads, the reason when it does not.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogCheck {
    pub ok: bool,
    /// The error text when `ok` is false
    pub message: Option<String>,
    pub vendors: usize,
    pub boards: usize,
    pub images: usize,
    pub warnings: Vec<String>,
}

// ─── Active source ───────────────────────────────────────────────────────────

static SOURCE: Lazy<RwLock<CatalogSource>> = Lazy::new(|| RwLock::new(CatalogSource::BuiltIn));
static CACHE: Lazy<Mutex<Option<(Instant, CatalogSource, Catalog)>>> =
    Lazy::new(|| Mutex::new(None));
static LAST_ERROR: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

/// The active source; the built-in catalog until the `catalog_source` setting says otherwise.
pub fn current_source() -> CatalogSource {
    SOURCE
        .read()
        .map(|source| source.clone())
        .unwrap_or(CatalogSource::BuiltIn)
}

/// Switch the active source and forget what the previous one served.
pub fn set_source(source: CatalogSource) {
    if let Ok(mut current) = SOURCE.write() {
        *current = source;
    }
    if let Ok(mut cache) = CACHE.lock() {
        *cache = None;
    }
    if let Ok(mut error) = LAST_ERROR.lock() {
        *error = None;
    }
}

/// Why the active external source could not be used on the last load, if it could not.
pub fn last_error() -> Option<String> {
    LAST_ERROR.lock().ok().and_then(|error| error.clone())
}

/// The catalog the app shows: the active source on top of the built-in one. A source that fails
/// leaves the built-in catalog in place and its reason in `last_error`.
pub async fn load_active() -> Catalog {
    let source = current_source();
    if source == CatalogSource::BuiltIn {
        return built_in();
    }
    if let Ok(cache) = CACHE.lock() {
        if let Some((at, cached_source, catalog)) = cache.as_ref() {
            if *cached_source == source && at.elapsed() < CACHE_TTL {
                return catalog.clone();
            }
        }
    }
    match load(&source).await {
        Ok(catalog) => {
            if let Ok(mut error) = LAST_ERROR.lock() {
                *error = None;
            }
            if let Ok(mut cache) = CACHE.lock() {
                *cache = Some((Instant::now(), source, catalog.clone()));
            }
            catalog
        }
        Err(e) => {
            log_warn!(MODULE, "{}; using the built-in catalog", e);
            if let Ok(mut error) = LAST_ERROR.lock() {
                *error = Some(e.to_string());
            }
            built_in()
        }
    }
}

/// Read the `catalog_source` setting at startup.
pub fn init_from_store(app: &tauri::AppHandle) {
    use tauri_plugin_store::StoreExt;

    let value = app
        .store(config::app::SETTINGS_STORE)
        .ok()
        .and_then(|store| store.get(SETTING_KEY))
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default();
    let source = CatalogSource::parse(&value);
    if source != CatalogSource::BuiltIn {
        log_info!(MODULE, "External catalog source: {}", source.to_setting());
    }
    set_source(source);
}

/// Persist the source in the settings store and make it the active one.
pub fn save_source(app: &tauri::AppHandle, source: &CatalogSource) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;

    let store = app
        .store(config::app::SETTINGS_STORE)
        .map_err(|e| format!("Failed to open the settings store: {e}"))?;
    if *source == CatalogSource::BuiltIn {
        store.delete(SETTING_KEY);
    } else {
        store.set(SETTING_KEY, source.to_setting());
    }
    store
        .save()
        .map_err(|e| format!("Failed to save the settings store: {e}"))?;
    set_source(source.clone());
    Ok(())
}

// ─── Loading ─────────────────────────────────────────────────────────────────

pub async fn load(source: &CatalogSource) -> Result<Catalog, CatalogError> {
    load_with_warnings(source).await.map(|(catalog, _)| catalog)
}

/// Load a source for the check button: counts or the error, plus anything that deserves a warning.
pub async fn check(source: &CatalogSource) -> CatalogCheck {
    match load_with_warnings(source).await {
        Ok((catalog, warnings)) => CatalogCheck {
            ok: true,
            message: None,
            vendors: catalog.vendors.len(),
            boards: catalog.boards.len(),
            images: catalog.images.values().map(Vec::len).sum(),
            warnings,
        },
        Err(e) => CatalogCheck {
            ok: false,
            message: Some(e.to_string()),
            vendors: 0,
            boards: 0,
            images: 0,
            warnings: Vec::new(),
        },
    }
}

async fn load_with_warnings(
    source: &CatalogSource,
) -> Result<(Catalog, Vec<String>), CatalogError> {
    let builtin = built_in();
    let (text, base) = match source {
        CatalogSource::BuiltIn => return Ok((builtin, Vec::new())),
        CatalogSource::Url(url) => {
            let catalog_url = catalog_url(url)?;
            let text = fetch_text(&catalog_url).await?;
            (text, external::Base::Url(catalog_url))
        }
        CatalogSource::Folder(path) => {
            let (file, dir) = catalog_file(path);
            let text = read_text(&file)?;
            (text, external::Base::Folder(dir))
        }
    };
    let parsed = external::parse(&text, &base, &builtin)?;
    let warnings = parsed.warnings.clone();
    Ok((merge(builtin, parsed), warnings))
}

/// The URL of `catalog.json` for what the user typed: the file itself, or a directory URL.
fn catalog_url(value: &str) -> Result<Url, CatalogError> {
    let invalid = |reason: String| CatalogError::Invalid {
        place: "catalog_source".to_string(),
        reason,
    };
    let url = Url::parse(value.trim()).map_err(|e| invalid(format!("not a valid URL: {e}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(invalid(
            "only http and https URLs are supported".to_string(),
        ));
    }
    if url.path().to_ascii_lowercase().ends_with(".json") {
        return Ok(url);
    }
    let mut dir = url;
    if !dir.path().ends_with('/') {
        let path = format!("{}/", dir.path());
        dir.set_path(&path);
    }
    dir.join("catalog.json")
        .map_err(|e| invalid(format!("not a valid URL: {e}")))
}

/// The `catalog.json` file and its folder for a path that is either of them.
fn catalog_file(path: &Path) -> (PathBuf, PathBuf) {
    if path.is_dir() {
        (path.join("catalog.json"), path.to_path_buf())
    } else {
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        (path.to_path_buf(), dir)
    }
}

fn read_text(file: &Path) -> Result<String, CatalogError> {
    let unreachable = |reason: String| CatalogError::Unreachable {
        source: file.display().to_string(),
        reason,
    };
    let meta = std::fs::metadata(file).map_err(|e| unreachable(e.to_string()))?;
    if meta.len() > MAX_CATALOG_BYTES {
        return Err(unreachable(
            "the file is too large for a catalog".to_string(),
        ));
    }
    std::fs::read_to_string(file).map_err(|e| unreachable(e.to_string()))
}

async fn fetch_text(url: &Url) -> Result<String, CatalogError> {
    let unreachable = |reason: String| CatalogError::Unreachable {
        source: url.to_string(),
        reason,
    };
    let client = reqwest::Client::builder()
        .user_agent(config::app::USER_AGENT)
        .connect_timeout(Duration::from_secs(config::http::SHORT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(config::http::SHORT_TIMEOUT_SECS))
        .build()
        .map_err(|e| unreachable(e.to_string()))?;
    let response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|e| unreachable(e.to_string()))?;
    if !response.status().is_success() {
        return Err(unreachable(format!("HTTP {}", response.status())));
    }
    if response
        .content_length()
        .is_some_and(|len| len > MAX_CATALOG_BYTES)
    {
        return Err(unreachable(
            "the file is too large for a catalog".to_string(),
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| unreachable(e.to_string()))?;
    if bytes.len() as u64 > MAX_CATALOG_BYTES {
        return Err(unreachable(
            "the file is too large for a catalog".to_string(),
        ));
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| CatalogError::Invalid {
        place: "catalog.json".to_string(),
        reason: "the file is not UTF-8 text".to_string(),
    })
}

/// The external entries on top of the built-in ones, with the counts the UI shows recomputed.
fn merge(mut catalog: Catalog, parsed: external::Parsed) -> Catalog {
    catalog.vendors.extend(parsed.vendors);
    catalog.boards.extend(parsed.boards);
    for image in parsed.images {
        catalog
            .images
            .entry(image.board_slug.clone())
            .or_default()
            .push(image);
    }
    for board in &mut catalog.boards {
        board.image_count = catalog.images.get(&board.slug).map_or(0, Vec::len) as u32;
    }
    for vendor in &mut catalog.vendors {
        vendor.board_count = catalog
            .boards
            .iter()
            .filter(|board| board.vendor_slug == vendor.slug)
            .count() as u32;
    }
    catalog
}

/// The URL whose reachability decides online state; None means there is nothing to reach.
pub fn connectivity_probe(source: &CatalogSource) -> Option<&str> {
    match source {
        CatalogSource::Url(url) => Some(url),
        CatalogSource::BuiltIn | CatalogSource::Folder(_) => None,
    }
}

/// Whether `path` is a file inside the folder of the active folder catalog. Only such files may
/// be read as image sources.
pub fn is_in_active_folder(path: &Path) -> bool {
    let CatalogSource::Folder(source) = current_source() else {
        return false;
    };
    let (_, dir) = catalog_file(&source);
    is_inside(path, &dir)
}

fn is_inside(path: &Path, dir: &Path) -> bool {
    match (path.canonicalize(), dir.canonicalize()) {
        (Ok(path), Ok(dir)) => path.starts_with(dir),
        _ => false,
    }
}

// Twin of WUKONG_BOARD in src/config/wukong.ts.
fn built_in() -> Catalog {
    Catalog {
        vendors: vec![ApiVendor {
            slug: "wukong".to_string(),
            name: "Wukong".to_string(),
            logo_url: None,
            website: None,
            description: None,
            board_count: 1,
            partner_tier: None,
        }],
        boards: vec![ApiBoardSummary {
            slug: "wukongpi".to_string(),
            name: "Wukong Pi Zero".to_string(),
            vendor_slug: "wukong".to_string(),
            vendor_name: "Wukong".to_string(),
            support_tier: "standard".to_string(),
            image_count: 0,
            has_desktop: false,
            promoted: false,
            image_url: None,
            soc: Some("H2+".to_string()),
            architecture: Some("armhf".to_string()),
            summary: None,
            qdl: None,
        }],
        images: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// Serves `routes` (path -> body) on a free local port until the test ends.
    async fn serve(routes: Vec<(&'static str, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let routes = routes.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]).to_string();
                    let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
                    let response = match routes.iter().find(|(p, _)| *p == path) {
                        Some((_, body)) => format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        ),
                        None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_string(),
                    };
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        format!("http://{addr}")
    }

    fn two_vendor_catalog() -> String {
        format!(
            r#"{{ "version": 1,
              "vendors": [{{ "slug": "acme", "name": "Acme" }}],
              "boards": [{{ "slug": "acme-one", "name": "Acme One", "vendor": "acme" }}],
              "images": [{{ "board": "acme-one", "name": "Img", "url": "img.img.xz", "sha256": "{SHA}" }}] }}"#
        )
    }

    #[tokio::test]
    async fn built_in_catalog_is_the_wukong_board_without_images() {
        let catalog = load(&CatalogSource::BuiltIn).await.unwrap();

        assert_eq!(catalog.vendors.len(), 1);
        assert_eq!(catalog.vendors[0].slug, "wukong");
        assert_eq!(catalog.vendors[0].name, "Wukong");
        assert_eq!(catalog.vendors[0].board_count, 1);

        assert_eq!(catalog.boards.len(), 1);
        let board = &catalog.boards[0];
        assert_eq!(board.slug, "wukongpi");
        assert_eq!(board.name, "Wukong Pi Zero");
        assert_eq!(board.vendor_slug, "wukong");
        assert_eq!(board.soc.as_deref(), Some("H2+"));
        assert!(board.qdl.is_none());

        assert!(catalog.images.values().all(Vec::is_empty));
    }

    #[tokio::test]
    async fn a_url_catalog_adds_vendors_boards_and_images_to_the_built_in_one() {
        let base = serve(vec![("/catalog.json", two_vendor_catalog())]).await;

        let catalog = load(&CatalogSource::Url(base.clone())).await.unwrap();

        let vendors: Vec<_> = catalog.vendors.iter().map(|v| v.slug.as_str()).collect();
        assert_eq!(vendors, ["wukong", "acme"]);
        assert_eq!(catalog.boards.len(), 2);
        assert_eq!(catalog.vendors[1].board_count, 1);
        let images = &catalog.images["acme-one"];
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].download.direct_url, format!("{base}/img.img.xz"));
        assert_eq!(catalog.boards[1].image_count, 1);
    }

    #[tokio::test]
    async fn a_url_pointing_at_the_json_file_is_used_as_is() {
        let base = serve(vec![("/cat/list.json", two_vendor_catalog())]).await;

        let catalog = load(&CatalogSource::Url(format!("{base}/cat/list.json")))
            .await
            .unwrap();

        assert_eq!(catalog.boards.len(), 2);
    }

    #[tokio::test]
    async fn a_missing_url_catalog_is_unreachable_with_the_status() {
        let base = serve(vec![]).await;

        let err = load(&CatalogSource::Url(base)).await.unwrap_err();

        match err {
            CatalogError::Unreachable { reason, .. } => assert!(reason.contains("404"), "{reason}"),
            other => panic!("expected Unreachable, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn nothing_listening_is_unreachable() {
        // Port 9 (discard) is not served on the test host.
        let err = load(&CatalogSource::Url("http://127.0.0.1:9".into()))
            .await
            .unwrap_err();

        assert!(matches!(err, CatalogError::Unreachable { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn a_broken_url_catalog_reports_the_place() {
        let base = serve(vec![(
            "/catalog.json",
            r#"{ "version": 1, "images": [{ "board": "x" }] }"#.to_string(),
        )])
        .await;

        let err = load(&CatalogSource::Url(base)).await.unwrap_err();

        assert!(matches!(err, CatalogError::Invalid { .. }), "{err:?}");
        assert!(err.to_string().contains("catalog.json"), "{err}");
    }

    #[tokio::test]
    async fn a_folder_catalog_loads_from_the_folder_or_the_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("w.img"), b"data").unwrap();
        let text = r#"{ "version": 1, "images": [{ "board": "wukongpi", "name": "Local", "file": "w.img" }] }"#;
        std::fs::write(dir.path().join("catalog.json"), text).unwrap();

        for source in [dir.path().to_path_buf(), dir.path().join("catalog.json")] {
            let catalog = load(&CatalogSource::Folder(source)).await.unwrap();
            assert_eq!(catalog.images["wukongpi"].len(), 1);
            assert_eq!(catalog.boards[0].image_count, 1);
        }
    }

    #[tokio::test]
    async fn a_missing_folder_is_unreachable() {
        let dir = tempfile::tempdir().unwrap();

        let err = load(&CatalogSource::Folder(dir.path().join("nope")))
            .await
            .unwrap_err();

        assert!(matches!(err, CatalogError::Unreachable { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn check_reports_counts_warnings_and_errors() {
        let dir = tempfile::tempdir().unwrap();
        let text = r#"{ "version": 1, "images": [{ "board": "wukongpi", "name": "x", "url": "https://cdn.test/a.img" }] }"#;
        std::fs::write(dir.path().join("catalog.json"), text).unwrap();

        let good = check(&CatalogSource::Folder(dir.path().to_path_buf())).await;
        assert!(good.ok);
        assert_eq!((good.vendors, good.boards, good.images), (1, 1, 1));
        assert_eq!(good.warnings.len(), 1);

        std::fs::write(dir.path().join("catalog.json"), "not json").unwrap();
        let bad = check(&CatalogSource::Folder(dir.path().to_path_buf())).await;
        assert!(!bad.ok);
        assert!(bad.message.unwrap().contains("catalog.json"));
    }

    #[tokio::test]
    async fn the_documented_example_catalogs_are_valid() {
        let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/catalog");

        let by_url = external::parse(
            &std::fs::read_to_string(docs.join("catalog.json")).unwrap(),
            &external::Base::Url(Url::parse("http://localhost:8080/catalog.json").unwrap()),
            &built_in(),
        )
        .unwrap();
        assert!(by_url.warnings.is_empty(), "{:?}", by_url.warnings);
        assert_eq!(
            by_url.images[0].download.direct_url,
            "http://localhost:8080/images/demo.img"
        );

        let by_folder = load(&CatalogSource::Folder(docs.join("folder")))
            .await
            .unwrap();
        assert_eq!(by_folder.images["example-one"].len(), 1);

        // The listed checksum is that of the shipped file.
        let bytes = std::fs::read(docs.join("images/demo.img")).unwrap();
        let expected = by_url.images[0].download.sha_url.clone().unwrap();
        let actual = format!("sha256:{:x}", sha2::Sha256::digest(&bytes));
        assert_eq!(expected, actual);
    }

    #[test]
    fn the_setting_text_maps_to_a_source_and_back() {
        assert_eq!(CatalogSource::parse(""), CatalogSource::BuiltIn);
        assert_eq!(CatalogSource::parse("   "), CatalogSource::BuiltIn);
        assert_eq!(
            CatalogSource::parse(" https://c.example/catalog.json "),
            CatalogSource::Url("https://c.example/catalog.json".into())
        );
        assert_eq!(
            CatalogSource::parse("HTTP://localhost:8080"),
            CatalogSource::Url("HTTP://localhost:8080".into())
        );
        let folder = CatalogSource::parse("D:\\catalog");
        assert_eq!(folder, CatalogSource::Folder(PathBuf::from("D:\\catalog")));
        assert_eq!(CatalogSource::parse(&folder.to_setting()), folder);
        assert_eq!(CatalogSource::BuiltIn.to_setting(), "");
    }

    #[test]
    fn only_files_below_the_folder_count_as_inside_it() {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.img"), b"x").unwrap();
        std::fs::write(other.path().join("b.img"), b"x").unwrap();

        assert!(is_inside(&dir.path().join("a.img"), dir.path()));
        assert!(!is_inside(&other.path().join("b.img"), dir.path()));
        assert!(!is_inside(&dir.path().join("..").join("x"), dir.path()));
        assert!(!is_inside(&dir.path().join("missing.img"), dir.path()));
    }

    #[test]
    fn the_default_source_is_built_in() {
        assert_eq!(current_source(), CatalogSource::BuiltIn);
    }

    #[test]
    fn only_a_url_catalog_needs_a_connectivity_check() {
        assert_eq!(connectivity_probe(&CatalogSource::BuiltIn), None);
        assert_eq!(
            connectivity_probe(&CatalogSource::Folder(PathBuf::from("catalog"))),
            None
        );
        let url = "https://catalog.example.com/catalog.json";
        assert_eq!(
            connectivity_probe(&CatalogSource::Url(url.into())),
            Some(url)
        );
    }
}
