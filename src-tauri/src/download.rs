// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2025-2026 Daniele Briguglio, superkali@armbian.com

//! Downloading Armbian images from the web.

use futures_util::StreamExt;
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config;
use crate::decompress::decompress_with_rust_xz;
use crate::utils::{
    bytes_to_mb, discard_if_empty, tagged, validate_cache_path, validate_file_name,
    ProgressTracker, TAG_CANCELLED,
};
use crate::{log_debug, log_error, log_info, log_warn};

const MODULE: &str = "download";

pub(crate) const TAG_SHA_MISMATCH: &str = "[SHA_MISMATCH]";
const SHA_CANCELLED: &str = "SHA256 verification cancelled";

pub(crate) fn sha_mismatch_err(expected: &str, actual: impl std::fmt::Display) -> String {
    tagged(
        TAG_SHA_MISMATCH,
        format!("SHA256 mismatch: expected {}, got {}", expected, actual),
    )
}

// A mismatch keeps its tag at the start; other SHA failures keep the legacy wording.
fn sha_failure_err(e: &str) -> String {
    if e.starts_with(TAG_SHA_MISMATCH) {
        e.to_string()
    } else {
        format!("SHA256 verification failed: {}", e)
    }
}

/// Download progress state
pub struct DownloadState {
    pub total_bytes: AtomicU64,
    pub downloaded_bytes: AtomicU64,
    pub is_verifying_sha: AtomicBool,
    pub is_decompressing: AtomicBool,
    pub is_cancelled: AtomicBool,
    pub error: Mutex<Option<String>>,
    pub output_path: Mutex<Option<PathBuf>>,
    /// Temp file kept when SHA is unavailable, so the user can decide to proceed.
    pub temp_path: Mutex<Option<PathBuf>>,
}

impl DownloadState {
    pub fn new() -> Self {
        Self {
            total_bytes: AtomicU64::new(0),
            downloaded_bytes: AtomicU64::new(0),
            is_verifying_sha: AtomicBool::new(false),
            is_decompressing: AtomicBool::new(false),
            is_cancelled: AtomicBool::new(false),
            error: Mutex::new(None),
            output_path: Mutex::new(None),
            temp_path: Mutex::new(None),
        }
    }

    pub fn reset(&self) {
        self.total_bytes.store(0, Ordering::SeqCst);
        self.downloaded_bytes.store(0, Ordering::SeqCst);
        self.is_verifying_sha.store(false, Ordering::SeqCst);
        self.is_decompressing.store(false, Ordering::SeqCst);
        self.is_cancelled.store(false, Ordering::SeqCst);
    }
}

impl Default for DownloadState {
    fn default() -> Self {
        Self::new()
    }
}

fn cache_file_name(name: &str) -> Result<&str, String> {
    validate_file_name(name).inspect_err(|e| log_warn!(MODULE, "{}", e))
}

// Re-checked after stripping: "...xz" is a valid name whose stem is "..".
fn decompressed_name(filename: &str) -> Result<&str, String> {
    cache_file_name(filename.trim_end_matches(".xz"))
}

/// The file behind a `file://` URL: a catalog image read from the catalog folder.
fn local_source(url: &str) -> Option<Result<PathBuf, String>> {
    if !url.starts_with("file:") {
        return None;
    }
    Some(
        reqwest::Url::parse(url)
            .ok()
            .and_then(|parsed| parsed.to_file_path().ok())
            .ok_or_else(|| format!("Invalid file URL: {url}")),
    )
}

/// Extract filename from URL
fn extract_filename(url: &str) -> Result<&str, String> {
    log_debug!(MODULE, "Extracting filename from URL: {}", url);
    let url_path = url.split(['?', '#']).next().unwrap_or(url);
    let filename = url_path
        .split('/')
        .next_back()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Invalid URL: no filename".to_string())?;
    log_debug!(MODULE, "Extracted filename: {}", filename);
    cache_file_name(filename)
}

/// A catalog that lists `sha256` values hands them over as `sha256:<hex>` in place of a checksum URL.
pub(crate) const INLINE_SHA_PREFIX: &str = "sha256:";

