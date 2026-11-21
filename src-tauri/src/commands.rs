//! 暴露给前端的命令。前端只能通过这些命令接触系统能力，
//! 不能直接读写注册表、不能拼命令字符串。

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::autostart;
use crate::errors::AppError;
use crate::install::{self, ScreenSaverStatus};
use crate::screensaver::{self, AppState};
use crate::settings::{
    self, Appearance, LaunchSettings, ScreenSaverSettings, Settings, SystemSettings, UiSettings,
};

/// 前端只能改这几段。注册信息（回滚点、安装路径）由后端独占，
/// 否则一次保存就会把回滚点清掉。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub appearance: Appearance,
    pub screensaver: ScreenSaverSettings,
    pub launch: LaunchSettings,
    pub system: SystemSettings,
    pub ui: UiSettings,
}

/// 前端准备好之后调用：这时才把设置窗口显示出来，避免闪一下不透明底色。
#[tauri::command]
pub fn autostart_status() -> bool {
    autostart::is_enabled()
}

/// 开关写的是注册表，不是配置：立刻生效，且以实际写入结果为准。
#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    autostart::set_enabled(enabled)?;
    // 设置页与托盘菜单共用同一份事实，两边都要刷新。
    let _ = app.emit("autostart-changed", enabled);
    Ok(())
}

#[tauri::command]
pub fn ui_ready(window: tauri::WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

/// 前端把生效语言（只有 zh/en）推过来，Rust 重建托盘菜单。
/// 偏好三态不进 Rust——它不知道系统语言，归并归前端做。
#[tauri::command]
pub fn set_locale(app: AppHandle, locale: String) -> Result<(), String> {
    let language = match locale.as_str() {
        "zh" => crate::tray::Language::Zh,
        "en" => crate::tray::Language::En,
        _ => {
            return Err(AppError::SetLocaleFailed {
                detail: format!("unsupported locale: {locale}"),
            }
            .into());
        }
    };
    crate::tray::set_language(&app, language).map_err(|e| {
        AppError::SetLocaleFailed {
            detail: e.to_string(),
        }
        .into()
    })
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    state
        .settings
        .lock()
        .map(|s| s.clone())
        .map_err(|_| AppError::ConfigUnavailable.into())
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<Settings, String> {
    let mut guard = state
        .settings
        .lock()
        .map_err(|_| String::from(AppError::ConfigUnavailable))?;

    guard.appearance = patch.appearance;
    // 自定义主题在这里收敛：截断数量、丢弃非法项、clamp 不透明度、修正悬空 id，
    // 并补齐每个主题的钟面配置（ADR-0022）。
    settings::normalize_appearance(&mut guard.appearance);
    guard.screensaver = patch.screensaver;
    guard.launch = patch.launch;
    guard.system = patch.system;
    guard.ui = patch.ui;

    settings::save(&app, &guard)?;
    Ok(guard.clone())
}

#[tauri::command]
pub async fn preview_screensaver(app: AppHandle) -> Result<(), String> {
    // 必须是 async 命令：同步命令在 Tauri 的主线程上执行，而创建窗口要等主线程
    // 处理事件循环，`WebviewWindowBuilder::build()` 会直接卡死（窗口只建到一半）。
    screensaver::start_session(&app, false, false)
}

#[tauri::command]
pub fn end_screensaver(app: AppHandle) {
    screensaver::end_session(&app);
}

#[tauri::command]
pub fn screensaver_status(app: AppHandle) -> Result<ScreenSaverStatus, String> {
    install::status(&app)
}

/// 前端据此决定「下载并安装」按钮出不出现：只有安装版能被安装器替换，
/// 便携版的更新只能自己下载新包（ADR-0013）。
#[tauri::command]
pub fn is_installed_build() -> bool {
    install::is_installed_build()
}

#[tauri::command]
pub fn install_screensaver(app: AppHandle) -> Result<ScreenSaverStatus, String> {
    install::install(&app)
}

#[tauri::command]
pub fn restore_screensaver(app: AppHandle) -> Result<ScreenSaverStatus, String> {
    install::restore(&app)
}

#[tauri::command]
pub fn open_windows_screensaver_settings() -> Result<(), String> {
    install::open_windows_settings()
}

/// 关于页的外链。前端只传 key，URL 固定在这里：不把 URL 交给前端拼，
/// 否则一次 XSS 就能拿它当任意程序启动器。
const ABOUT_LINKS: &[(&str, &str)] = &[
    ("website", "https://shixiongzhidao.cn"),
    (
        "issues",
        "https://github.com/ShiXiongZhiDao/FlipClock/issues",
    ),
    ("projects", "https://github.com/ShiXiongZhiDao"),
    ("screensaver", "https://github.com/ShiXiongZhiDao/FlipClock"),
];

/// 只做查表、不启动进程，便于测试白名单本身。
fn about_link_url(key: &str) -> Result<&'static str, String> {
    ABOUT_LINKS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, url)| *url)
        .ok_or_else(|| {
            AppError::UnknownAboutLink {
                key: key.to_string(),
            }
            .into()
        })
}

