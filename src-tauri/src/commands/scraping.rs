// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2025-2026 Daniele Briguglio, superkali@armbian.com

//! Board photos and vendor logos. The built-in catalog has none, so the UI
//! falls back to its neutral icon and nothing is downloaded.

#[tauri::command]
pub async fn get_cached_board_image(board_slug: String) -> Result<Option<String>, String> {
    let _ = board_slug;
    Ok(None)
}

#[tauri::command]
pub async fn get_cached_vendor_logo(vendor_slug: String) -> Result<Option<String>, String> {
    let _ = vendor_slug;
    Ok(None)
}
