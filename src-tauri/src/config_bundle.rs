//! 配置包（ADR-0027）：全量导出、选择性导入、合并与备份。
//!
//! 包结构：
//!   manifest.json
//!   themes/<主题id>/<图片>
//!   fonts/<字体文件>
//!
//! 导出永远全量；导入只复制被选中的主题/字体。所有条目先校验再落盘，
//! 主题 id、文件名、图片尺寸与字体魔数都走现有白名单。

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::commands::{
    FONT_FILE_MAX_BYTES, THEME_IMAGE_MAX_BYTES, THEME_IMAGE_MAX_SIDE, fnv1a_hex, font_format,
};
use crate::errors::AppError;
use crate::settings::{
    self, Appearance, BUILTIN_FONT_IDS, BUILTIN_THEME_IDS, CUSTOM_FONT_LIMIT, CUSTOM_THEME_LIMIT,
    ClockProfile, CustomFont, SCHEMA_VERSION, Settings, ThemeSpec,
};

const BUNDLE_FORMAT_VERSION: u32 = 1;
const MANIFEST_NAME: &str = "manifest.json";
const PRE_IMPORT_BACKUP: &str = "pre-import.zip";
const MANIFEST_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// 一个配置包解压后的总大小上限。单个资产仍受 32MB 限制。
const BUNDLE_MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const BUNDLE_MAX_ENTRIES: usize = 512;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// 能导入的最老配置包 schema。
///
/// 配置包从 schema 4 开始存在（ADR-0027）；此后每次升版都只**新增**字段，
/// 读取侧的 `serde(default)` 会把缺的补上，所以旧包能安全导入。
/// 反过来必须拒绝：包比程序新时，里面有本程序不认识的字段，静默丢掉等于替用户改配置。
const MIN_BUNDLE_SCHEMA: u32 = 4;

fn bundle_schema_supported(schema_version: u32) -> bool {
    (MIN_BUNDLE_SCHEMA..=SCHEMA_VERSION).contains(&schema_version)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleSettings {
    pub appearance: Appearance,
    pub screensaver: settings::ScreenSaverSettings,
    pub launch: settings::LaunchSettings,
    pub system: BundleSystem,
    pub ui: settings::UiSettings,
}

/// 配置包里的系统偏好。
///
/// 只带 `closeToTray`。自启初始化标记是**本机的一次性判定**：跟着包走到另一台
/// 机器，等于把「这台机器已经问过」这个事实搬过去。这里用独立结构而不是复用
/// `SystemSettings`，是为了在类型层面就让那个字段进不了包——而不是导出时
/// 记得抹掉、导入时记得跳过（两条都得靠人不忘）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleSystem {
    #[serde(default = "close_to_tray_default")]
    pub close_to_tray: bool,
}

