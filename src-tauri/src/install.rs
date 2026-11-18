//! 屏保组件的安装、注册状态与回滚。
//!
//! 只改当前用户（`HKCU\Control Panel\Desktop`），不动 `System32`，不申请管理员权限。

use std::fs;
use std::io::Read;
use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::platform;
use crate::screensaver::AppState;
use crate::settings;

const SCRNSAVE_EXE: &str = "SCRNSAVE.EXE";
const SCREEN_SAVE_ACTIVE: &str = "ScreenSaveActive";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenSaverStatus {
    /// notInstalled | installed | missing | outdated
    pub state: &'static str,
    pub target_path: String,
    pub active_path: Option<String>,
    pub rollback_available: bool,
}

pub fn target_path() -> Result<PathBuf, String> {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .map_err(|_| "无法定位 LOCALAPPDATA".to_string())?;
    Ok(base.join("FlipClock").join("FlipClock.scr"))
}

fn current_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("无法定位当前程序：{e}"))
}

/// 分块比较两个文件是否逐字节相同，避免为了判断版本而引入哈希依赖。
fn files_equal(a: &PathBuf, b: &PathBuf) -> bool {
    let (Ok(meta_a), Ok(meta_b)) = (fs::metadata(a), fs::metadata(b)) else {
        return false;
    };
    if meta_a.len() != meta_b.len() {
        return false;
    }

    let (Ok(mut file_a), Ok(mut file_b)) = (fs::File::open(a), fs::File::open(b)) else {
        return false;
    };

    let mut buf_a = vec![0u8; 64 * 1024];
    let mut buf_b = vec![0u8; 64 * 1024];
    loop {
        let read_a = file_a.read(&mut buf_a).unwrap_or(0);
        let read_b = file_b.read(&mut buf_b).unwrap_or(0);
        if read_a != read_b {
            return false;
        }
        if read_a == 0 {
            return true;
        }
        if buf_a[..read_a] != buf_b[..read_b] {
            return false;
        }
    }
}

pub fn status(app: &AppHandle) -> Result<ScreenSaverStatus, String> {
    let target = target_path()?;
    let target_text = target.to_string_lossy().to_string();
    let active = platform::read_desktop_string(SCRNSAVE_EXE).filter(|v| !v.is_empty());
    let rollback_available = app
        .state::<AppState>()
        .settings
        .lock()
        .ok()
        .map(|s| s.registration.rollback_scrnsave_exe.is_some())
        .unwrap_or(false);

    let points_here = active
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case(&target_text))
        .unwrap_or(false);

    let state = if !points_here {
        "notInstalled"
    } else if !target.exists() {
        "missing"
    } else {
        match current_exe() {
            // 当前进程就是那份 .scr 时，比较自己是恒等的，视为已安装。
            Ok(exe) if files_equal(&exe, &target) => "installed",
            _ => "outdated",
        }
    };

    Ok(ScreenSaverStatus {
        state,
        target_path: target_text,
        active_path: active,
        rollback_available,
    })
}

pub fn install(app: &AppHandle) -> Result<ScreenSaverStatus, String> {
    let target = target_path()?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建安装目录失败：{e}"))?;
    }

    let source = current_exe()?;
    if source != target && !files_equal(&source, &target) {
        fs::copy(&source, &target).map_err(|e| format!("复制屏保组件失败：{e}"))?;
    }

    let target_text = target.to_string_lossy().to_string();
    let previous = platform::read_desktop_string(SCRNSAVE_EXE);
    let previous_active = platform::read_desktop_string(SCREEN_SAVE_ACTIVE);

    {
        let state = app.state::<AppState>();
        let mut guard = state
            .settings
            .lock()
            .map_err(|_| "配置状态不可用".to_string())?;

        // 只有当当前指向不是我们自己时，才更新回滚点，避免把回滚点覆盖成自己。
        let already_ours = previous
            .as_deref()
            .map(|value| value.eq_ignore_ascii_case(&target_text))
            .unwrap_or(false);
        if !already_ours {
            guard.registration.rollback_scrnsave_exe = previous.clone();
            guard.registration.rollback_screen_save_active = previous_active.clone();
        }
        guard.registration.installed_path = Some(target_text.clone());
        settings::save(app, &guard)?;
    }

    platform::write_desktop_string(SCRNSAVE_EXE, &target_text)?;
    platform::write_desktop_string(SCREEN_SAVE_ACTIVE, "1")?;

    status(app)
}

pub fn restore(app: &AppHandle) -> Result<ScreenSaverStatus, String> {
    let (rollback_exe, rollback_active) = {
        let state = app.state::<AppState>();
        let guard = state
            .settings
            .lock()
            .map_err(|_| "配置状态不可用".to_string())?;
        (
            guard.registration.rollback_scrnsave_exe.clone(),
            guard.registration.rollback_screen_save_active.clone(),
        )
    };

    match rollback_exe.as_deref().map(str::trim) {
        Some(value) if !value.is_empty() => {
            platform::write_desktop_string(SCRNSAVE_EXE, value)?;
        }
        _ => {
            // 没有回滚点：清掉本程序的指向，让用户自己重新选，而不是留一个假值。
            platform::delete_desktop_value(SCRNSAVE_EXE)?;
        }
    }

    if let Some(active) = rollback_active.as_deref() {
        if !active.is_empty() {
            platform::write_desktop_string(SCREEN_SAVE_ACTIVE, active)?;
        }
    }

    {
        let state = app.state::<AppState>();
        let mut guard = state
            .settings
            .lock()
            .map_err(|_| "配置状态不可用".to_string())?;
        guard.registration.rollback_scrnsave_exe = None;
        guard.registration.rollback_screen_save_active = None;
        guard.registration.installed_path = None;
        settings::save(app, &guard)?;
    }

    status(app)
}

pub fn open_windows_settings() -> Result<(), String> {
    std::process::Command::new("rundll32.exe")
        .args(["shell32.dll,Control_RunDLL", "desk.cpl,,@screensaver"])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开系统屏保设置失败：{e}"))
}
