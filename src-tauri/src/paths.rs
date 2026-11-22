//! 屏保相关路径：组件位置与独立 WebView2 数据目录。

use std::fs;
use std::path::PathBuf;

use crate::errors::AppError;

const SAVER_DATA_DIR: &str = "saver-webview";

/// Windows 屏保组件的稳定安装位置。
pub fn screensaver_target() -> Result<PathBuf, String> {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .map_err(|_| String::from(AppError::NoLocalAppData))?;
    Ok(base.join("FlipClock").join("FlipClock.scr"))
}

/// 屏保进程专用的 WebView2 用户数据目录。
///
/// 与主应用的默认目录分开：主应用设置/时钟窗口还活着时，Windows 拉起的屏保
/// 会话共用默认目录会创建失败（0x800700AA）。见 ADR-0032 修订。
/// 主目录不可写时退回 `%TEMP%\FlipClock-saver-<pid>`，避免再次静默起不来。
pub fn saver_data_dir() -> Result<PathBuf, String> {
    let primary = screensaver_target()?.with_file_name(SAVER_DATA_DIR);
    if fs::create_dir_all(&primary).is_ok() {
        return Ok(primary);
    }

    let fallback = std::env::temp_dir().join(format!("FlipClock-saver-{}", std::process::id()));
    fs::create_dir_all(&fallback)
        .map_err(|e| format!("无法创建屏保 WebView2 数据目录：{e}"))?;
    Ok(fallback)
}