fn close_to_tray_default() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleManifest {
    pub format_version: u32,
    pub schema_version: u32,
    pub app_version: String,
    pub exported_at: u64,
    pub settings: BundleSettings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleSummary {
    pub format_version: u32,
    pub schema_version: u32,
    pub app_version: String,
    pub exported_at: u64,
    pub current_theme: String,
    pub builtin_themes: Vec<BundleThemeSummary>,
    pub custom_themes: Vec<BundleThemeSummary>,
    pub fonts: Vec<BundleFontSummary>,
    pub preferences: bool,
    pub existing_custom_themes: usize,
    pub existing_fonts: usize,
    pub backup_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleThemeSummary {
    pub id: String,
    pub name: String,
    pub current: bool,
    pub font: String,
    pub missing_images: Vec<String>,
    /// none | same | id | name
    pub conflict: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleFontSummary {
    pub id: String,
    pub name: String,
    pub used_by: Vec<String>,
    pub missing: bool,
    pub duplicate: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleSelection {
    pub builtin_profiles: bool,
    pub theme_ids: Vec<String>,
    pub font_ids: Vec<String>,
    pub preferences: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub themes: usize,
    pub fonts: usize,
    pub skipped: Vec<ImportSkip>,
    pub current_theme: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSkip {
    pub id: String,
    pub name: String,
    pub reason: ImportSkipReason,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportSkipReason {
    ThemeLimit,
    FontLimit,
    FontMissing,
}

struct BundleArchive {
    manifest: BundleManifest,
    names: HashSet<String>,
    archive: ZipArchive<File>,
}

struct PreparedTheme {
    id: String,
    name: String,
    font: String,
    current: bool,
    missing_images: Vec<String>,
    conflict: String,
    same_id: Option<String>,
    spec: ThemeSpec,
    images: Vec<(String, Vec<u8>)>,
}

struct PreparedBuiltin {
    id: String,
    font: String,
    current: bool,
}

struct PreparedFont {
    font: CustomFont,
    bytes: Option<Vec<u8>>,
    missing: bool,
    duplicate: bool,
    used_by: Vec<String>,
}

struct PreparedBundle {
    format_version: u32,
    schema_version: u32,
    app_version: String,
    exported_at: u64,
    current_theme: String,
    builtin_themes: Vec<PreparedBuiltin>,
    custom_themes: Vec<PreparedTheme>,
    fonts: Vec<PreparedFont>,
    bundle: BundleSettings,
    target_custom_themes: usize,
    target_fonts: usize,
    backup_path: String,
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0)
}

fn app_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

fn bundle_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("backups"))
        .map_err(|_| String::from(AppError::ConfigDirUnavailable))
}

fn backup_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = bundle_dir(app)?;
    fs::create_dir_all(&dir).map_err(|error| {
        String::from(AppError::ConfigBundleBackupFailed {
            detail: error.to_string(),
        })
    })?;
    Ok(dir.join(PRE_IMPORT_BACKUP))
}

struct BundleIo {
    themes_dir: PathBuf,
    fonts_dir: PathBuf,
    backup_path: PathBuf,
    app_version: String,
}

fn io_from_app(app: &AppHandle) -> Result<BundleIo, String> {
    Ok(BundleIo {
        themes_dir: settings::themes_dir(app)?,
        fonts_dir: settings::fonts_dir(app)?,
        backup_path: backup_path(app)?,
        app_version: app_version(app),
    })
}

fn format_error(error: impl std::fmt::Display) -> String {
    String::from(AppError::ConfigBundleInvalid {
        detail: error.to_string(),
    })
}

fn read_error(error: impl std::fmt::Display) -> String {
    String::from(AppError::ConfigBundleReadFailed {
        detail: error.to_string(),
    })
}

fn write_error(error: impl std::fmt::Display) -> String {
    String::from(AppError::ConfigBundleWriteFailed {
        detail: error.to_string(),
    })
}

/* -------------------------------- 导出 -------------------------------- */

pub fn export_bundle(app: &AppHandle, settings: &Settings, path: &Path) -> Result<(), String> {
    let io = io_from_app(app)?;
    export_bundle_with(settings, &io, path)
}

fn export_bundle_with(settings: &Settings, io: &BundleIo, path: &Path) -> Result<(), String> {
    let mut portable = settings.clone();
    settings::normalize_appearance(&mut portable.appearance);

    let manifest = BundleManifest {
        format_version: BUNDLE_FORMAT_VERSION,
        schema_version: SCHEMA_VERSION,
        app_version: io.app_version.clone(),
        exported_at: epoch_seconds(),
        settings: BundleSettings {
            appearance: portable.appearance.clone(),
            screensaver: portable.screensaver.clone(),
            launch: portable.launch.clone(),
            system: BundleSystem {
                close_to_tray: portable.system.close_to_tray,
            },
            ui: portable.ui.clone(),
        },
    };
    let manifest_json = serde_json::to_vec_pretty(&manifest).map_err(write_error)?;

    let themes_dir = &io.themes_dir;
    let fonts_dir = &io.fonts_dir;
    let temp = path.with_extension("tmp");

    let file = File::create(&temp).map_err(write_error)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o644);

    zip.start_file(MANIFEST_NAME, options)
        .map_err(write_error)?;
    zip.write_all(&manifest_json).map_err(write_error)?;

    for theme in &portable.appearance.custom_themes {
        for image in [&theme.background.image, &theme.digit.image]
            .into_iter()
            .flatten()
        {
            if !settings::is_image_name(image) {
                continue;
            }
            let source = themes_dir.join(&theme.id).join(image);
            if !source.exists() {
                continue;
            }
            let bytes = fs::read(&source).map_err(write_error)?;
            zip.start_file(format!("themes/{}/{}", theme.id, image), options)
                .map_err(write_error)?;
            zip.write_all(&bytes).map_err(write_error)?;
        }
    }

    for font in &portable.appearance.fonts {
        let source = fonts_dir.join(&font.file);
        if !source.exists() {
            continue;
        }
        let bytes = fs::read(&source).map_err(write_error)?;
        zip.start_file(format!("fonts/{}", font.file), options)
            .map_err(write_error)?;
        zip.write_all(&bytes).map_err(write_error)?;
    }

    zip.finish().map_err(write_error)?;
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        write_error(error)
    })?;
    Ok(())
}

/* -------------------------------- 读取与校验 -------------------------------- */

fn validate_entry(name: &str, size: u64) -> Result<(), String> {
    if name == MANIFEST_NAME {
        if size > MANIFEST_MAX_BYTES {
            return Err(AppError::ConfigBundleTooLarge {
                detail: String::from(MANIFEST_NAME),
            }
            .into());
        }
        return Ok(());
    }
    if name.ends_with('/') {
        return Err(AppError::ConfigBundleUnsafePath {
            detail: name.to_string(),
        }
        .into());
    }
    if let Some(rest) = name.strip_prefix("themes/") {
        let mut parts = rest.split('/');
        let id = parts.next().unwrap_or_default();
        let file = parts.next().unwrap_or_default();
        if parts.next().is_some()
            || !settings::is_safe_theme_id(id)
            || !settings::is_image_name(file)
        {
            return Err(AppError::ConfigBundleUnsafePath {
                detail: name.to_string(),
            }
            .into());
        }
        if size > THEME_IMAGE_MAX_BYTES {
            return Err(AppError::ConfigBundleTooLarge {
                detail: name.to_string(),
            }
            .into());
        }
        return Ok(());
    }
    if let Some(file) = name.strip_prefix("fonts/") {
        if file.contains('/') || !settings::is_font_name(file) {
            return Err(AppError::ConfigBundleUnsafePath {
                detail: name.to_string(),
            }
            .into());
        }
        if size > FONT_FILE_MAX_BYTES {
            return Err(AppError::ConfigBundleTooLarge {
                detail: name.to_string(),
            }
            .into());
        }
        return Ok(());
    }
    Err(AppError::ConfigBundleUnsafePath {
        detail: name.to_string(),
    }
    .into())
}

