// SPDX-License-Identifier: GPL-2.0-or-later

//! Where boards, vendors and images come from. The built-in catalog is compiled in and
//! always present, so the app works offline; it names the board but ships no image.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use crate::images::{ApiBoardSummary, ApiImage, ApiVendor};

/// Catalog origin, picked by the `catalog_source` setting.
// Url and Folder are only built once that setting exists.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogSource {
    BuiltIn,
    Url(String),
    Folder(PathBuf),
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
    /// This build cannot read the source
    Unsupported(String),
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogError::Unsupported(source) => write!(f, "Unsupported catalog source: {source}"),
        }
    }
}

/// The active source; the built-in catalog until the `catalog_source` setting says otherwise.
pub fn current_source() -> CatalogSource {
    CatalogSource::BuiltIn
}

pub async fn load(source: &CatalogSource) -> Result<Catalog, CatalogError> {
    match source {
        CatalogSource::BuiltIn => Ok(built_in()),
        CatalogSource::Url(url) => Err(CatalogError::Unsupported(url.clone())),
        CatalogSource::Folder(path) => Err(CatalogError::Unsupported(path.display().to_string())),
    }
}

/// The URL whose reachability decides online state; None means there is nothing to reach.
pub fn connectivity_probe(source: &CatalogSource) -> Option<&str> {
    match source {
        CatalogSource::Url(url) => Some(url),
        CatalogSource::BuiltIn | CatalogSource::Folder(_) => None,
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
    async fn external_sources_are_refused_until_they_are_supported() {
        for source in [
            CatalogSource::Url("https://example.com/catalog.json".into()),
            CatalogSource::Folder(PathBuf::from("catalog")),
        ] {
            assert!(matches!(
                load(&source).await,
                Err(CatalogError::Unsupported(_))
            ));
        }
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