fn inline_sha(hex: &str) -> Result<String, String> {
    let hex = hex.trim().to_ascii_lowercase();
    if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(hex)
    } else {
        Err(format!(
            "[SHA_UNAVAILABLE] Invalid SHA256 hash format: {hex}"
        ))
    }
}

/// Fetch the expected SHA256. Errors are prefixed [SHA_UNAVAILABLE] to
/// distinguish a fetch failure from a genuine mismatch.
async fn fetch_expected_sha(client: &Client, sha_url: &str) -> Result<String, String> {
    if let Some(inline) = sha_url.strip_prefix(INLINE_SHA_PREFIX) {
        return inline_sha(inline);
    }
    log_debug!(MODULE, "Fetching SHA256 from: {}", sha_url);

    let response = client
        .get(sha_url)
        .send()
        .await
        .map_err(|e| format!("[SHA_UNAVAILABLE] Failed to fetch SHA: {}", e))?;

    // Post-redirect URL reveals which mirror served the SHA.
    let final_sha_url = response.url().to_string();
    if final_sha_url != sha_url {
        log_debug!(MODULE, "SHA redirected to mirror: {}", final_sha_url);
    }

    if !response.status().is_success() {
        return Err(format!(
            "[SHA_UNAVAILABLE] SHA fetch failed with status: {}",
            response.status()
        ));
    }

    let content = response
        .text()
        .await
        .map_err(|e| format!("[SHA_UNAVAILABLE] Failed to read SHA response: {}", e))?;

    // SHA file format is "hash *filename" or "hash  filename".
    let hash = content
        .split_whitespace()
        .next()
        .ok_or("[SHA_UNAVAILABLE] Invalid SHA file format")?
        .to_lowercase();

    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "[SHA_UNAVAILABLE] Invalid SHA256 hash format: {}",
            hash
        ));
    }

    log_debug!(MODULE, "Expected SHA256: {}", hash);
    Ok(hash)
}

/// Calculate SHA256 of a file
fn calculate_file_sha256(path: &Path, state: &Arc<DownloadState>) -> Result<String, String> {
    log_debug!(MODULE, "Calculating SHA256 of: {}", path.display());
    log_debug!(
        MODULE,
        "File size: {:?} bytes",
        path.metadata().ok().map(|m| m.len())
    );

    let mut file = File::open(path).map_err(|e| format!("Failed to open file for SHA: {}", e))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; config::logging::SHA_BUFFER_SIZE];
    let mut bytes_processed = 0u64;

    loop {
        if state.is_cancelled.load(Ordering::SeqCst) {
            log_info!(MODULE, "SHA256 calculation cancelled by user");
            return Err(tagged(TAG_CANCELLED, SHA_CANCELLED));
        }

        let bytes_read = file
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read file for SHA: {}", e))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
        bytes_processed += bytes_read as u64;

        if bytes_processed % (10 * 1024 * 1024) == 0 {
            log_debug!(
                MODULE,
                "SHA256 calculation progress: {} MB",
                bytes_processed / (1024 * 1024)
            );
        }
    }

    let result = hasher.finalize();
    let hash = format!("{:x}", result);
    log_debug!(MODULE, "Calculated SHA256: {}", hash);
    Ok(hash)
}

/// Verify file SHA256 against expected value
async fn verify_sha256(
    client: &Client,
    file_path: &Path,
    sha_url: &str,
    state: &Arc<DownloadState>,
) -> Result<(), String> {
    if state.is_cancelled.load(Ordering::SeqCst) {
        return Err(tagged(TAG_CANCELLED, SHA_CANCELLED));
    }

    let expected = fetch_expected_sha(client, sha_url).await?;

    if state.is_cancelled.load(Ordering::SeqCst) {
        return Err(tagged(TAG_CANCELLED, SHA_CANCELLED));
    }

    let actual = calculate_file_sha256(file_path, state)?;

    if expected == actual {
        log_info!(MODULE, "SHA256 verification PASSED");
        Ok(())
    } else {
        log_error!(
            MODULE,
            "SHA256 verification FAILED! Expected: {}, Got: {}",
            expected,
            actual
        );
        Err(sha_mismatch_err(&expected, actual))
    }
}