fn read_entry(archive: &mut ZipArchive<File>, name: &str, limit: u64) -> Result<Vec<u8>, String> {
    let mut entry = archive.by_name(name).map_err(format_error)?;
    if entry.size() > limit {
        return Err(AppError::ConfigBundleTooLarge {
            detail: name.to_string(),
        }
        .into());
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes).map_err(read_error)?;
    if bytes.len() as u64 > limit {
        return Err(AppError::ConfigBundleTooLarge {
            detail: name.to_string(),
        }
        .into());
    }
    Ok(bytes)
}

fn read_bundle(path: &Path) -> Result<BundleArchive, String> {
    let file = File::open(path).map_err(read_error)?;
    let mut archive = ZipArchive::new(file).map_err(format_error)?;
    if archive.len() > BUNDLE_MAX_ENTRIES {
        return Err(AppError::ConfigBundleTooLarge {
            detail: String::from("too-many-entries"),
        }
        .into());
    }

    let mut names = HashSet::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(format_error)?;
        if entry.is_symlink() {
            return Err(AppError::ConfigBundleUnsafePath {
                detail: entry.name().map(|n| n.to_string()).unwrap_or_default(),
            }
            .into());
        }
        let name = entry.name().map_err(format_error)?.to_string();
        let size = entry.size();
        total = total.saturating_add(size);
        if total > BUNDLE_MAX_TOTAL_BYTES {
            return Err(AppError::ConfigBundleTooLarge {
                detail: String::from("total-size"),
            }
            .into());
        }
        validate_entry(&name, size)?;
        names.insert(name);
    }

    if !names.contains(MANIFEST_NAME) {
        return Err(format_error("missing manifest.json"));
    }
    let manifest_bytes = read_entry(&mut archive, MANIFEST_NAME, MANIFEST_MAX_BYTES)?;
    let mut manifest: BundleManifest =
        serde_json::from_slice(&manifest_bytes).map_err(format_error)?;
    if manifest.format_version != BUNDLE_FORMAT_VERSION
        || !bundle_schema_supported(manifest.schema_version)
    {
        return Err(AppError::ConfigBundleUnsupportedVersion {
            format_version: manifest.format_version,
            schema_version: manifest.schema_version,
        }
        .into());
    }
    settings::normalize_appearance(&mut manifest.settings.appearance);

    Ok(BundleArchive {
        manifest,
        names,
        archive,
    })
}

/* -------------------------------- 指纹与图片 -------------------------------- */

fn hash_update(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

/// 主题外观 + 引用图片内容的指纹；缺图用固定标记，不参与图片字节。
/// id 与名字是标识，不参与指纹——同一外观在不同 id/名字下仍应被认成同一主题，
/// 这样重复导入才不会一直新建副本。
fn theme_fingerprint(spec: &ThemeSpec, images: &[(String, Vec<u8>)]) -> String {
    let mut canonical = spec.clone();
    canonical.id.clear();
    canonical.name.clear();
    let mut hash = FNV_OFFSET;
    if let Ok(json) = serde_json::to_vec(&canonical) {
        hash_update(&mut hash, &json);
    }
    let mut sorted = images.to_vec();
    sorted.sort_by(|left, right| left.0.cmp(&right.0));
    for (name, bytes) in sorted {
        hash_update(&mut hash, name.as_bytes());
        hash_update(&mut hash, &[0]);
        hash_update(&mut hash, &bytes);
    }
    format!("{hash:016x}")
}

fn image_dimensions_bytes(bytes: &[u8]) -> Option<(u32, u32)> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    reader.into_dimensions().ok()
}

fn target_theme_images(themes_dir: &Path, theme: &ThemeSpec) -> Vec<(String, Vec<u8>)> {
    let mut images = Vec::new();
    for image in [&theme.background.image, &theme.digit.image]
        .into_iter()
        .flatten()
    {
        if !settings::is_image_name(image) {
            continue;
        }
        let path = themes_dir.join(&theme.id).join(image);
        if let Ok(bytes) = fs::read(path) {
            if let Some((width, height)) = image_dimensions_bytes(&bytes) {
                if width.max(height) <= THEME_IMAGE_MAX_SIDE {
                    images.push((image.clone(), bytes));
                }
            }
        }
    }
    images
}

fn effective_bundle_theme(
    archive: &mut ZipArchive<File>,
    names: &HashSet<String>,
    theme: &ThemeSpec,
) -> Result<(ThemeSpec, Vec<String>, Vec<(String, Vec<u8>)>), String> {
    let mut effective = theme.clone();
    let mut missing = Vec::new();
    let mut images = Vec::new();

    for slot in 0..2 {
        let image = if slot == 0 {
            theme.background.image.clone()
        } else {
            theme.digit.image.clone()
        };
        let Some(image) = image else { continue };
        let entry = format!("themes/{}/{}", theme.id, image);
        if !names.contains(&entry) {
            missing.push(image);
            if slot == 0 {
                effective.background.image = None;
            } else {
                effective.digit.image = None;
            }
            continue;
        }
        let bytes = read_entry(archive, &entry, THEME_IMAGE_MAX_BYTES)?;
        match image_dimensions_bytes(&bytes) {
            Some((width, height)) if width.max(height) <= THEME_IMAGE_MAX_SIDE => {
                images.push((image, bytes));
            }
            _ => {
                missing.push(image);
                if slot == 0 {
                    effective.background.image = None;
                } else {
                    effective.digit.image = None;
                }
            }
        }
    }
    Ok((effective, missing, images))
}

/* -------------------------------- 准备（inspect/import 共用） -------------------------------- */

