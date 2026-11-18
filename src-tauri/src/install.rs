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
/// 安装器把 `$INSTDIR` 写进卸载键的 `InstallLocation`（值带引号）。
/// 安装时允许用户改目录，所以「是不是安装版」以这里为准，而不是猜默认路径。
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\FlipClock";
const EXE_NAME: &str = "FlipClock.exe";

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

/// 主程序是不是由安装包装出来的。
///
/// 先读卸载键的 `InstallLocation`：安装器允许用户改目录，写死
/// `%LOCALAPPDATA%\FlipClock` 会把装在别处的用户误判成便携版。
/// 没有这个键（没装过、或已卸载）才退回默认安装位置比对。
///
/// 便携版（用户把 exe 拷到桌面或下载目录）一律为 false——它没有安装器可跑，
/// Windows 上的更新器无从落地，前端据此不显示「下载并安装」（ADR-0013）。
pub fn is_installed_build() -> bool {
    let Ok(exe) = current_exe() else {
        return false;
    };
    let exe_text = exe.to_string_lossy().to_string();

    if let Some(location) = platform::read_hkcu_string(UNINSTALL_KEY, "InstallLocation") {
        let dir = location.trim().trim_matches('"').trim();
        if !dir.is_empty() {
            let installed = PathBuf::from(dir).join(EXE_NAME);
            return exe_text.eq_ignore_ascii_case(&installed.to_string_lossy());
        }
    }

    match target_path() {
        Ok(target) => {
            let installed = target.with_extension("exe");
            exe_text.eq_ignore_ascii_case(&installed.to_string_lossy())
        }
        Err(_) => false,
    }
}

/// 更新之后，`.scr` 仍是上一版的字节副本——安装器只替换主程序，不管它。
/// 非屏保实例启动时把副本刷成当前这一版：注册指向不动，回滚点也不动
/// （回滚点记的是「用户原来的屏保」，与版本无关）。
///
/// 返回是否真的复制过。屏保正在跑时目标文件被占用，复制会失败；
/// 那不是需要吞掉的错误，留给系统页显示「版本不匹配」让用户点一下重装。
pub fn refresh_if_outdated() -> Result<bool, String> {
    if !is_installed_build() {
        return Ok(false);
    }

    let target = target_path()?;
    let target_text = target.to_string_lossy().to_string();
    let active = platform::read_desktop_string(SCRNSAVE_EXE).filter(|value| !value.is_empty());
    let points_here = active
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case(&target_text))
        .unwrap_or(false);

    // 没装过组件（注册不指向我们）就别自作主张替用户装一个。
    if !points_here || !target.exists() {
        return Ok(false);
    }

    let source = current_exe()?;
    if files_equal(&source, &target) {
        return Ok(false);
    }

    replace_component(&source, &target)?;
    Ok(true)
}

/// 先写临时文件再改名。
///
/// 直接覆盖会在写入中途留下半截 `.scr`，而 Windows 随时可能拉起那个文件；
/// 同卷改名是原子的，最坏情况是旧版本继续留着，而不是一个跑不起来的屏保。
fn replace_component(source: &PathBuf, target: &PathBuf) -> Result<(), String> {
    let temp = target.with_extension("scr.new");
    fs::copy(source, &temp).map_err(|e| format!("写入屏保组件失败：{e}"))?;
    if let Err(error) = fs::rename(&temp, target) {
        let _ = fs::remove_file(&temp);
        return Err(format!("替换屏保组件失败：{error}"));
    }
    Ok(())
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
        replace_component(&source, &target)?;
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

    // 只在「当前选的还是我们」时动注册表。用户后来换了别的屏保，
    // 这里再按旧回滚点写回去，等于把他的新选择也一起抹掉。
    let target_text = target_path()?.to_string_lossy().to_string();
    let still_ours = platform::read_desktop_string(SCRNSAVE_EXE)
        .filter(|value| !value.is_empty())
        .map(|value| value.eq_ignore_ascii_case(&target_text))
        .unwrap_or(false);

    if still_ours {
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
