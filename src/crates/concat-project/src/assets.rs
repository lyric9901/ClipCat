// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Creative Asset Ecosystem: 16-category asset indexing, offline database,
//! and asset pack manifest specification.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The 16 canonical creative asset categories supported by Concat.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetCategory {
    /// Sound effects (meme sounds, whooshes, impacts, risers, ambience).
    SoundEffects,
    /// Background music tracks, loops, and score elements.
    Music,
    /// Video effect shader packages and presets.
    VideoEffects,
    /// Clip transitions (wipes, dissolves, glitches, 3D moves).
    Transitions,
    /// Kinetic text animations and typography motion templates.
    TextAnimations,
    /// Typefaces and font packages (TTF, OTF, WOFF2).
    Fonts,
    /// Lower thirds, title cards, and broadcast titles.
    Titles,
    /// Decorative 2D stickers and animated graphics.
    Stickers,
    /// Video overlays (grain, VHS, light leaks, dust, scanlines).
    Overlays,
    /// Complex motion graphics compositions and vector sequences.
    MotionGraphics,
    /// 3D Lookup Tables (.cube files) for color grading.
    Luts,
    /// Three-way wheel and curve grading presets.
    ColorPresets,
    /// Full project and sequence video templates.
    Templates,
    /// Video and still backgrounds, patterns, gradients.
    Backgrounds,
    /// Vector shapes, arrows, callouts, and graphic elements.
    ShapesAndElements,
    /// Local machine-learning models (Whisper, ONNX segmentation, upscale).
    AiModels,
}

impl AssetCategory {
    /// Returns the static string representation of the category.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::SoundEffects => "sound-effects",
            Self::Music => "music",
            Self::VideoEffects => "video-effects",
            Self::Transitions => "transitions",
            Self::TextAnimations => "text-animations",
            Self::Fonts => "fonts",
            Self::Titles => "titles",
            Self::Stickers => "stickers",
            Self::Overlays => "overlays",
            Self::MotionGraphics => "motion-graphics",
            Self::Luts => "luts",
            Self::ColorPresets => "color-presets",
            Self::Templates => "templates",
            Self::Backgrounds => "backgrounds",
            Self::ShapesAndElements => "shapes-and-elements",
            Self::AiModels => "ai-models",
        }
    }

    /// Returns human-readable English display name.
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::SoundEffects => "Sound Effects",
            Self::Music => "Music",
            Self::VideoEffects => "Video Effects",
            Self::Transitions => "Transitions",
            Self::TextAnimations => "Text Animations",
            Self::Fonts => "Fonts",
            Self::Titles => "Titles",
            Self::Stickers => "Stickers",
            Self::Overlays => "Overlays",
            Self::MotionGraphics => "Motion Graphics",
            Self::Luts => "LUTs",
            Self::ColorPresets => "Color Presets",
            Self::Templates => "Templates",
            Self::Backgrounds => "Backgrounds",
            Self::ShapesAndElements => "Shapes & Elements",
            Self::AiModels => "AI Models",
        }
    }

    /// All 16 categories in standard browsing order.
    pub const ALL: [Self; 16] = [
        Self::SoundEffects,
        Self::Music,
        Self::VideoEffects,
        Self::Transitions,
        Self::TextAnimations,
        Self::Fonts,
        Self::Titles,
        Self::Stickers,
        Self::Overlays,
        Self::MotionGraphics,
        Self::Luts,
        Self::ColorPresets,
        Self::Templates,
        Self::Backgrounds,
        Self::ShapesAndElements,
        Self::AiModels,
    ];

    /// Parses category from string identifier.
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim().to_ascii_lowercase();
        match trimmed.as_str() {
            "sound-effects" | "sound_effects" | "sfx" | "sounds" => Some(Self::SoundEffects),
            "music" | "songs" | "audio" => Some(Self::Music),
            "video-effects" | "video_effects" | "effects" | "fx" => Some(Self::VideoEffects),
            "transitions" | "transition" => Some(Self::Transitions),
            "text-animations" | "text_animations" | "kinetic" => Some(Self::TextAnimations),
            "fonts" | "font" | "typefaces" => Some(Self::Fonts),
            "titles" | "title" | "lower-thirds" => Some(Self::Titles),
            "stickers" | "sticker" => Some(Self::Stickers),
            "overlays" | "overlay" => Some(Self::Overlays),
            "motion-graphics" | "motion_graphics" | "motion" | "graphics" => {
                Some(Self::MotionGraphics)
            }
            "luts" | "lut" | "cube" => Some(Self::Luts),
            "color-presets" | "color_presets" | "grades" => Some(Self::ColorPresets),
            "templates" | "template" => Some(Self::Templates),
            "backgrounds" | "background" | "backdrops" => Some(Self::Backgrounds),
            "shapes-and-elements" | "shapes" | "elements" => Some(Self::ShapesAndElements),
            "ai-models" | "ai_models" | "models" => Some(Self::AiModels),
            _ => None,
        }
    }
}