/// Stream `url` into `temp_path`, removing the partial file on any failure.
async fn fetch_remote(
    client: &Client,
    url: &str,
    temp_path: &Path,
    state: &Arc<DownloadState>,
) -> Result<(), String> {
    log_info!(MODULE, "Starting download...");
    let response = client.get(url).send().await.map_err(|e| {
        log_error!(MODULE, "Failed to start download: {}", e);
        format!("Failed to start download: {}", e)
    })?;

    // Post-redirect URL reveals which mirror is being used.
    let final_url = response.url().to_string();
    if final_url != url {
        log_debug!(MODULE, "Redirected to mirror: {}", final_url);
    }

    if !response.status().is_success() {
        log_error!(MODULE, "Download failed with status: {}", response.status());
        return Err(format!(
            "Download failed with status: {}",
            response.status()
        ));
    }

    let total_size = response.content_length().unwrap_or(0);
    state.total_bytes.store(total_size, Ordering::SeqCst);

    log_info!(
        MODULE,
        "Download size: {} bytes ({:.2} MB)",
        total_size,
        bytes_to_mb(total_size)
    );

    let mut temp_file =
        File::create(temp_path).map_err(|e| format!("Failed to create temp file: {}", e))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut tracker = ProgressTracker::new(
        "Download",
        MODULE,
        total_size,
        config::logging::DOWNLOAD_LOG_INTERVAL_MB,
    );

    #[cfg(debug_assertions)]
    let throttle = crate::dev_scenarios::network::download_throttle();

    while let Some(chunk) = stream.next().await {
        if state.is_cancelled.load(Ordering::SeqCst) {
            log_info!(MODULE, "Download cancelled by user");
            drop(temp_file);
            let _ = std::fs::remove_file(temp_path);
            return Err("Download cancelled".to_string());
        }

        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                drop(temp_file);
                let _ = std::fs::remove_file(temp_path);
                return Err(format!("Download error: {}", e));
            }
        };
        if let Err(e) = temp_file.write_all(&chunk) {
            drop(temp_file);
            let _ = std::fs::remove_file(temp_path);
            return Err(format!("Failed to write chunk: {}", e));
        }

        downloaded += chunk.len() as u64;
        state.downloaded_bytes.store(downloaded, Ordering::SeqCst);
        tracker.update(chunk.len() as u64);

        #[cfg(debug_assertions)]
        throttle.pace(chunk.len()).await;
    }

    drop(temp_file);
    tracker.finish();
    Ok(())
}

/// Copy a catalog image from the catalog folder into `temp_path`, so it goes through the same
/// checksum and decompression steps as a download.
async fn copy_local(
    source: &Path,
    temp_path: &Path,
    state: &Arc<DownloadState>,
) -> Result<(), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    log_info!(MODULE, "Copying catalog image: {}", source.display());
    let mut input = tokio::fs::File::open(source)
        .await
        .map_err(|e| format!("Failed to open {}: {}", source.display(), e))?;
    let total = input.metadata().await.map(|m| m.len()).unwrap_or(0);
    state.total_bytes.store(total, Ordering::SeqCst);

    let mut output = tokio::fs::File::create(temp_path)
        .await
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut copied = 0u64;
    loop {
        if state.is_cancelled.load(Ordering::SeqCst) {
            drop(output);
            let _ = std::fs::remove_file(temp_path);
            return Err("Download cancelled".to_string());
        }
        let read = match input.read(&mut buffer).await {
            Ok(n) => n,
            Err(e) => {
                drop(output);
                let _ = std::fs::remove_file(temp_path);
                return Err(format!("Failed to read {}: {}", source.display(), e));
            }
        };
        if read == 0 {
            break;
        }
        if let Err(e) = output.write_all(&buffer[..read]).await {
            drop(output);
            let _ = std::fs::remove_file(temp_path);
            return Err(format!("Failed to write chunk: {}", e));
        }
        copied += read as u64;
        state.downloaded_bytes.store(copied, Ordering::SeqCst);
    }
    output
        .flush()
        .await
        .map_err(|e| format!("Failed to write temp file: {}", e))?;
    Ok(())
}