fn profile_font(appearance: &Appearance, theme_id: &str) -> String {
    appearance
        .profiles
        .get(theme_id)
        .map(|profile| profile.font.clone())
        .unwrap_or_else(|| String::from("bebas"))
}

fn prepare_bundle_with(
    settings: &Settings,
    io: &BundleIo,
    path: &Path,
) -> Result<PreparedBundle, String> {
    let mut bundle = read_bundle(path)?;
    let themes_dir = io.themes_dir.as_path();

    let target_themes = &settings.appearance.custom_themes;
    let target_fingerprints: Vec<(String, String)> = target_themes
        .iter()
        .map(|theme| {
            let images = target_theme_images(themes_dir, theme);
            (theme.id.clone(), theme_fingerprint(theme, &images))
        })
        .collect();
    let target_names: Vec<(String, String)> = target_themes
        .iter()
        .map(|theme| (theme.name.trim().to_string(), theme.id.clone()))
        .collect();
    let mut known_font_hashes: HashSet<String> = settings
        .appearance
        .fonts
        .iter()
        .map(|font| font.hash.clone())
        .collect();

    let mut custom_themes = Vec::new();
    let mut used_by: HashMap<String, Vec<String>> = HashMap::new();
    let current_theme = bundle.manifest.settings.appearance.theme.clone();

    for theme in &bundle.manifest.settings.appearance.custom_themes {
        let (effective, missing_images, images) =
            effective_bundle_theme(&mut bundle.archive, &bundle.names, theme)?;
        let fingerprint = theme_fingerprint(&effective, &images);
        let same_id = target_fingerprints
            .iter()
            .find(|(_, value)| value == &fingerprint)
            .map(|(id, _)| id.clone());
        let conflict = if same_id.is_some() {
            "same"
        } else if target_themes.iter().any(|item| item.id == theme.id) {
            "id"
        } else if target_names
            .iter()
            .any(|(name, _)| name == &theme.name.trim().to_string())
        {
            "name"
        } else {
            "none"
        };
        let font = profile_font(&bundle.manifest.settings.appearance, &theme.id);
        used_by
            .entry(font.clone())
            .or_default()
            .push(theme.id.clone());
        custom_themes.push(PreparedTheme {
            id: theme.id.clone(),
            name: theme.name.clone(),
            font,
            current: current_theme == theme.id,
            missing_images,
            conflict: String::from(conflict),
            same_id,
            spec: effective,
            images,
        });
    }

    let mut builtin_themes = Vec::new();
    for id in BUILTIN_THEME_IDS {
        let font = profile_font(&bundle.manifest.settings.appearance, id);
        used_by
            .entry(font.clone())
            .or_default()
            .push(String::from(id));
        builtin_themes.push(PreparedBuiltin {
            id: String::from(id),
            font,
            current: current_theme == id,
        });
    }

    let mut fonts = Vec::new();
    for font in &bundle.manifest.settings.appearance.fonts {
        let entry = format!("fonts/{}", font.file);
        let (bytes, missing) = if !bundle.names.contains(&entry) {
            (None, true)
        } else {
            let bytes = read_entry(&mut bundle.archive, &entry, FONT_FILE_MAX_BYTES)?;
            if bytes.len() as u64 > FONT_FILE_MAX_BYTES || font_format(&bytes).is_none() {
                (None, true)
            } else {
                (Some(bytes), false)
            }
        };
        let hash = bytes.as_ref().map(|value| fnv1a_hex(value));
        let duplicate = hash
            .as_ref()
            .map(|value| !known_font_hashes.insert(value.clone()))
            .unwrap_or(false);
        fonts.push(PreparedFont {
            font: font.clone(),
            bytes,
            missing,
            duplicate,
            used_by: used_by.get(&font.id).cloned().unwrap_or_default(),
        });
    }

    Ok(PreparedBundle {
        format_version: bundle.manifest.format_version,
        schema_version: bundle.manifest.schema_version,
        app_version: bundle.manifest.app_version.clone(),
        exported_at: bundle.manifest.exported_at,
        current_theme,
        builtin_themes,
        custom_themes,
        fonts,
        bundle: bundle.manifest.settings.clone(),
        target_custom_themes: target_themes.len(),
        target_fonts: settings.appearance.fonts.len(),
        backup_path: io.backup_path.to_string_lossy().to_string(),
    })
}

fn theme_summary(theme: &PreparedTheme) -> BundleThemeSummary {
    BundleThemeSummary {
        id: theme.id.clone(),
        name: theme.name.clone(),
        current: theme.current,
        font: theme.font.clone(),
        missing_images: theme.missing_images.clone(),
        conflict: theme.conflict.clone(),
    }
}

pub fn inspect_bundle(
    app: &AppHandle,
    settings: &Settings,
    path: &Path,
) -> Result<BundleSummary, String> {
    let io = io_from_app(app)?;
    inspect_bundle_with(settings, &io, path)
}