#[tauri::command]
pub fn open_about_link(key: String) -> Result<(), String> {
    // WebView 里的 target="_blank" 不会打开系统浏览器，必须走系统 shell。
    let url = about_link_url(&key)?;

    let mut command = std::process::Command::new("cmd");
    command.args(["/c", "start", "", url]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // cmd.exe 是控制台程序；GUI 父进程不隐藏它，打开链接时就会闪一下黑窗。
        command.creation_flags(0x0800_0000);
    }
    command.spawn().map(|_| ()).map_err(|e| {
        AppError::OpenBrowserFailed {
            detail: e.to_string(),
        }
        .into()
    })
}

/// 从全屏时钟回到设置窗口。先开设置窗口再关全屏窗口，
/// 否则关掉唯一窗口会直接结束进程。
#[tauri::command]
pub async fn leave_fullscreen_clock(app: AppHandle) -> Result<(), String> {
    // 同 preview_screensaver：这里可能现场创建设置窗口，不能占着主线程。
    screensaver::open_settings_window(&app)?;
    if let Some(window) = app.get_webview_window("clock") {
        let _ = window.close();
    }
    Ok(())
}

/* ------------------------------- 主题图片（ADR-0020） ------------------------------ */

/// 主题图片上限。32MB 是文件大小；解码后的内存通常是它的三到四倍，
/// 屏保在多显示器上还会同时加载，所以另有单边像素上限。
pub(crate) const THEME_IMAGE_MAX_BYTES: u64 = 32 * 1024 * 1024;
pub(crate) const THEME_IMAGE_MAX_SIDE: u32 = 8192;

/// 主题 id 会被拼进文件路径，必须只允许小写字母、数字与连字符：
/// 这条同时挡住 `..`、斜杠与盘符。判据在 settings 里，配置校验与图片导入共用一份。
fn is_safe_kind(kind: &str) -> bool {
    matches!(kind, "background" | "digit")
}

/// 打开主题编辑的全屏预览窗口。
///
/// **必须是 async**：同步命令跑在 Tauri 主线程上，而建窗口要等主线程回到事件循环，
/// `WebviewWindowBuilder::build()` 会原地死锁——表现就是「点一下，整个应用卡住」。
/// 同一个坑 `preview_screensaver` 上面已经踩过一次并留了注释。
#[tauri::command]
pub async fn open_theme_preview(app: AppHandle, payload: String) -> Result<(), String> {
    screensaver::open_theme_preview(&app, payload)?;
    Ok(())
}

#[tauri::command]
pub fn get_theme_preview() -> Result<Option<String>, String> {
    // 预览窗口被直接关掉再打开时可能读不到草稿，返回 None 让前端自己退出。
    Ok(screensaver::take_theme_preview())
}

/// 关窗同样走 async：销毁窗口也要事件循环参与。
#[tauri::command]
pub async fn close_theme_preview(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(screensaver::MODE_THEME_PREVIEW) {
        // 关窗失败不能吞：以前这里是 `let _ = window.close()`，失败后照样发
        // theme-preview-closed，前端于是 resetStudio() 把草稿清了——窗口却留在
        // 屏上。用户看到的是「点了没反应」，丢掉的却是整场草稿。
        // 见 ADR-0020 D19「创建与关闭失败都必须留痕」。
        if let Err(error) = window.close() {
            eprintln!("FlipClock: 关闭全屏配置窗口失败 {error}");
            return Err(AppError::ThemePreviewCloseFailed {
                detail: error.to_string(),
            }
            .into());
        }
    }
    // 事件只在窗口确实关掉之后发。提前发等于对设置窗口撒谎：它会白刷一次配置，
    // 而预览窗口还在最前面。
    let _ = app.emit("theme-preview-closed", ());
    Ok(())
}