/// Download and decompress an Armbian image; when sha_url is given, verifies the compressed file first.
pub async fn download_image(
    url: &str,
    sha_url: Option<&str>,
    output_dir: &PathBuf,
    state: Arc<DownloadState>,
) -> Result<PathBuf, String> {
    state.reset();
    // Clear any stale temp_path left by a previous failed download.
    *state.temp_path.lock().await = None;

    let local = match local_source(url) {
        Some(path) => {
            let path = path?;
            if !crate::catalog::is_in_active_folder(&path) {
                return Err(format!(
                    "Refused a file outside the catalog folder: {}",
                    path.display()
                ));
            }
            Some(path)
        }
        None => None,
    };
    let local_name = local
        .as_ref()
        .map(|path| path.file_name().and_then(|n| n.to_str()).unwrap_or(""));
    let filename = match local_name {
        Some(name) => cache_file_name(name)?,
        None => extract_filename(url)?,
    };

    let output_filename = decompressed_name(filename)?;
    let output_path = output_dir.join(output_filename);

    log_info!(MODULE, "Download requested: {}", url);
    log_debug!(MODULE, "Output path: {}", output_path.display());

    if let Some(cached_path) = crate::cache::get_cached_image(output_filename) {
        log_info!(MODULE, "Using cached image: {}", cached_path.display());
        *state.output_path.lock().await = Some(cached_path.clone());
        return Ok(cached_path);
    }

    #[cfg(debug_assertions)]
    crate::dev_scenarios::network::check_download_start()?;

    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    let client = Client::builder()
        .user_agent(config::app::USER_AGENT)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let temp_path = output_dir.join(format!("{}{}", filename, config::images::DOWNLOAD_SUFFIX));
    match &local {
        Some(source) => copy_local(source, &temp_path, &state).await?,
        None => fetch_remote(&client, url, &temp_path, &state).await?,
    }

    if let Some(sha_url) = sha_url {
        state.is_verifying_sha.store(true, Ordering::SeqCst);
        log_info!(MODULE, "Verifying SHA256...");
        let verified = verify_sha256(&client, &temp_path, sha_url, &state).await;
        #[cfg(debug_assertions)]
        let verified = crate::dev_scenarios::network::sha_result(verified);
        match verified {
            Ok(()) => {
                log_info!(MODULE, "SHA256 verification successful");
            }
            Err(e) => {
                log_error!(MODULE, "SHA256 verification failed: {}", e);
                state.is_verifying_sha.store(false, Ordering::SeqCst);

                if state.is_cancelled.load(Ordering::SeqCst) {
                    let _ = std::fs::remove_file(&temp_path);
                    return Err("Download cancelled".to_string());
                }

                // SHA unavailable: keep the file so the user can choose to proceed.
                if e.contains("[SHA_UNAVAILABLE]") {
                    log_info!(
                        MODULE,
                        "SHA unavailable, keeping temp file for user decision: {}",
                        temp_path.display()
                    );
                    *state.temp_path.lock().await = Some(temp_path.clone());
                    return Err(format!("SHA256 verification failed: {}", e));
                }

                // Genuine mismatch means a corrupted image: delete it.
                let _ = std::fs::remove_file(&temp_path);
                return Err(sha_failure_err(&e));
            }
        }
        state.is_verifying_sha.store(false, Ordering::SeqCst);
    } else {
        log_warn!(MODULE, "No SHA URL provided, skipping verification");
    }

    if filename.ends_with(".xz") {
        state.is_decompressing.store(true, Ordering::SeqCst);
        log_info!(
            MODULE,
            "Starting decompression with Rust lzma-rust2 (multi-threaded)..."
        );

        if let Err(e) = decompress_with_rust_xz(&temp_path, &output_path, &state) {
            let _ = std::fs::remove_file(&temp_path);
            let _ = std::fs::remove_file(&output_path);
            return Err(e);
        }
        log_info!(MODULE, "Decompression complete");

        let _ = std::fs::remove_file(&temp_path);
    } else {
        if let Err(e) = std::fs::rename(&temp_path, &output_path) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(format!("Failed to move file: {}", e));
        }
    }

    discard_if_empty(&output_path)?;
    log_info!(MODULE, "Image ready: {}", output_path.display());
    *state.output_path.lock().await = Some(output_path.clone());
    Ok(output_path)
}