/// A creative asset item indexed in the local database or online catalogue.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetItem {
    /// Unique identifier across providers (e.g. `concat.lut.cinematic-teal` or `myinstants.vine-boom`).
    pub id: String,
    /// Human-readable title of the asset.
    pub name: String,
    /// Primary asset category.
    pub category: AssetCategory,
    /// Detailed description, usage notes, or keywords.
    #[serde(default)]
    pub description: String,
    /// Optional preview URL (audio snippet, animated GIF, WebM video, or sample text).
    #[serde(default)]
    pub preview_url: Option<String>,
    /// Optional thumbnail image URL.
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    /// Remote download URL if an online asset.
    #[serde(default)]
    pub download_url: Option<String>,
    /// Originating provider name (e.g. "Concat Built-in", "MyInstants", "Local Import").
    #[serde(default = "default_source")]
    pub source: String,
    /// Semantic version string where applicable (e.g. "1.0.0").
    #[serde(default = "default_version")]
    pub version: String,
    /// Local file path on disk if already installed / cached.
    #[serde(default)]
    pub local_path: Option<String>,
    /// License identifier (e.g. "CC0-1.0", "OFL-1.1", "MIT", "Apache-2.0").
    #[serde(default = "default_license")]
    pub license: String,
    /// Author attribution or copyright statement.
    #[serde(default)]
    pub attribution: Option<String>,
    /// File size in bytes for download progress and cache accounting.
    #[serde(default)]
    pub file_bytes: u64,
    /// Hex-encoded SHA-256 hash for cryptographic integrity verification.
    #[serde(default)]
    pub sha256: String,
    /// Whether the asset file is currently present in local storage.
    #[serde(default)]
    pub downloaded: bool,
    /// Whether user marked this asset as a favorite.
    #[serde(default)]
    pub favorite: bool,
    /// Search tags and classification keywords.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_source() -> String {
    "Concat Built-in".to_string()
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_license() -> String {
    "CC0-1.0".to_string()
}

impl AssetItem {
    /// Creates a minimal local asset record.
    pub fn new_local(
        id: impl Into<String>,
        name: impl Into<String>,
        category: AssetCategory,
        local_path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category,
            description: String::new(),
            preview_url: None,
            thumbnail_url: None,
            download_url: None,
            source: "Local Import".to_string(),
            version: "1.0.0".to_string(),
            local_path: Some(local_path.into()),
            license: "Personal".to_string(),
            attribution: None,
            file_bytes: 0,
            sha256: String::new(),
            downloaded: true,
            favorite: false,
            tags: Vec::new(),
        }
    }

    /// Verifies if search query matches name, description, tags, or id.
    pub fn matches_query(&self, query: &str) -> bool {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return true;
        }
        self.name.to_ascii_lowercase().contains(&q)
            || self.description.to_ascii_lowercase().contains(&q)
            || self.id.to_ascii_lowercase().contains(&q)
            || self.source.to_ascii_lowercase().contains(&q)
            || self
                .tags
                .iter()
                .any(|t| t.to_ascii_lowercase().contains(&q))
    }
}

