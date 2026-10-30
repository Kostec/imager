// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2025-2026 Daniele Briguglio, superkali@armbian.com

//! Board, image, and vendor data, served from the active catalog.

mod filters;
mod models;

pub use filters::{map_board, map_images};
pub use models::{ApiBoardSummary, ApiImage, ApiQdl, ApiVendor, BoardInfo, ImageInfo};

use crate::catalog::{self, Catalog};
use crate::config;
use crate::utils::assets_dir;
use crate::{log_debug, log_error, log_warn};

use once_cell::sync::Lazy;
use reqwest::header::{HeaderMap, HeaderValue};

/// HTTP client for the QDL board lookup below (10s timeout, X-Armbian-Client header).
static API_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert(
        config::http::CLIENT_HEADER_NAME,
        HeaderValue::from_static(config::http::CLIENT_HEADER_VALUE),
    );

    reqwest::Client::builder()
        .default_headers(headers)
        .user_agent(config::app::USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(
            config::http::CONNECT_TIMEOUT_SECS,
        ))
        .timeout(std::time::Duration::from_secs(
            config::http::SHORT_TIMEOUT_SECS,
        ))
        .build()
        .expect("Failed to create API HTTP client")
});

/// Fetch a single board's QDL block from the API detail endpoint. Returns None
/// when the board has no QDL metadata or on any network/parse error, so the
/// caller falls back to the bundled registry.
pub async fn fetch_board_qdl(slug: &str) -> Option<ApiQdl> {
    let url = format!("{}/boards/{}", config::urls::api_base(), slug);
    let response = API_CLIENT
        .get(&url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let parsed: models::ApiResponse<ApiBoardSummary> = response.json().await.ok()?;
    parsed.data.qdl
}

/// Delete the pre-migration API cache file.
pub fn cleanup_legacy_cache() {
    let legacy_path = assets_dir().join("api-images.json");
    if legacy_path.exists() {
        match std::fs::remove_file(&legacy_path) {
            Ok(_) => log_debug!(
                "images",
                "Removed legacy api-images.json cache: {}",
                legacy_path.display()
            ),
            Err(e) => log_warn!("images", "Failed to remove legacy cache: {}", e),
        }
    }
}

async fn load_catalog() -> Result<Catalog, String> {
    catalog::load(&catalog::current_source())
        .await
        .map_err(|e| {
            log_error!("images", "Failed to load the catalog: {}", e);
            e.to_string()
        })
}

/// All boards of the active catalog.
pub async fn fetch_boards() -> Result<Vec<ApiBoardSummary>, String> {
    Ok(load_catalog().await?.boards)
}

/// A board's images in the active catalog, narrowed by the optional filters.
pub async fn fetch_images_for_board(
    slug: &str,
    variant: Option<&str>,
    distribution: Option<&str>,
    branch: Option<&str>,
    promoted: Option<bool>,
) -> Result<Vec<ApiImage>, String> {
    let mut images = load_catalog()
        .await?
        .images
        .remove(slug)
        .unwrap_or_default();
    images.retain(|image| {
        variant.is_none_or(|v| image.variant == v)
            && distribution.is_none_or(|d| image.distribution == d)
            && branch.is_none_or(|b| image.kernel_branch == b)
            && promoted.is_none_or(|p| image.promoted == p)
    });
    Ok(images)
}

/// All vendors of the active catalog.
pub async fn fetch_vendors() -> Result<Vec<ApiVendor>, String> {
    Ok(load_catalog().await?.vendors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn boards_vendors_and_images_come_from_the_built_in_catalog() {
        // Nothing listens here: a request would fail instead of answering.
        std::env::set_var("ARMBIAN_API_BASE", "http://127.0.0.1:9");

        let boards = fetch_boards().await.unwrap();
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].slug, "wukongpi");

        let vendors = fetch_vendors().await.unwrap();
        assert_eq!(vendors.len(), 1);
        assert_eq!(vendors[0].slug, "wukong");

        let images = fetch_images_for_board("wukongpi", None, None, None, None)
            .await
            .unwrap();
        assert!(images.is_empty());
    }
}