/// Finish a download without SHA verification, reusing the already-downloaded
/// temp file. Called when the user proceeds after a SHA-unavailable error.
pub async fn continue_without_sha(
    state: Arc<DownloadState>,
    output_dir: &Path,
) -> Result<PathBuf, String> {
    let temp_path = state
        .temp_path
        .lock()
        .await
        .take()
        .ok_or("No pending download to continue")?;

    // Defense in depth: ensure temp_path is still inside the cache directory.
    if let Err(e) = validate_cache_path(&temp_path) {
        log_error!(
            MODULE,
            "Security: temp_path {} is outside cache directory: {}",
            temp_path.display(),
            e
        );
        return Err("Invalid temp file location".to_string());
    }

    log_info!(
        MODULE,
        "Continuing without SHA verification: {}",
        temp_path.display()
    );

    let filename = temp_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid temp path")?;

    // temp_path is "<name>.downloading"; strip that, then the .xz if present.
    let original_filename =
        cache_file_name(filename.trim_end_matches(config::images::DOWNLOAD_SUFFIX))?;
    let output_filename = decompressed_name(original_filename)?;
    let output_path = output_dir.join(output_filename);

    log_info!(MODULE, "Output path: {}", output_path.display());

    if original_filename.ends_with(".xz") {
        state.is_decompressing.store(true, Ordering::SeqCst);
        log_info!(
            MODULE,
            "Starting decompression with Rust lzma-rust2 (multi-threaded)..."
        );

        if let Err(e) = decompress_with_rust_xz(&temp_path, &output_path, &state) {
            let _ = std::fs::remove_file(&temp_path);
            let _ = std::fs::remove_file(&output_path);
            return Err(e);
        }

        state.is_decompressing.store(false, Ordering::SeqCst);
        log_info!(MODULE, "Decompression complete");

        let _ = std::fs::remove_file(&temp_path);
    } else {
        if let Err(e) = std::fs::rename(&temp_path, &output_path) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(format!("Failed to move file: {}", e));
        }
    }

    discard_if_empty(&output_path)?;
    log_info!(MODULE, "Image ready: {}", output_path.display());
    *state.output_path.lock().await = Some(output_path.clone());
    Ok(output_path)
}