fn inspect_bundle_with(
    settings: &Settings,
    io: &BundleIo,
    path: &Path,
) -> Result<BundleSummary, String> {
    let prepared = prepare_bundle_with(settings, io, path)?;
    let builtin_themes = prepared
        .builtin_themes
        .iter()
        .map(|theme| BundleThemeSummary {
            id: theme.id.clone(),
            name: String::new(),
            current: theme.current,
            font: theme.font.clone(),
            missing_images: Vec::new(),
            conflict: String::from("none"),
        })
        .collect();
    let custom_themes = prepared.custom_themes.iter().map(theme_summary).collect();
    let fonts = prepared
        .fonts
        .iter()
        .map(|font| BundleFontSummary {
            id: font.font.id.clone(),
            name: font.font.name.clone(),
            used_by: font.used_by.clone(),
            missing: font.missing,
            duplicate: font.duplicate,
        })
        .collect();
    Ok(BundleSummary {
        format_version: prepared.format_version,
        schema_version: prepared.schema_version,
        app_version: prepared.app_version,
        exported_at: prepared.exported_at,
        current_theme: prepared.current_theme,
        builtin_themes,
        custom_themes,
        fonts,
        preferences: true,
        existing_custom_themes: prepared.target_custom_themes,
        existing_fonts: prepared.target_fonts,
        backup_path: prepared.backup_path,
    })
}

/* -------------------------------- 导入 -------------------------------- */

fn new_custom_id(prefix: &str, used: &HashSet<String>) -> String {
    let stamp = millis();
    for index in 0..1000 {
        let id = format!("custom-{prefix}-{stamp}-{index}");
        if !used.contains(&id) {
            return id;
        }
    }
    format!("custom-{prefix}-{stamp}-fallback")
}

fn map_profile(profile: &ClockProfile, font_map: &HashMap<String, String>) -> ClockProfile {
    let mut mapped = profile.clone();
    let is_builtin = BUILTIN_FONT_IDS.contains(&mapped.font.as_str());
    if !is_builtin {
        mapped.font = font_map
            .get(&mapped.font)
            .cloned()
            .unwrap_or_else(|| String::from("bebas"));
    }
    mapped
}

fn import_error(error: impl std::fmt::Display) -> String {
    String::from(AppError::ConfigBundleImportFailed {
        detail: error.to_string(),
    })
}

fn write_asset(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(import_error)?;
    }
    fs::write(path, bytes).map_err(import_error)
}

/// 导入先写这里，全部资产落盘后再整棵拷进 app config；中途失败不碰目标目录。
struct StagingDir {
    path: PathBuf,
}

impl StagingDir {
    fn create(base: &Path) -> Result<Self, String> {
        let path = base.join(format!(
            ".import-staging-{}-{}",
            std::process::id(),
            millis()
        ));
        fs::create_dir_all(&path).map_err(import_error)?;
        Ok(Self { path })
    }
}

impl Drop for StagingDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    if !from.exists() {
        return Ok(());
    }
    fs::create_dir_all(to).map_err(import_error)?;
    for entry in fs::read_dir(from).map_err(import_error)? {
        let entry = entry.map_err(import_error)?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if entry.file_type().map_err(import_error)?.is_dir() {
            copy_tree(&source, &target)?;
        } else {
            fs::copy(&source, &target).map_err(import_error)?;
        }
    }
    Ok(())
}

pub fn import_bundle(
    app: &AppHandle,
    current: &Settings,
    path: &Path,
    selection: &BundleSelection,
) -> Result<(Settings, ImportResult), String> {
    let io = io_from_app(app)?;
    import_bundle_with(current, &io, path, selection)
}

