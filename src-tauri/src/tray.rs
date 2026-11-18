//! 系统托盘。屏保（`/s`）与系统预览（`/p`）不建托盘——它们只活几秒，
//! 建了反而会在任务栏通知区留下一个不该出现的图标。

use tauri::menu::{CheckMenuItem, Menu, MenuBuilder, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Wry};

use crate::{autostart, screensaver};

pub const TRAY_ID: &str = "flipclock-tray";

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let settings_item = MenuItem::with_id(app, "settings", "打开设置", true, None::<&str>)?;
    let clock_item = MenuItem::with_id(app, "clock", "全屏时钟", true, None::<&str>)?;
    let autostart_item = CheckMenuItem::with_id(
        app,
        "autostart",
        "开机自启",
        true,
        autostart::is_enabled(),
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

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
                        if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
                            let _ = tray.set_menu(Some(menu));
                        }
                    }
                    Err(message) => {
                        let _ = app.emit("autostart-failed", message);
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