#[tauri::command]
pub fn themes_dir_path(app: AppHandle) -> Result<String, String> {
    let dir = settings::themes_dir(&app)?;
    Ok(dir.to_string_lossy().to_string())
}

/// 把用户选中的图片复制进主题目录，返回文件名（配置文件里只存文件名）。
#[tauri::command]
pub fn import_theme_image(
    app: AppHandle,
    theme_id: String,
    source_path: String,
    kind: String,
) -> Result<String, String> {
    if !settings::is_safe_theme_id(&theme_id) {
        return Err(AppError::InvalidThemeId { detail: theme_id }.into());
    }
    if !is_safe_kind(&kind) {
        return Err(AppError::ThemeImageUnsupported { detail: kind }.into());
    }

    let source = std::path::Path::new(&source_path);
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| AppError::ThemeImageUnsupported {
            detail: String::from("no-extension"),
        })?;
    if !matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp") {
        return Err(AppError::ThemeImageUnsupported { detail: extension }.into());
    }

    let meta = std::fs::metadata(source).map_err(|error| AppError::ThemeImageUnsupported {
        detail: error.to_string(),
    })?;
    if meta.len() > THEME_IMAGE_MAX_BYTES {
        return Err(AppError::ThemeImageTooLarge { limit_mb: 32 }.into());
    }

    // 只读文件头拿尺寸，不解码整张图。
    match image::image_dimensions(source) {
        Ok((width, height)) if width.max(height) > THEME_IMAGE_MAX_SIDE => {
            return Err(AppError::ThemeImageTooManyPixels {
                limit_side: THEME_IMAGE_MAX_SIDE,
            }
            .into());
        }
        Ok(_) => {}
        Err(error) => {
            return Err(AppError::ThemeImageUnsupported {
                detail: error.to_string(),
            }
            .into());
        }
    }

    let dir = settings::themes_dir(&app)?.join(&theme_id);
    std::fs::create_dir_all(&dir).map_err(|error| AppError::ThemeImageCopyFailed {
        detail: error.to_string(),
    })?;

    // 文件名带时间戳：换图时不覆盖旧文件，缓存也不会指向同一个名字。
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    let name = format!("{kind}-{stamp}.{extension}");
    std::fs::copy(source, dir.join(&name)).map_err(|error| AppError::ThemeImageCopyFailed {
        detail: error.to_string(),
    })?;
    Ok(name)
}

#[tauri::command]
pub fn remove_theme_image(
    app: AppHandle,
    theme_id: String,
    filename: String,
) -> Result<(), String> {
    if !settings::is_safe_theme_id(&theme_id) {
        return Err(AppError::InvalidThemeId { detail: theme_id }.into());
    }
    if !settings::is_image_name(&filename) {
        return Err(AppError::ThemeImageUnsupported { detail: filename }.into());
    }
    let path = settings::themes_dir(&app)?.join(&theme_id).join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|error| AppError::ThemeImageCopyFailed {
            detail: error.to_string(),
        })?;
    }
    Ok(())
}

/// 删除整个主题目录。删主题时必须调，否则图片会变成孤儿文件。
#[tauri::command]
pub fn delete_theme_dir(app: AppHandle, theme_id: String) -> Result<(), String> {
    if !settings::is_safe_theme_id(&theme_id) {
        return Err(AppError::InvalidThemeId { detail: theme_id }.into());
    }
    let dir = settings::themes_dir(&app)?.join(&theme_id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|error| AppError::ThemeImageCopyFailed {
            detail: error.to_string(),
        })?;
    }
    Ok(())
}

/* ------------------------------- 导入字体（ADR-0026） ------------------------------ */

