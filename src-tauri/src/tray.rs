//! 系统托盘。屏保（`/s`）与系统预览（`/p`）不建托盘——它们只活几秒，
//! 建了反而会在任务栏通知区留下一个不该出现的图标。

use std::sync::{Mutex, OnceLock};

use tauri::menu::{CheckMenuItem, Menu, MenuBuilder, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Wry};

use crate::{autostart, screensaver};

pub const TRAY_ID: &str = "flipclock-tray";

/// 托盘只认生效语言（zh/en）。偏好三态不进 Rust——前端归并完再推过来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Zh,
    En,
}

static LANGUAGE: OnceLock<Mutex<Language>> = OnceLock::new();

fn language() -> Language {
    LANGUAGE
        .get_or_init(|| Mutex::new(Language::Zh))
        .lock()
        .map(|guard| *guard)
        .unwrap_or(Language::Zh)
}

/// 四项菜单文字。这是 Rust 侧唯一的用户可见文案——其余错误串都走错误码（ADR-0018）。
/// 只有四项，两张表比引入完整 i18n 层便宜得多。
const MENU_TEXT: [(&str, &str, &str); 4] = [
    ("打开设置", "Open Settings", "settings"),
    ("全屏时钟", "Fullscreen Clock", "clock"),
    ("开机自启", "Autostart", "autostart"),
    ("退出", "Quit", "quit"),
];

fn menu_label(index: usize, language: Language) -> &'static str {
    let (zh, en, _) = MENU_TEXT[index];
    match language {
        Language::Zh => zh,
        Language::En => en,
    }
}

/// 重建托盘菜单。前端推语言、自启状态变化都走这里。
pub fn refresh_menu(app: &AppHandle) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(build_menu(app)?))?;
    }
    Ok(())
}

/// 由 set_locale 命令调用。语言没变就不重建菜单——省一次无谓的 UI 抖动。
pub fn set_language(app: &AppHandle, next: Language) -> tauri::Result<()> {
    let changed = LANGUAGE
        .get_or_init(|| Mutex::new(Language::Zh))
        .lock()
        .map(|mut guard| {
            if *guard == next {
                false
            } else {
                *guard = next;
                true
            }
        })
        .unwrap_or(true);
    if changed {
        refresh_menu(app)?;
    }
    Ok(())
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let lang = language();
    let settings_item = MenuItem::with_id(
        app,
        MENU_TEXT[0].2,
        menu_label(0, lang),
        true,
        None::<&str>,
    )?;
    let clock_item =
        MenuItem::with_id(app, MENU_TEXT[1].2, menu_label(1, lang), true, None::<&str>)?;
    let autostart_item = CheckMenuItem::with_id(
        app,
        MENU_TEXT[2].2,
        menu_label(2, lang),
        true,
        autostart::is_enabled(),
        None::<&str>,
    )?;
    let quit_item =
        MenuItem::with_id(app, MENU_TEXT[3].2, menu_label(3, lang), true, None::<&str>)?;

    MenuBuilder::new(app)
        .items(&[&settings_item, &clock_item])
        .separator()
        .items(&[&autostart_item])
        .separator()
        .items(&[&quit_item])
        .build()
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("FlipClock")
        // 左键唤出设置，右键才是菜单；否则左键会同时弹菜单和开窗。
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "settings" => {
                let _ = screensaver::open_settings_window(app);
            }
            "clock" => {
                let _ = screensaver::open_fullscreen_clock(app, false);
            }
            "autostart" => {
                let next = !autostart::is_enabled();
                match autostart::set_enabled(next) {
                    Ok(()) => {
                        // 勾选状态与设置页共用同一份事实（注册表），两边同时刷新。
                        let _ = app.emit("autostart-changed", next);
                        let _ = refresh_menu(app);
                    }
                    Err(error) => {
                        // 事件负载也是稳定码：将来前端接上监听时直接走 errorText()。
                        let _ = app.emit(
                            "autostart-failed",
                            crate::errors::AppError::AutostartWriteFailed {
                                key: next.to_string(),
                                detail: error,
                            }
                            .as_string(),
                        );
                    }
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = screensaver::open_settings_window(tray.app_handle());
            }
        });

    // Windows 上 default_window_icon 读的是内嵌的 .ico 资源（与任务栏同一份），
    // 托盘不再单独维护一张图。
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}
