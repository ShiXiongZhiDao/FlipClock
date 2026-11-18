mod commands;
mod install;
mod launch_mode;
mod platform;
mod screensaver;
mod settings;

use std::sync::Mutex;

use tauri::Manager;

use launch_mode::{LaunchArgs, ParsedLaunch};
use screensaver::AppState;
use settings::NoArgMode;

pub fn run() {
    let args = LaunchArgs::parse(std::env::args_os());
    if let Some(pos) = args.window_at {
        screensaver::set_window_at(pos);
    }
    if let Some(size) = args.window_size {
        screensaver::set_window_size(size);
    }

    tauri::Builder::default()
        .setup(move |app| {
            let handle = app.handle().clone();
            let loaded = settings::load(&handle);
            let no_arg_mode = loaded.launch.no_arg_mode;
            let debug_windowed = args.debug_windowed;

            handle.manage(AppState {
                settings: Mutex::new(loaded),
                session: Mutex::new(None),
            });

            let outcome = match args.intent {
                ParsedLaunch::ScreenSaver => {
                    screensaver::start_session(&handle, true, debug_windowed)
                }
                ParsedLaunch::Configure => screensaver::open_settings_window(&handle),
                ParsedLaunch::SystemPreview { parent } => {
                    screensaver::start_system_preview(&handle, parent)
                }
                ParsedLaunch::Fullscreen => {
                    screensaver::open_fullscreen_clock(&handle, debug_windowed)
                }
                ParsedLaunch::Unspecified => match no_arg_mode {
                    NoArgMode::Settings => screensaver::open_settings_window(&handle),
                    NoArgMode::Fullscreen => {
                        screensaver::open_fullscreen_clock(&handle, debug_windowed)
                    }
                },
            };

            if let Err(message) = outcome {
                // 屏保是被 Windows 拉起的，崩掉不该弹窗打扰用户；
                // 设置入口失败才需要让用户看见原因。
                eprintln!("FlipClock: {message}");
                let is_saver = matches!(
                    args.intent,
                    ParsedLaunch::ScreenSaver | ParsedLaunch::SystemPreview { .. }
                );
                if is_saver {
                    handle.exit(1);
                    return Ok(());
                }
                return Err(Box::new(std::io::Error::other(message)));
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::preview_screensaver,
            commands::end_screensaver,
            commands::screensaver_status,
            commands::install_screensaver,
            commands::restore_screensaver,
            commands::open_windows_screensaver_settings,
            commands::leave_fullscreen_clock,
            commands::ui_ready,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FlipClock");
}