const FONT_FILE_MAX_MB: u32 = 32;
pub(crate) const FONT_FILE_MAX_BYTES: u64 = FONT_FILE_MAX_MB as u64 * 1024 * 1024;

#[tauri::command]
pub fn fonts_dir_path(app: AppHandle) -> Result<String, String> {
    let dir = settings::fonts_dir(&app)?;
    Ok(dir.to_string_lossy().to_string())
}

/// 只认文件头，不认扩展名：改名后的 .exe 不会因为后缀是 .ttf 就混进来。
pub(crate) fn font_format(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x00, 0x01, 0x00, 0x00]) || bytes.starts_with(b"true") {
        Some("ttf")
    } else if bytes.starts_with(b"OTTO") {
        Some("otf")
    } else if bytes.starts_with(b"wOFF") {
        Some("woff")
    } else if bytes.starts_with(b"wOF2") {
        Some("woff2")
    } else {
        None
    }
}

/// 只用于导入去重，不是安全哈希；固定实现，跨版本稳定即可。
pub(crate) fn fnv1a_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn default_font_name(source: &std::path::Path) -> String {
    let name = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("字体");
    name.chars().take(settings::FONT_NAME_MAX_CHARS).collect()
}

/// 把用户选中的字体复制进字体库目录，返回文件名 / 内容 hash / 默认显示名。
#[tauri::command]
pub fn import_font(
    app: AppHandle,
    font_id: String,
    source_path: String,
) -> Result<settings::CustomFont, String> {
    if !settings::is_safe_font_id(&font_id) {
        return Err(AppError::InvalidFontId { detail: font_id }.into());
    }

    let source = std::path::Path::new(&source_path);
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| AppError::FontUnsupported {
            detail: String::from("no-extension"),
        })?;
    if !matches!(extension.as_str(), "ttf" | "otf" | "woff" | "woff2") {
        return Err(AppError::FontUnsupported { detail: extension }.into());
    }

    let meta = std::fs::metadata(source).map_err(|error| AppError::FontUnsupported {
        detail: error.to_string(),
    })?;
    if meta.len() > FONT_FILE_MAX_BYTES {
        return Err(AppError::FontTooLarge {
            limit_mb: FONT_FILE_MAX_MB,
        }
        .into());
    }
    if meta.len() < 12 {
        return Err(AppError::FontUnsupported {
            detail: String::from("too-short"),
        }
        .into());
    }

    let bytes = std::fs::read(source).map_err(|error| AppError::FontUnsupported {
        detail: error.to_string(),
    })?;
    let format = font_format(&bytes).ok_or_else(|| AppError::FontUnsupported {
        detail: String::from("magic"),
    })?;

    let dir = settings::fonts_dir(&app)?;
    let file = format!("{font_id}.{format}");
    let destination = dir.join(&file);
    if destination.exists() {
        return Err(AppError::FontUnsupported {
            detail: String::from("already-exists"),
        }
        .into());
    }
    std::fs::write(&destination, &bytes).map_err(|error| AppError::FontCopyFailed {
        detail: error.to_string(),
    })?;

    Ok(settings::CustomFont {
        id: font_id,
        name: default_font_name(source),
        file,
        hash: fnv1a_hex(&bytes),
    })
}

/// 删除一款导入字体的文件。配置条目的删除由前端负责；文件不存在时视为成功。
#[tauri::command]
pub fn remove_font(app: AppHandle, font_id: String, filename: String) -> Result<(), String> {
    if !settings::is_font_file_for_id(&font_id, &filename) {
        return Err(AppError::FontUnsupported { detail: filename }.into());
    }
    let path = settings::fonts_dir(&app)?.join(&filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|error| AppError::FontCopyFailed {
            detail: error.to_string(),
        })?;
    }
    Ok(())
}

/// 导出全量配置包（ADR-0027）。前端先 flush，保存对话框给出目标路径。
#[tauri::command]
pub fn export_config_bundle(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let settings = state
        .settings
        .lock()
        .map(|value| value.clone())
        .map_err(|_| String::from(AppError::ConfigUnavailable))?;
    crate::config_bundle::export_bundle(&app, &settings, std::path::Path::new(&path))
}