fn import_bundle_with(
    current: &Settings,
    io: &BundleIo,
    path: &Path,
    selection: &BundleSelection,
) -> Result<(Settings, ImportResult), String> {
    let prepared = prepare_bundle_with(current, io, path)?;
    let theme_by_id: HashMap<&str, &PreparedTheme> = prepared
        .custom_themes
        .iter()
        .map(|theme| (theme.id.as_str(), theme))
        .collect();
    let font_by_id: HashMap<&str, &PreparedFont> = prepared
        .fonts
        .iter()
        .map(|font| (font.font.id.as_str(), font))
        .collect();

    for id in &selection.theme_ids {
        if !theme_by_id.contains_key(id.as_str()) {
            return Err(format_error(format!("unknown theme id {id}")));
        }
    }
    for id in &selection.font_ids {
        if !font_by_id.contains_key(id.as_str()) {
            return Err(format_error(format!("unknown font id {id}")));
        }
    }

    let mut selected_fonts: HashSet<String> = selection.font_ids.iter().cloned().collect();
    let selected_theme_ids: HashSet<String> = selection.theme_ids.iter().cloned().collect();

    for theme in &prepared.custom_themes {
        if selected_theme_ids.contains(&theme.id) && font_by_id.contains_key(theme.font.as_str()) {
            selected_fonts.insert(theme.font.clone());
        }
    }
    if selection.builtin_profiles {
        for theme in &prepared.builtin_themes {
            if font_by_id.contains_key(theme.font.as_str()) {
                selected_fonts.insert(theme.font.clone());
            }
        }
    }

    if !selection.builtin_profiles
        && selected_theme_ids.is_empty()
        && selected_fonts.is_empty()
        && !selection.preferences
    {
        return Ok((
            current.clone(),
            ImportResult {
                themes: 0,
                fonts: 0,
                skipped: Vec::new(),
                current_theme: current.appearance.theme.clone(),
            },
        ));
    }

    // 备份必须在任何落盘前完成；失败就中止。
    export_bundle_with(current, io, &io.backup_path)
        .map_err(|error| String::from(AppError::ConfigBundleBackupFailed { detail: error }))?;

    let fonts_dir = &io.fonts_dir;
    let staging = StagingDir::create(io.themes_dir.parent().unwrap_or(io.themes_dir.as_path()))?;
    let staged_themes = staging.path.join("themes");
    let staged_fonts = staging.path.join("fonts");

    let mut next = current.clone();
    let mut next_themes = next.appearance.custom_themes.clone();
    let mut next_fonts = next.appearance.fonts.clone();
    let mut font_map: HashMap<String, String> = HashMap::new();
    let mut theme_map: HashMap<String, String> = HashMap::new();
    let mut skipped = Vec::new();
    let mut imported_themes = 0usize;
    let mut imported_fonts = 0usize;

    let mut used_theme_ids: HashSet<String> = next_themes.iter().map(|t| t.id.clone()).collect();
    let mut used_font_ids: HashSet<String> = next_fonts.iter().map(|f| f.id.clone()).collect();
    let mut used_font_files: HashSet<String> = next_fonts.iter().map(|f| f.file.clone()).collect();

    for theme in &prepared.custom_themes {
        if !selected_theme_ids.contains(&theme.id) {
            continue;
        }
        if let Some(same_id) = &theme.same_id {
            theme_map.insert(theme.id.clone(), same_id.clone());
            imported_themes += 1;
            continue;
        }
        if next_themes.len() >= CUSTOM_THEME_LIMIT {
            skipped.push(ImportSkip {
                id: theme.id.clone(),
                name: theme.name.clone(),
                reason: ImportSkipReason::ThemeLimit,
            });
            continue;
        }

        let id_conflict = next_themes.iter().any(|item| item.id == theme.id);
        let name_conflict = next_themes
            .iter()
            .any(|item| item.name.trim() == theme.name.trim());
        let target_id = if id_conflict || name_conflict {
            let id = new_custom_id("theme", &used_theme_ids);
            used_theme_ids.insert(id.clone());
            id
        } else {
            used_theme_ids.insert(theme.id.clone());
            theme.id.clone()
        };

        let mut spec = theme.spec.clone();
        spec.id = target_id.clone();
        for (name, bytes) in &theme.images {
            write_asset(&staged_themes.join(&target_id).join(name), bytes)?;
        }
        theme_map.insert(theme.id.clone(), target_id.clone());
        next_themes.push(spec);
        imported_themes += 1;
    }

    for prepared_font in &prepared.fonts {
        let font = &prepared_font.font;
        if !selected_fonts.contains(&font.id) {
            continue;
        }
        if prepared_font.missing || prepared_font.bytes.is_none() {
            font_map.insert(font.id.clone(), String::from("bebas"));
            skipped.push(ImportSkip {
                id: font.id.clone(),
                name: font.name.clone(),
                reason: ImportSkipReason::FontMissing,
            });
            continue;
        }
        let bytes = prepared_font.bytes.as_ref().unwrap();
        let hash = fnv1a_hex(bytes);
        if let Some(existing) = next_fonts.iter().find(|item| item.hash == hash) {
            font_map.insert(font.id.clone(), existing.id.clone());
            imported_fonts += 1;
            continue;
        }
        if next_fonts.len() >= CUSTOM_FONT_LIMIT {
            font_map.insert(font.id.clone(), String::from("bebas"));
            skipped.push(ImportSkip {
                id: font.id.clone(),
                name: font.name.clone(),
                reason: ImportSkipReason::FontLimit,
            });
            continue;
        }

        let format = font_format(bytes).unwrap_or("ttf");
        let id_conflict = used_font_ids.contains(&font.id);
        let mut target_id = if id_conflict {
            new_custom_id("font", &used_font_ids)
        } else {
            font.id.clone()
        };
        let mut file = format!("{target_id}.{format}");
        if used_font_files.contains(&file) || (fonts_dir.join(&file).exists() && id_conflict) {
            target_id = new_custom_id("font", &used_font_ids);
            file = format!("{target_id}.{format}");
        }
        used_font_ids.insert(target_id.clone());
        used_font_files.insert(file.clone());
        write_asset(&staged_fonts.join(&file), bytes)?;
        next_fonts.push(CustomFont {
            id: target_id.clone(),
            name: font.name.clone(),
            file,
            hash,
        });
        font_map.insert(font.id.clone(), target_id);
        imported_fonts += 1;
    }

    copy_tree(&staged_themes, &io.themes_dir)?;
    copy_tree(&staged_fonts, &io.fonts_dir)?;

    let mut next_profiles = next.appearance.profiles.clone();
    if selection.builtin_profiles {
        for id in BUILTIN_THEME_IDS {
            if let Some(profile) = prepared.bundle.appearance.profiles.get(id) {
                next_profiles.insert(String::from(id), map_profile(profile, &font_map));
            }
        }
    }
    for theme in &prepared.custom_themes {
        if !selected_theme_ids.contains(&theme.id) {
            continue;
        }
        let Some(target_id) = theme_map.get(&theme.id) else {
            continue;
        };
        if let Some(profile) = prepared.bundle.appearance.profiles.get(&theme.id) {
            next_profiles.insert(target_id.clone(), map_profile(profile, &font_map));
        }
    }

    let mut next_theme = next.appearance.theme.clone();
    if selection.builtin_profiles && BUILTIN_THEME_IDS.contains(&prepared.current_theme.as_str()) {
        next_theme = prepared.current_theme.clone();
    } else if let Some(mapped) = theme_map.get(&prepared.current_theme) {
        next_theme = mapped.clone();
    }

    next.appearance = Appearance {
        theme: next_theme,
        custom_themes: next_themes,
        fonts: next_fonts,
        profiles: next_profiles,
    };
    if selection.preferences {
        next.screensaver = prepared.bundle.screensaver.clone();
        next.launch = prepared.bundle.launch.clone();
        // 只搬托盘偏好。自启初始化标记根本不在 BundleSystem 里，
        // 所以「从别人的包导入把本机默认自启判定改掉」这件事在类型上就发生不了。
        next.system.close_to_tray = prepared.bundle.system.close_to_tray;
        next.ui = prepared.bundle.ui.clone();
    }
    settings::normalize_appearance(&mut next.appearance);
    next.schema_version = SCHEMA_VERSION;

    Ok((
        next.clone(),
        ImportResult {
            themes: imported_themes,
            fonts: imported_fonts,
            skipped,
            current_theme: next.appearance.theme,
        },
    ))
}