/// Filter criteria for querying the asset database.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AssetFilter {
    /// Filter by exact category.
    pub category: Option<AssetCategory>,
    /// Free-text search across name, tags, description.
    pub query: Option<String>,
    /// Restrict results to user favorites.
    pub only_favorites: bool,
    /// Restrict results to downloaded / locally available items.
    pub only_downloaded: bool,
    /// Filter by exact tag.
    pub tag: Option<String>,
}

/// Reusable Creative Asset Pack manifest (.concatpack).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPackManifest {
    /// Unique pack ID (e.g. `concat.pack.cinematic-starter`).
    pub id: String,
    /// Display name of the pack.
    pub name: String,
    /// Semantic version of the pack.
    pub version: String,
    /// Detailed description.
    pub description: String,
    /// Pack creator or publisher.
    pub author: String,
    /// Primary asset category of the pack.
    pub category: AssetCategory,
    /// Licensing terms.
    pub license: String,
    /// Project homepage or source link.
    #[serde(default)]
    pub homepage: Option<String>,
    /// Pack dependencies (other pack IDs required).
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Assets bundled inside this pack.
    #[serde(default)]
    pub assets: Vec<AssetItem>,
    /// Expected SHA-256 digest of the archive file.
    #[serde(default)]
    pub sha256: String,
}

/// Local offline asset database managing indexing, favorites, and installed packs.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDatabase {
    /// All indexed asset records.
    pub items: Vec<AssetItem>,
    /// Installed asset packs.
    #[serde(default)]
    pub installed_packs: Vec<AssetPackManifest>,
}

impl AssetDatabase {
    /// Creates a new empty database.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or updates an asset item by unique ID.
    pub fn insert_or_update(&mut self, item: AssetItem) {
        if let Some(existing) = self.items.iter_mut().find(|it| it.id == item.id) {
            let fav = existing.favorite;
            *existing = item;
            existing.favorite = fav; // preserve user favorite state on metadata updates
        } else {
            self.items.push(item);
        }
    }

    /// Gets an asset by ID.
    pub fn get(&self, id: &str) -> Option<&AssetItem> {
        self.items.iter().find(|it| it.id == id)
    }

