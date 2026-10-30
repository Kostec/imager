// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2025-2026 Daniele Briguglio, superkali@armbian.com

//! Board and image queries: fetch and filter board/image data from the active catalog.

use std::collections::HashSet;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use tauri::State;

use crate::config;
use crate::devices::{get_block_devices as devices_get_block_devices, BlockDevice};
use crate::images::{
    fetch_boards, fetch_images_for_board, fetch_vendors, map_board, map_images, ApiVendor,
    BoardInfo, ImageInfo,
};
use crate::{log_debug, log_error, log_info};

use super::state::AppState;

/// Track previously seen device paths to detect changes
static PREV_DEVICE_PATHS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// Get list of available boards from the active catalog
#[tauri::command]
pub async fn get_boards(state: State<'_, AppState>) -> Result<Vec<BoardInfo>, String> {
    log_debug!("board_queries", "Fetching boards list");

    #[cfg(debug_assertions)]
    if super::dev_scenarios::api_returns_empty("Boards").await? {
        return Ok(Vec::new());
    }

    let mut boards_guard = state.boards.lock().await;
    if boards_guard.is_none() {
        log_debug!("board_queries", "Cache miss - fetching from API");
        let api_boards = fetch_boards().await.map_err(|e| {
            log_error!("board_queries", "Failed to fetch boards: {}", e);
            e
        })?;
        *boards_guard = Some(api_boards);
    }

    let api_boards = boards_guard
        .as_ref()
        .ok_or_else(|| "Boards cache was not populated after fetch".to_string())?;
    let boards: Vec<BoardInfo> = api_boards.iter().map(map_board).collect();
    log_debug!("board_queries", "Found {} boards", boards.len());
    Ok(boards)
}

/// Get images available for a specific board from the active catalog
#[tauri::command]
pub async fn get_images_for_board(
    board_slug: String,
    preapp_filter: Option<String>,
    kernel_filter: Option<String>,
    variant_filter: Option<String>,
    stability: Option<String>,
) -> Result<Vec<ImageInfo>, String> {
    log_debug!(
        "board_queries",
        "Getting images for board: {} (stability: {:?}, preapp: {:?}, kernel: {:?}, variant: {:?})",
        board_slug,
        stability,
        preapp_filter,
        kernel_filter,
        variant_filter
    );

    #[cfg(debug_assertions)]
    if super::dev_scenarios::api_returns_empty("Images").await? {
        return Ok(Vec::new());
    }

    // Push the filters the API supports server-side; the rest are applied below.
    let api_images = fetch_images_for_board(
        &board_slug,
        variant_filter.as_deref(),
        None, // distribution filter not used in current UI
        kernel_filter.as_deref(),
        None, // promoted filter not used directly
    )
    .await
    .map_err(|e| {
        log_error!(
            "board_queries",
            "Failed to fetch images for board {}: {}",
            board_slug,
            e
        );
        e
    })?;

    let mut images = map_images(api_images);

    // Client-side filters the API can't express.
    if let Some(ref filter) = preapp_filter {
        if filter == config::images::EMPTY_FILTER {
            images.retain(|img| img.preinstalled_application.is_empty());
        } else {
            images.retain(|img| img.preinstalled_application == *filter);
        }
    }

    if let Some(ref filter) = stability {
        images.retain(|img| img.stability == *filter);
    }

    log_debug!(
        "board_queries",
        "Found {} images for board {}",
        images.len(),
        board_slug
    );
    Ok(images)
}

/// Get list of vendors/manufacturers from the active catalog
#[tauri::command]
pub async fn get_vendors(state: State<'_, AppState>) -> Result<Vec<ApiVendor>, String> {
    log_debug!("board_queries", "Fetching vendors list");

    #[cfg(debug_assertions)]
    if super::dev_scenarios::api_returns_empty("Vendors").await? {
        return Ok(Vec::new());
    }

    let mut vendors_guard = state.vendors.lock().await;
    if vendors_guard.is_none() {
        log_debug!("board_queries", "Vendors cache miss - fetching from API");
        let api_vendors = fetch_vendors().await.map_err(|e| {
            log_error!("board_queries", "Failed to fetch vendors: {}", e);
            e
        })?;
        *vendors_guard = Some(api_vendors);
    }

    let vendors = vendors_guard
        .as_ref()
        .ok_or_else(|| "Vendors cache was not populated after fetch".to_string())?
        .clone();
    log_debug!("board_queries", "Found {} vendors", vendors.len());
    Ok(vendors)
}

/// Get available block devices
#[tauri::command]
pub async fn get_block_devices() -> Result<Vec<BlockDevice>, String> {
    #[cfg(debug_assertions)]
    let scanned = super::dev_scenarios::list_block_devices(devices_get_block_devices);
    #[cfg(not(debug_assertions))]
    let scanned = devices_get_block_devices();

    let devices = scanned.map_err(|e| {
        log_error!("board_queries", "Failed to get block devices: {}", e);
        e
    })?;

    // Log only when the device set changes, to avoid flooding the polling loop.
    let current_paths: HashSet<String> = devices.iter().map(|d| d.path.clone()).collect();
    let mut prev_paths = PREV_DEVICE_PATHS.lock().unwrap();

    if *prev_paths != current_paths {
        let added: Vec<_> = current_paths.difference(&prev_paths).collect();
        let removed: Vec<_> = prev_paths.difference(&current_paths).collect();

        if !added.is_empty() {
            log_info!("board_queries", "Device(s) added: {:?}", added);
        }
        if !removed.is_empty() {
            log_info!("board_queries", "Device(s) removed: {:?}", removed);
        }
        if added.is_empty() && removed.is_empty() {
            // First scan.
            log_info!("board_queries", "Found {} block devices", devices.len());
        }

        *prev_paths = current_paths;
    }

    Ok(devices)
}

// ─── Catalog source ──────────────────────────────────────────────────────────

/// The `catalog_source` setting: empty for the built-in catalog, else a URL or a folder path
#[tauri::command]
pub fn get_catalog_source() -> String {
    crate::catalog::current_source().to_setting()
}

/// Make `value` the catalog source (empty = built-in) and drop what the old one served
#[tauri::command]
pub async fn set_catalog_source(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    value: String,
) -> Result<(), String> {
    let source = crate::catalog::CatalogSource::parse(&value);
    crate::catalog::save_source(&app, &source)?;
    log_info!(
        "board_queries",
        "Catalog source set to {:?}",
        source.to_setting()
    );
    *state.boards.lock().await = None;
    *state.vendors.lock().await = None;
    Ok(())
}

/// Try a source without activating it: counts when it loads, the reason when it does not
#[tauri::command]
pub async fn check_catalog_source(value: String) -> crate::catalog::CatalogCheck {
    crate::catalog::check(&crate::catalog::CatalogSource::parse(&value)).await
}

/// Why the active external catalog could not be used, if it could not
#[tauri::command]
pub fn get_catalog_error() -> Option<String> {
    crate::catalog::last_error()
}

/// Let the user pick the folder that holds `catalog.json`
#[tauri::command]
pub async fn pick_catalog_folder(window: tauri::Window) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let picked = window
        .dialog()
        .file()
        .set_title("Select the catalog folder")
        .blocking_pick_folder();
    Ok(picked
        .and_then(|path| path.into_path().ok())
        .map(|path| path.display().to_string()))
}