/// Remove the temp file left by a failed download (user cancelled after SHA-unavailable).
pub async fn cleanup_pending_download(state: Arc<DownloadState>) {
    if let Some(temp_path) = state.temp_path.lock().await.take() {
        log_info!(
            MODULE,
            "Cleaning up pending download: {}",
            temp_path.display()
        );
        let _ = std::fs::remove_file(&temp_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::TAG_INVALID_FILE_NAME;

    fn refused(r: Result<&str, String>) -> bool {
        r.is_err_and(|e| e.starts_with(TAG_INVALID_FILE_NAME))
    }

    #[test]
    fn filename_is_the_last_url_segment() {
        assert_eq!(
            extract_filename("https://images.example.com/a/b/x.img.xz?mirror=1"),
            Ok("x.img.xz")
        );
        assert_eq!(
            extract_filename("https://images.example.com/a/x%5C..img.xz"),
            Ok("x%5C..img.xz")
        );
        assert_eq!(
            extract_filename("https://images.example.com/a/x.img.xz#..\\y"),
            Ok("x.img.xz")
        );
        assert_eq!(
            extract_filename("https://images.example.com/a/x.img.xz#frag?q=1"),
            Ok("x.img.xz")
        );
        assert!(extract_filename("https://images.example.com/a/").is_err());
    }

    #[test]
    fn filename_with_windows_separators_is_refused() {
        assert!(refused(extract_filename(
            "https://images.example.com/a/..\\..\\x.img.xz"
        )));
        assert!(refused(extract_filename(
            "https://images.example.com/a/C:\\x.img.xz"
        )));
        assert!(refused(extract_filename("https://images.example.com/a/..")));
        assert!(refused(extract_filename(
            "https://images.example.com/a/x\0.img"
        )));
    }

    #[test]
    fn decompressed_name_cannot_collapse_to_a_dot_entry() {
        assert_eq!(decompressed_name("x.img.xz"), Ok("x.img"));
        assert_eq!(decompressed_name("x.img"), Ok("x.img"));
        assert!(refused(decompressed_name("...xz")));
        assert!(refused(decompressed_name("..xz")));
        assert!(refused(decompressed_name(".xz")));
    }

    #[test]
    fn sha_mismatch_keeps_tag_at_start_of_final_error() {
        let e = sha_mismatch_err("aa", "bb");
        assert_eq!(e, "[SHA_MISMATCH] SHA256 mismatch: expected aa, got bb");
        assert_eq!(sha_failure_err(&e), e);
    }

    #[test]
    fn sha_unavailable_wording_is_unchanged() {
        let e = "[SHA_UNAVAILABLE] Invalid SHA file format";
        assert_eq!(
            sha_failure_err(e),
            "SHA256 verification failed: [SHA_UNAVAILABLE] Invalid SHA file format"
        );
    }

    #[test]
    fn an_inline_sha256_is_the_expected_hash() {
        let hex = "ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        assert_eq!(inline_sha(hex), Ok(hex.to_ascii_lowercase()));
        assert!(inline_sha("abc").is_err_and(|e| e.contains("[SHA_UNAVAILABLE]")));
        assert!(inline_sha(&"g".repeat(64)).is_err());
    }

    #[test]
    fn file_urls_are_local_sources_and_http_urls_are_not() {
        assert!(local_source("https://example.com/a.img").is_none());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a b.img");
        let url = reqwest::Url::from_file_path(&path).unwrap();
        assert_eq!(local_source(url.as_str()).unwrap().unwrap(), path);
        assert!(
            local_source("file:///").unwrap().is_err()
                || local_source("file://host/x").unwrap().is_err()
        );
    }

    #[tokio::test]
    async fn a_local_source_outside_the_catalog_folder_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.img");
        std::fs::write(&file, b"x").unwrap();
        let url = reqwest::Url::from_file_path(&file).unwrap();

        let result = download_image(
            url.as_str(),
            None,
            &dir.path().join("out"),
            Arc::new(DownloadState::new()),
        )
        .await;

        assert!(result.is_err_and(|e| e.contains("outside the catalog folder")));
    }

    /// Serves `body` for every request on a free local port.
    async fn serve(body: &'static [u8]) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 2048];
                    let _ = socket.read(&mut buf).await;
                    let head = format!(
                        "HTTP/1.1 200 OK
Content-Length: {}
Connection: close

",
                        body.len()
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(body).await;
                });
            }
        });
        format!("http://{addr}")
    }

    // sha256 of the bytes "wukong image" (sha256sum)
    const BODY: &[u8] = b"wukong image";
    const BODY_SHA: &str = "459384a9b0e55264aff52979c306fccd5af27ca2c432dbcee75c2c8bace55df9";

    #[tokio::test]
    async fn a_catalog_image_with_a_wrong_sha256_is_deleted_and_refused() {
        let base = serve(BODY).await;
        let out = tempfile::tempdir().unwrap();
        let wrong = format!("{INLINE_SHA_PREFIX}{}", "0".repeat(64));

        let result = download_image(
            &format!("{base}/wrongsum-test.img"),
            Some(&wrong),
            &out.path().to_path_buf(),
            Arc::new(DownloadState::new()),
        )
        .await;

        assert!(result.is_err_and(|e| e.starts_with(TAG_SHA_MISMATCH)));
        let left: Vec<_> = std::fs::read_dir(out.path()).unwrap().collect();
        assert!(left.is_empty(), "files left behind: {left:?}");
    }

    #[tokio::test]
    async fn a_catalog_image_with_the_right_sha256_is_kept() {
        let base = serve(BODY).await;
        let out = tempfile::tempdir().unwrap();
        let right = format!("{INLINE_SHA_PREFIX}{BODY_SHA}");

        let path = download_image(
            &format!("{base}/rightsum-test.img"),
            Some(&right),
            &out.path().to_path_buf(),
            Arc::new(DownloadState::new()),
        )
        .await
        .unwrap();

        assert_eq!(std::fs::read(path).unwrap(), BODY);
    }
}