/// 只读配置包，返回选择摘要；不写盘、不改配置。
#[tauri::command]
pub fn inspect_config_bundle(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<crate::config_bundle::BundleSummary, String> {
    let settings = state
        .settings
        .lock()
        .map(|value| value.clone())
        .map_err(|_| String::from(AppError::ConfigUnavailable))?;
    crate::config_bundle::inspect_bundle(&app, &settings, std::path::Path::new(&path))
}

/// 归档：先备份当前配置，再合并选中的内容、保存并更新内存状态。
#[tauri::command]
pub fn import_config_bundle(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    selection: crate::config_bundle::BundleSelection,
) -> Result<crate::config_bundle::ImportResult, String> {
    let current = state
        .settings
        .lock()
        .map(|value| value.clone())
        .map_err(|_| String::from(AppError::ConfigUnavailable))?;
    let (next, result) = crate::config_bundle::import_bundle(
        &app,
        &current,
        std::path::Path::new(&path),
        &selection,
    )?;
    settings::save(&app, &next)?;
    let mut guard = state
        .settings
        .lock()
        .map_err(|_| String::from(AppError::ConfigUnavailable))?;
    *guard = next;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::DigitLayout;

    /// 前端发来的 patch 形状是手写 JSON，字段名或枚举值错一个就会静默回默认值，
    /// 所以这里把线上的实际载荷形态钉住。
    #[test]
    fn patch_accepts_the_frontend_payload_shape() {
        let raw = serde_json::json!({
            "appearance": {
                "theme": "classic-dark",
                "fonts": [
                    { "id": "custom-a", "name": "测试字体", "file": "custom-a.ttf", "hash": "abc123" }
                ],
                "profiles": {
                    "classic-dark": {
                        "format": "12h",
                        "showDate": true,
                        "showSeconds": false,
                        "showColon": true,
                        "digitLayout": "per-pair",
                        "font": "bahnschrift",
                        "clockScale": 1.0,
                        "brightness": 0.75,
                        "sound": { "enabled": true, "volume": 0.6 }
                    }
                }
            },
            "screensaver": { "monitorMode": "primary" },
            "launch": { "noArgMode": "settings" },
            "system": { "closeToTray": false },
            "ui": { "language": "system" }
        });

        let patch: SettingsPatch = serde_json::from_value(raw).expect("patch 载荷应能反序列化");
        let profile = &patch.appearance.profiles["classic-dark"];
        assert_eq!(profile.format, crate::settings::TimeFormat::H12);
        assert_eq!(profile.digit_layout, DigitLayout::PerPair);
        assert_eq!(profile.font, "bahnschrift");
        assert_eq!(patch.appearance.fonts.len(), 1);
        assert_eq!(patch.appearance.fonts[0].file, "custom-a.ttf");
        assert!(profile.show_colon);
        assert!(!patch.system.close_to_tray);
        assert_eq!(
            patch.ui.language,
            crate::settings::LanguagePreference::System
        );
    }

    #[test]
    fn font_format_uses_magic_bytes_not_the_extension() {
        assert_eq!(
            font_format(&[0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            Some("ttf")
        );
        assert_eq!(font_format(b"OTTO............"), Some("otf"));
        assert_eq!(font_format(b"wOFF............"), Some("woff"));
        assert_eq!(font_format(b"wOF2............"), Some("woff2"));
        assert_eq!(font_format(b"ttcf............"), None);
        assert_eq!(font_format(b"not a font....."), None);
    }

    #[test]
    fn fnv_hash_is_stable() {
        // 空输入就是 FNV-1a 的 offset basis；去重口径跨版本不能漂。
        assert_eq!(fnv1a_hex(b""), "cbf29ce484222325");
    }

    #[test]
    fn about_links_only_open_known_destinations() {
        assert_eq!(
            super::about_link_url("website").unwrap(),
            "https://shixiongzhidao.cn"
        );
        assert!(
            super::about_link_url("projects")
                .unwrap()
                .contains("ShiXiongZhiDao")
        );
        assert!(
            super::about_link_url("issues")
                .unwrap()
                .ends_with("/issues")
        );
        // 前端传来的任意 URL 不能越过白名单。
        assert!(super::about_link_url("https://evil.example").is_err());
        assert!(super::about_link_url("").is_err());
    }
}