/* -------------------------------- 测试 -------------------------------- */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_schema_gate_accepts_older_additive_versions() {
        // 1.1.1 导出的是 schema 4；升到 5 之后它必须还能导入，
        // 否则每升一次 schema，用户手里已经导出的包就全废了。
        assert!(bundle_schema_supported(4));
        assert!(bundle_schema_supported(SCHEMA_VERSION));
        // 比程序新的包里有不认识的字段，必须拒绝。
        assert!(!bundle_schema_supported(SCHEMA_VERSION + 1));
        // 配置包出现之前没有更老的包，不假装兼容。
        assert!(!bundle_schema_supported(3));
    }

    #[test]
    fn bundle_never_carries_the_local_autostart_marker() {
        // 老包里可能带 autostartInitialized；结构里没有这个字段，读进来即丢弃。
        let parsed: BundleSystem =
            serde_json::from_str(r#"{"closeToTray":false,"autostartInitialized":true}"#).unwrap();
        assert!(!parsed.close_to_tray);
    }

    /// 真实场景：v1.1.1 导出的包是 schema 4，`system` 里还带着 autostartInitialized。
    /// 升到 schema 5 之后它必须能读进来——否则「导出配置包」这个功能会在每次升版时
    /// 把用户手里的包全部作废。
    #[test]
    fn a_schema_4_manifest_from_1_1_1_still_parses() {
        let json = r#"{
            "formatVersion": 1,
            "schemaVersion": 4,
            "appVersion": "1.1.1",
            "exportedAt": 0,
            "settings": {
                "appearance": {
                    "theme": "classic-dark",
                    "customThemes": [],
                    "fonts": [],
                    "profiles": {}
                },
                "screensaver": { "monitorMode": "primary" },
                "launch": { "noArgMode": "settings" },
                "system": { "closeToTray": false, "autostartInitialized": true },
                "ui": { "language": "system", "appearance": "system" }
            }
        }"#;
        let manifest: BundleManifest = serde_json::from_str(json).unwrap();
        assert!(bundle_schema_supported(manifest.schema_version));
        // 多出来的后端标记被忽略，托盘偏好照读。
        assert!(!manifest.settings.system.close_to_tray);
    }

    #[test]
    fn entry_validation_rejects_traversal() {
        assert!(validate_entry("manifest.json", 10).is_ok());
        assert!(validate_entry("themes/custom-a/background-1.png", 10).is_ok());
        assert!(validate_entry("fonts/custom-a.ttf", 10).is_ok());
        assert!(validate_entry("themes/../evil.png", 10).is_err());
        assert!(validate_entry("themes/custom-a/../../x.png", 10).is_err());
        assert!(validate_entry("fonts/../x.ttf", 10).is_err());
        assert!(validate_entry("other/file.png", 10).is_err());
        assert!(validate_entry("themes/", 0).is_err());
        assert!(validate_entry("themes/../", 0).is_err());
        assert!(validate_entry("fonts/", 0).is_err());
        assert!(validate_entry("other/", 0).is_err());
    }

    #[test]
    fn map_profile_missing_font_falls_back_to_bebas() {
        let profile = ClockProfile {
            font: String::from("custom-missing"),
            ..ClockProfile::default()
        };
        let mut font_map = HashMap::new();
        font_map.insert(String::from("custom-missing"), String::from("bebas"));

        let mapped = map_profile(&profile, &font_map);

        assert_eq!(mapped.font, "bebas");
    }

    #[test]
    fn fingerprint_is_stable_and_order_independent() {
        let theme = ThemeSpec {
            id: String::from("custom-a"),
            name: String::from("A"),
            background: crate::settings::BackgroundSpec {
                color: String::from("#000000"),
                opacity: 100.0,
                image: Some(String::from("background-1.png")),
                fit: crate::settings::BackgroundFit::Cover,
                dim: 0.0,
            },
            card: crate::settings::MaterialPart {
                material: crate::settings::ThemeMaterial::Matte,
                color: String::from("#111111"),
                opacity: 100.0,
                line_color: None,
            },
            digit: crate::settings::DigitSpec {
                material: crate::settings::ThemeMaterial::Matte,
                color: String::from("#ffffff"),
                opacity: 100.0,
                line_color: None,
                image: None,
            },
            date: crate::settings::FlatPart {
                color: String::from("#999999"),
                opacity: 100.0,
            },
            seam: crate::settings::FlatPart {
                color: String::from("#000000"),
                opacity: 72.0,
            },
            separator: crate::settings::FlatPart {
                color: String::from("#ffffff"),
                opacity: 62.0,
            },
        };
        let images = vec![
            (String::from("b.png"), vec![2]),
            (String::from("a.png"), vec![1]),
        ];
        let left = theme_fingerprint(&theme, &images);
        let right = theme_fingerprint(&theme, &images);
        assert_eq!(left, right);
        assert_eq!(left.len(), 16);
    }

    #[test]
    fn new_ids_avoid_existing_values() {
        let mut used = HashSet::new();
        used.insert(String::from("custom-theme-x"));
        let id = new_custom_id("theme", &used);
        assert!(id.starts_with("custom-theme-"));
        assert!(!used.contains(&id));
    }

    fn sample_theme(id: &str, name: &str, image: Option<&str>) -> ThemeSpec {
        use crate::settings::{
            BackgroundFit, BackgroundSpec, DigitSpec, FlatPart, MaterialPart, ThemeMaterial,
        };
        ThemeSpec {
            id: String::from(id),
            name: String::from(name),
            background: BackgroundSpec {
                color: String::from("#000000"),
                opacity: 100.0,
                image: image.map(String::from),
                fit: BackgroundFit::Cover,
                dim: 0.0,
            },
            card: MaterialPart {
                material: ThemeMaterial::Matte,
                color: String::from("#111111"),
                opacity: 100.0,
                line_color: None,
            },
            digit: DigitSpec {
                material: ThemeMaterial::Matte,
                color: String::from("#ffffff"),
                opacity: 100.0,
                line_color: None,
                image: None,
            },
            date: FlatPart {
                color: String::from("#999999"),
                opacity: 100.0,
            },
            seam: FlatPart {
                color: String::from("#000000"),
                opacity: 72.0,
            },
            separator: FlatPart {
                color: String::from("#ffffff"),
                opacity: 62.0,
            },
        }
    }

    /// 真 zip 往返：导出 → 导入 → 合并 → 再导一次应幂等。
    #[test]
    fn bundle_roundtrip_merges_and_is_idempotent() {
        use crate::settings::LanguagePreference;

        let root = std::env::temp_dir().join(format!(
            "flipclock-config-bundle-{}-{}",
            std::process::id(),
            millis()
        ));
        let source_themes = root.join("source/themes");
        let source_fonts = root.join("source/fonts");
        let target_themes = root.join("target/themes");
        let target_fonts = root.join("target/fonts");
        let target_backups = root.join("target/backups");
        std::fs::create_dir_all(source_themes.join("custom-a")).unwrap();
        std::fs::create_dir_all(&source_fonts).unwrap();
        std::fs::create_dir_all(&target_backups).unwrap();

        let mut png = Vec::new();
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(source_themes.join("custom-a/background-1.png"), &png).unwrap();

        let font_bytes: &[u8] = b"wOFF            ";
        std::fs::write(source_fonts.join("custom-a.woff"), font_bytes).unwrap();
        let font = CustomFont {
            id: String::from("custom-a"),
            name: String::from("测试字体"),
            file: String::from("custom-a.woff"),
            hash: fnv1a_hex(font_bytes),
        };

        let mut source = Settings::default();
        // 源配置的自启判定为 false（相当于「那台机器是全新安装」）。
        // 导入方不能被它影响：这个标记只属于本机。
        source.system.autostart_initialized = false;
        source.appearance.custom_themes = vec![sample_theme(
            "custom-a",
            "测试主题",
            Some("background-1.png"),
        )];
        source.appearance.fonts = vec![font];
        source
            .appearance
            .profiles
            .get_mut("classic-dark")
            .unwrap()
            .font = String::from("custom-a");
        source.ui.language = LanguagePreference::En;

        let source_io = BundleIo {
            themes_dir: source_themes,
            fonts_dir: source_fonts,
            backup_path: root.join("source/backups/pre-import.zip"),
            app_version: String::from("test"),
        };
        let target_io = BundleIo {
            themes_dir: target_themes.clone(),
            fonts_dir: target_fonts.clone(),
            backup_path: target_backups.join("pre-import.zip"),
            app_version: String::from("test"),
        };
        let bundle = root.join("config.zip");
        export_bundle_with(&source, &source_io, &bundle).unwrap();

        let selection = BundleSelection {
            builtin_profiles: true,
            theme_ids: vec![String::from("custom-a")],
            font_ids: vec![String::from("custom-a")],
            preferences: true,
        };
        let target = Settings::default();
        let (imported, result) =
            import_bundle_with(&target, &target_io, &bundle, &selection).unwrap();
        assert_eq!(result.themes, 1);
        assert_eq!(result.fonts, 1);
        assert!(result.skipped.is_empty());
        // 本机的自启判定不被包影响：source 是 false，导入后仍需保持 true。
        assert!(
            imported.system.autostart_initialized,
            "配置包不能把本机的默认自启判定改成「重新放开」"
        );
        assert_eq!(imported.appearance.custom_themes.len(), 1);
        assert_eq!(imported.appearance.custom_themes[0].id, "custom-a");
        assert_eq!(imported.appearance.fonts.len(), 1);
        assert_eq!(
            imported.appearance.profiles["classic-dark"].font,
            "custom-a"
        );
        assert_eq!(imported.ui.language, LanguagePreference::En);
        assert!(target_themes.join("custom-a/background-1.png").is_file());
        assert!(target_fonts.join("custom-a.woff").is_file());
        assert!(target_io.backup_path.is_file());
        assert!(
            target_themes
                .parent()
                .unwrap()
                .read_dir()
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".import-staging-"))
        );

        let (again, again_result) =
            import_bundle_with(&imported, &target_io, &bundle, &selection).unwrap();
        assert_eq!(again.appearance.custom_themes.len(), 1);
        assert_eq!(again.appearance.fonts.len(), 1);
        assert_eq!(again_result.themes, 1);
        assert_eq!(again_result.fonts, 1);

        let _ = std::fs::remove_dir_all(&root);
    }
}