    /// Gets a mutable asset reference by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut AssetItem> {
        self.items.iter_mut().find(|it| it.id == id)
    }

    /// Toggles favorite state of an asset. Returns new state, or false if not found.
    pub fn toggle_favorite(&mut self, id: &str) -> bool {
        if let Some(item) = self.get_mut(id) {
            item.favorite = !item.favorite;
            item.favorite
        } else {
            false
        }
    }

    /// Updates download status and local path of an asset.
    pub fn set_downloaded(&mut self, id: &str, path: impl Into<String>) -> bool {
        if let Some(item) = self.get_mut(id) {
            item.downloaded = true;
            item.local_path = Some(path.into());
            true
        } else {
            false
        }
    }

    /// Filters assets using the given criteria.
    pub fn filter(&self, criteria: &AssetFilter) -> Vec<&AssetItem> {
        self.items
            .iter()
            .filter(|it| {
                if let Some(cat) = criteria.category {
                    if it.category != cat {
                        return false;
                    }
                }
                if criteria.only_favorites && !it.favorite {
                    return false;
                }
                if criteria.only_downloaded && !it.downloaded {
                    return false;
                }
                if let Some(tag) = &criteria.tag {
                    if !it.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                        return false;
                    }
                }
                if let Some(query) = &criteria.query {
                    if !it.matches_query(query) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Computes the count of items in each category.
    pub fn category_counts(&self) -> HashMap<AssetCategory, usize> {
        let mut counts = HashMap::new();
        for cat in AssetCategory::ALL {
            counts.insert(cat, 0);
        }
        for item in &self.items {
            *counts.entry(item.category).or_insert(0) += 1;
        }
        counts
    }

    /// Removes an item by ID.
    pub fn remove(&mut self, id: &str) -> Option<AssetItem> {
        if let Some(idx) = self.items.iter().position(|it| it.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    /// Saves the database to disk atomically via temporary file replacement.
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = path.with_extension("tmp");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// Loads the database from disk. Returns default empty database if file does not exist.
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let db: Self = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(db)
    }
}

use std::sync::atomic::{AtomicBool, Ordering};

static FORCE_OFFLINE: AtomicBool = AtomicBool::new(false);

/// Utility for querying network connectivity status without blocking UI threads.
pub struct NetworkStatus;

impl NetworkStatus {
    /// Returns true if offline mode is active or forced.
    pub fn is_offline() -> bool {
        FORCE_OFFLINE.load(Ordering::Relaxed)
    }

    /// Explicitly forces offline mode (e.g. for air-gapped workflows or testing).
    pub fn set_force_offline(force: bool) {
        FORCE_OFFLINE.store(force, Ordering::Relaxed);
    }
}

/// Installer for unpacking and verifying creative asset packs.
pub struct AssetPackInstaller;

impl AssetPackInstaller {
    /// Installs an unpacked asset pack folder into the target library directory and updates the database.
    pub fn install_from_folder(
        pack_folder: &Path,
        target_library_dir: &Path,
        db: &mut AssetDatabase,
    ) -> Result<AssetPackManifest, String> {
        let manifest_file = pack_folder.join("manifest.json");
        if !manifest_file.exists() {
            return Err("pack folder is missing 'manifest.json'".to_string());
        }
        let manifest_content = std::fs::read_to_string(&manifest_file)
            .map_err(|e| format!("failed to read manifest.json: {e}"))?;
        let mut manifest: AssetPackManifest = serde_json::from_str(&manifest_content)
            .map_err(|e| format!("failed to parse manifest.json: {e}"))?;

        validate_pack_manifest(&manifest)?;

        let dest_pack_dir = target_library_dir.join(&manifest.id);
        std::fs::create_dir_all(&dest_pack_dir)
            .map_err(|e| format!("failed to create pack destination folder: {e}"))?;

        for asset in &mut manifest.assets {
            if let Some(ref rel_path_str) = asset.local_path {
                let sanitized = sanitize_relative_path(rel_path_str)?;
                let source_file = pack_folder.join(&sanitized);
                let dest_file = dest_pack_dir.join(&sanitized);

                if source_file.exists() {
                    if let Some(dest_parent) = dest_file.parent() {
                        let _ = std::fs::create_dir_all(dest_parent);
                    }
                    std::fs::copy(&source_file, &dest_file).map_err(|e| {
                        format!(
                            "failed to copy asset payload '{}': {e}",
                            sanitized.display()
                        )
                    })?;
                    asset.local_path = Some(dest_file.to_string_lossy().to_string());
                    asset.downloaded = true;
                }
            }
            db.insert_or_update(asset.clone());
        }

        // Record installed pack
        if let Some(existing) = db.installed_packs.iter_mut().find(|p| p.id == manifest.id) {
            *existing = manifest.clone();
        } else {
            db.installed_packs.push(manifest.clone());
        }

        Ok(manifest)
    }
}

use std::path::{Path, PathBuf};

/// Provider interface for discovering, browsing, and querying creative assets.
pub trait AssetProvider: Send + Sync {
    /// Human-readable identifier of the provider.
    fn name(&self) -> &str;
    /// Asset categories this provider can supply.
    fn supported_categories(&self) -> &[AssetCategory];
    /// Returns true if this provider needs network connectivity to fetch new items.
    fn is_online(&self) -> bool;
    /// Discovers assets in the given category.
    fn fetch_catalogue(&self, category: Option<AssetCategory>) -> Result<Vec<AssetItem>, String>;
    /// Searches assets by free-text keyword.
    fn search(
        &self,
        query: &str,
        category: Option<AssetCategory>,
    ) -> Result<Vec<AssetItem>, String>;
}

/// Provider that searches local files in the asset database.
pub struct LocalAssetProvider<'a> {
    /// Reference to the local database.
    pub db: &'a AssetDatabase,
}

impl<'a> AssetProvider for LocalAssetProvider<'a> {
    fn name(&self) -> &str {
        "Local Storage"
    }
    fn supported_categories(&self) -> &[AssetCategory] {
        &AssetCategory::ALL
    }
    fn is_online(&self) -> bool {
        false
    }
    fn fetch_catalogue(&self, category: Option<AssetCategory>) -> Result<Vec<AssetItem>, String> {
        Ok(self
            .db
            .filter(&AssetFilter {
                category,
                only_downloaded: true,
                ..Default::default()
            })
            .into_iter()
            .cloned()
            .collect())
    }
    fn search(
        &self,
        query: &str,
        category: Option<AssetCategory>,
    ) -> Result<Vec<AssetItem>, String> {
        Ok(self
            .db
            .filter(&AssetFilter {
                category,
                query: Some(query.to_string()),
                only_downloaded: true,
                ..Default::default()
            })
            .into_iter()
            .cloned()
            .collect())
    }
}

/// MyInstants sound provider with guaranteed offline fallback.
pub struct MyInstantsAssetProvider;

impl MyInstantsAssetProvider {
    /// Returns built-in offline meme soundboard as AssetItems.
    pub fn offline_fallback() -> Vec<AssetItem> {
        crate::meme_sounds::builtin_meme_catalogue()
            .into_iter()
            .map(|meme| AssetItem {
                id: format!("myinstants.{}", meme.id),
                name: meme.name,
                category: AssetCategory::SoundEffects,
                description: "Built-in offline meme sound effect".to_string(),
                preview_url: Some(meme.mp3_url.clone()),
                thumbnail_url: None,
                download_url: Some(meme.mp3_url),
                source: "MyInstants (Offline Board)".to_string(),
                version: "1.0.0".to_string(),
                local_path: None,
                license: "CC0-1.0".to_string(),
                attribution: None,
                file_bytes: 48000,
                sha256: String::new(),
                downloaded: false,
                favorite: false,
                tags: vec!["meme".to_string(), "sound".to_string(), "sfx".to_string()],
            })
            .collect()
    }
}

impl AssetProvider for MyInstantsAssetProvider {
    fn name(&self) -> &str {
        "MyInstants"
    }
    fn supported_categories(&self) -> &[AssetCategory] {
        &[AssetCategory::SoundEffects]
    }
    fn is_online(&self) -> bool {
        true
    }
    fn fetch_catalogue(&self, _category: Option<AssetCategory>) -> Result<Vec<AssetItem>, String> {
        Ok(Self::offline_fallback())
    }
    fn search(
        &self,
        query: &str,
        _category: Option<AssetCategory>,
    ) -> Result<Vec<AssetItem>, String> {
        let q = query.trim().to_ascii_lowercase();
        let fallback = Self::offline_fallback();
        let filtered: Vec<AssetItem> = fallback
            .into_iter()
            .filter(|it| it.matches_query(&q))
            .collect();
        Ok(filtered)
    }
}

/// Safe path validator preventing path traversal attacks (zip slips).
pub fn sanitize_relative_path(path: &str) -> Result<PathBuf, String> {
    let p = Path::new(path);
    for component in p.components() {
        match component {
            std::path::Component::Normal(part) => {
                let s = part.to_string_lossy();
                if s.contains(':') || s.contains('\\') {
                    return Err(format!("invalid character in path component: {s}"));
                }
            }
            std::path::Component::CurDir => continue,
            std::path::Component::ParentDir => {
                return Err("parent directory traversal '..' is strictly forbidden".to_string());
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err("absolute path is strictly forbidden in asset pack".to_string());
            }
        }
    }
    Ok(p.to_path_buf())
}

/// Validates an asset pack manifest for security and integrity.
pub fn validate_pack_manifest(manifest: &AssetPackManifest) -> Result<(), String> {
    if manifest.id.trim().is_empty() {
        return Err("pack id cannot be empty".to_string());
    }
    if !manifest
        .id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        return Err(format!(
            "pack id contains invalid characters: '{}'",
            manifest.id
        ));
    }
    if manifest.name.trim().is_empty() {
        return Err("pack name cannot be empty".to_string());
    }
    if manifest.version.trim().is_empty() {
        return Err("pack version cannot be empty".to_string());
    }
    for asset in &manifest.assets {
        if asset.id.trim().is_empty() {
            return Err("asset id cannot be empty".to_string());
        }
        if let Some(ref local) = asset.local_path {
            sanitize_relative_path(local)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_categories_parsing_and_names() {
        assert_eq!(AssetCategory::ALL.len(), 16);
        for cat in AssetCategory::ALL {
            let s = cat.as_str();
            assert_eq!(AssetCategory::parse(s), Some(cat));
            assert!(!cat.display_name().is_empty());
        }
        assert_eq!(
            AssetCategory::parse("sfx"),
            Some(AssetCategory::SoundEffects)
        );
        assert_eq!(AssetCategory::parse("cube"), Some(AssetCategory::Luts));
        assert_eq!(
            AssetCategory::parse("models"),
            Some(AssetCategory::AiModels)
        );
    }

    #[test]
    fn test_asset_database_crud_and_favorites() {
        let mut db = AssetDatabase::new();
        let sound = AssetItem {
            id: "sfx.vine-boom".to_string(),
            name: "Vine Boom".to_string(),
            category: AssetCategory::SoundEffects,
            description: "Famous meme boom sound effect".to_string(),
            preview_url: None,
            thumbnail_url: None,
            download_url: Some("https://example.com/vine.mp3".to_string()),
            source: "MyInstants".to_string(),
            version: "1.0.0".to_string(),
            local_path: None,
            license: "CC0-1.0".to_string(),
            attribution: None,
            file_bytes: 42000,
            sha256: "abcdef1234567890".to_string(),
            downloaded: false,
            favorite: false,
            tags: vec!["meme".to_string(), "boom".to_string(), "funny".to_string()],
        };

        db.insert_or_update(sound.clone());
        assert_eq!(db.items.len(), 1);
        assert_eq!(db.get("sfx.vine-boom").unwrap().name, "Vine Boom");

        // Favorite toggle
        let is_fav = db.toggle_favorite("sfx.vine-boom");
        assert!(is_fav);
        assert!(db.get("sfx.vine-boom").unwrap().favorite);

        // Update preserving favorite
        let mut updated = sound.clone();
        updated.name = "Vine Boom HD".to_string();
        db.insert_or_update(updated);
        assert_eq!(db.get("sfx.vine-boom").unwrap().name, "Vine Boom HD");
        assert!(db.get("sfx.vine-boom").unwrap().favorite); // preserved

        // Set downloaded
        assert!(db.set_downloaded("sfx.vine-boom", "C:/Concat/sounds/vine.mp3"));
        assert!(db.get("sfx.vine-boom").unwrap().downloaded);
        assert_eq!(
            db.get("sfx.vine-boom").unwrap().local_path.as_deref(),
            Some("C:/Concat/sounds/vine.mp3")
        );
    }

    #[test]
    fn test_asset_database_filtering_and_counts() {
        let mut db = AssetDatabase::new();
        db.insert_or_update(AssetItem::new_local(
            "lut.teal-orange",
            "Teal and Orange",
            AssetCategory::Luts,
            "luts/teal.cube",
        ));
        db.insert_or_update(AssetItem::new_local(
            "font.inter",
            "Inter Display",
            AssetCategory::Fonts,
            "fonts/Inter.ttf",
        ));
        db.insert_or_update(AssetItem::new_local(
            "sfx.whoosh",
            "Cinematic Whoosh",
            AssetCategory::SoundEffects,
            "sounds/whoosh.wav",
        ));

        let counts = db.category_counts();
        assert_eq!(counts[&AssetCategory::Luts], 1);
        assert_eq!(counts[&AssetCategory::Fonts], 1);
        assert_eq!(counts[&AssetCategory::SoundEffects], 1);
        assert_eq!(counts[&AssetCategory::Music], 0);

        // Filter by category
        let luts = db.filter(&AssetFilter {
            category: Some(AssetCategory::Luts),
            ..Default::default()
        });
        assert_eq!(luts.len(), 1);
        assert_eq!(luts[0].id, "lut.teal-orange");

        // Search query
        let search = db.filter(&AssetFilter {
            query: Some("whoosh".to_string()),
            ..Default::default()
        });
        assert_eq!(search.len(), 1);
        assert_eq!(search[0].id, "sfx.whoosh");
    }

    #[test]
    fn test_asset_pack_manifest_roundtrip() {
        let manifest = AssetPackManifest {
            id: "concat.pack.cinematic".to_string(),
            name: "Cinematic Creator Pack".to_string(),
            version: "1.2.0".to_string(),
            description: "Essential cinematic LUTs, transitions, and audio risers".to_string(),
            author: "Concat Studio".to_string(),
            category: AssetCategory::Templates,
            license: "MIT".to_string(),
            homepage: Some("https://concat.video".to_string()),
            dependencies: vec!["concat.core.shaders".to_string()],
            assets: vec![AssetItem::new_local(
                "lut.moody-film",
                "Moody Film",
                AssetCategory::Luts,
                "luts/moody.cube",
            )],
            sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
        };

        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let parsed: AssetPackManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "concat.pack.cinematic");
        assert_eq!(parsed.assets.len(), 1);
        assert_eq!(parsed.category, AssetCategory::Templates);
    }

    #[test]
    fn test_path_sanitization_and_manifest_validation() {
        assert!(sanitize_relative_path("luts/creative/teal.cube").is_ok());
        assert!(sanitize_relative_path("../secret.txt").is_err());
        assert!(sanitize_relative_path("a/../../b").is_err());
        assert!(sanitize_relative_path("C:/Windows/System32").is_err());
        assert!(sanitize_relative_path("/etc/passwd").is_err());

        let good = AssetPackManifest {
            id: "good.pack-1".to_string(),
            name: "Good Pack".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            author: "Tester".to_string(),
            category: AssetCategory::Luts,
            license: "MIT".to_string(),
            homepage: None,
            dependencies: vec![],
            assets: vec![AssetItem::new_local(
                "lut.1",
                "LUT 1",
                AssetCategory::Luts,
                "luts/1.cube",
            )],
            sha256: "0".repeat(64),
        };
        assert!(validate_pack_manifest(&good).is_ok());

        let mut bad_path = good.clone();
        bad_path.assets[0].local_path = Some("../bad.cube".to_string());
        assert!(validate_pack_manifest(&bad_path).is_err());

        let mut bad_id = good.clone();
        bad_id.id = "bad id with spaces!".to_string();
        assert!(validate_pack_manifest(&bad_id).is_err());
    }

    #[test]
    fn test_asset_providers_local_and_myinstants() {
        let mut db = AssetDatabase::new();
        db.insert_or_update(AssetItem::new_local(
            "font.inter",
            "Inter",
            AssetCategory::Fonts,
            "fonts/Inter.ttf",
        ));

        let local_prov = LocalAssetProvider { db: &db };
        assert_eq!(local_prov.name(), "Local Storage");
        assert!(!local_prov.is_online());
        let cat = local_prov
            .fetch_catalogue(Some(AssetCategory::Fonts))
            .unwrap();
        assert_eq!(cat.len(), 1);
        assert_eq!(cat[0].name, "Inter");

        let myinstants_prov = MyInstantsAssetProvider;
        assert_eq!(myinstants_prov.name(), "MyInstants");
        assert!(myinstants_prov.is_online());
        let memes = myinstants_prov.fetch_catalogue(None).unwrap();
        assert!(memes.len() >= 10);
        assert!(
            memes
                .iter()
                .any(|m| m.name.contains("Vine Boom") || m.id.contains("vine-boom"))
        );

        let search_meme = myinstants_prov.search("vine", None).unwrap();
        assert!(!search_meme.is_empty());
        assert!(search_meme[0].name.to_lowercase().contains("vine"));
    }

    #[test]
    fn test_asset_database_file_roundtrip_and_offline_guard() {
        let temp_dir = std::env::temp_dir().join("concat_test_db");
        let db_file = temp_dir.join("test_assets.json");

        let mut db = AssetDatabase::new();
        db.insert_or_update(AssetItem::new_local(
            "sfx.hit",
            "Hit",
            AssetCategory::SoundEffects,
            "sounds/hit.wav",
        ));
        db.toggle_favorite("sfx.hit");

        db.save_to_file(&db_file).expect("save db");
        assert!(db_file.exists());

        let loaded = AssetDatabase::load_from_file(&db_file).expect("load db");
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items[0].id, "sfx.hit");
        assert!(loaded.items[0].favorite);

        let _ = std::fs::remove_dir_all(&temp_dir);

        // Offline guard check
        assert!(!NetworkStatus::is_offline());
        NetworkStatus::set_force_offline(true);
        assert!(NetworkStatus::is_offline());
        NetworkStatus::set_force_offline(false);
        assert!(!NetworkStatus::is_offline());
    }

    #[test]
    fn test_asset_pack_installer_from_folder() {
        let temp_dir = std::env::temp_dir().join("concat_pack_install_test");
        let pack_folder = temp_dir.join("source_pack");
        let lib_folder = temp_dir.join("installed_library");

        std::fs::create_dir_all(pack_folder.join("luts")).unwrap();
        std::fs::write(
            pack_folder.join("luts/warm.cube"),
            b"LUT_3D_SIZE 2\n0 0 0\n1 1 1",
        )
        .unwrap();

        let manifest = AssetPackManifest {
            id: "pack.warm-tones".to_string(),
            name: "Warm Tones".to_string(),
            version: "1.0.0".to_string(),
            description: "Warm film looks".to_string(),
            author: "Concat".to_string(),
            category: AssetCategory::Luts,
            license: "MIT".to_string(),
            homepage: None,
            dependencies: vec![],
            assets: vec![AssetItem::new_local(
                "lut.warm",
                "Warm Look",
                AssetCategory::Luts,
                "luts/warm.cube",
            )],
            sha256: "0".repeat(64),
        };
        std::fs::write(
            pack_folder.join("manifest.json"),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();

        let mut db = AssetDatabase::new();
        let result = AssetPackInstaller::install_from_folder(&pack_folder, &lib_folder, &mut db);
        assert!(result.is_ok());

        assert_eq!(db.installed_packs.len(), 1);
        assert_eq!(db.installed_packs[0].id, "pack.warm-tones");

        let installed_lut = db.get("lut.warm").unwrap();
        assert!(installed_lut.downloaded);
        assert!(
            installed_lut
                .local_path
                .as_ref()
                .unwrap()
                .contains("warm.cube")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
