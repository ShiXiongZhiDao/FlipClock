//! 暴露给前端的命令。前端只能通过这些命令接触系统能力，
//! 不能直接读写注册表、不能拼命令字符串。

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::autostart;
use crate::install::{self, ScreenSaverStatus};
use crate::screensaver::{self, AppState};
use crate::settings::{
    self, Appearance, LaunchSettings, ScreenSaverSettings, Settings, SoundSettings, SystemSettings,
    TimeSettings,
};

/// 前端只能改这几段。注册信息（回滚点、安装路径）由后端独占，
/// 否则一次保存就会把回滚点清掉。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub appearance: Appearance,
    pub time: TimeSettings,
    pub sound: SoundSettings,
    pub screensaver: ScreenSaverSettings,
    pub launch: LaunchSettings,
    pub system: SystemSettings,
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

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    state
        .settings
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "配置状态不可用".to_string())
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
        .map_err(|_| "配置状态不可用".to_string())?;

    guard.appearance = patch.appearance;
    // 前端滑杆限了范围，这里再兜一次；边界只来自 settings::clamp_*。
    guard.appearance.glass.transparency =
        settings::clamp_glass(guard.appearance.glass.transparency);
    guard.appearance.clock_scale = settings::clamp_clock_scale(guard.appearance.clock_scale);
    guard.appearance.brightness = settings::clamp_brightness(guard.appearance.brightness);
    guard.time = patch.time;
    guard.sound = patch.sound;
    guard.sound.volume = settings::clamp_sound_volume(guard.sound.volume);
    guard.screensaver = patch.screensaver;
    guard.launch = patch.launch;
    guard.system = patch.system;

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
    ("issues", "https://github.com/ShiXiongZhiDao/FlipClock/issues"),
    ("projects", "https://github.com/ShiXiongZhiDao"),
    ("screensaver", "https://github.com/ShiXiongZhiDao/FlipClock"),
];

/// 只做查表、不启动进程，便于测试白名单本身。
fn about_link_url(key: &str) -> Result<&'static str, String> {
    ABOUT_LINKS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, url)| *url)
        .ok_or_else(|| format!("未知的外链标识：{key}"))
}

#[tauri::command]
pub fn open_about_link(key: String) -> Result<(), String> {
    // WebView 里的 target="_blank" 不会打开系统浏览器，必须走系统 shell。
    let url = about_link_url(&key)?;

    std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开浏览器失败：{e}"))
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
                "glass": { "transparency": 0.48 },
                "clockScale": 1.0,
                "digitLayout": "per-pair",
                "brightness": 0.75,
                "font": "bahnschrift"
            },
            "time": {
                "format": "24h",
                "showDate": true,
                "showSeconds": false,
                "showColon": true
            },
            "sound": { "enabled": true, "volume": 0.6 },
            "screensaver": { "monitorMode": "primary" },
            "launch": { "noArgMode": "settings" },
            "system": { "closeToTray": false }
        });

        let patch: SettingsPatch = serde_json::from_value(raw).expect("patch 载荷应能反序列化");
        assert_eq!(patch.appearance.digit_layout, DigitLayout::PerPair);
        assert_eq!(patch.appearance.font, crate::settings::DigitFont::Bahnschrift);
        assert!(patch.time.show_colon);
        assert!(!patch.system.close_to_tray);
    }

    #[test]
    fn about_links_only_open_known_destinations() {
        assert_eq!(
            super::about_link_url("website").unwrap(),
            "https://shixiongzhidao.cn"
        );
        assert!(super::about_link_url("projects")
            .unwrap()
            .contains("ShiXiongZhiDao"));
        assert!(super::about_link_url("issues").unwrap().ends_with("/issues"));
        // 前端传来的任意 URL 不能越过白名单。
        assert!(super::about_link_url("https://evil.example").is_err());
        assert!(super::about_link_url("").is_err());
    }
}
