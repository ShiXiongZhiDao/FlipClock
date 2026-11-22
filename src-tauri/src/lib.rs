mod autostart;
mod commands;
mod config_bundle;
mod errors;
mod install;
mod launch_mode;
mod platform;
mod screensaver;
mod settings;
mod tray;

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

    // 屏保与系统预览必须绕过单实例：托盘常驻时，否则 Windows 拉起的 /s 会被
    // 已有实例挡回去，屏保永远起不来。
    let is_saver = matches!(
        args.intent,
        ParsedLaunch::ScreenSaver | ParsedLaunch::SystemPreview { .. }
    );
    // 卸载器的钩子也走独立进程：托盘实例还在跑的时候，它必须能把屏保选择还回去。
    let is_headless_restore = matches!(args.intent, ParsedLaunch::RestoreScreensaver);
    // 只有「正常启动」才做应用级副作用：单实例、托盘、自启判定、刷新屏保组件。
    // 屏保进程与卸载钩子都是系统拉起来只干一件事的；顺手做这些会改到用户设置
    // ——2026-11-22 审出的真事故：屏保进程把 Run 键改写成了 `FlipClock.scr --tray`。
    let is_normal_start = !is_saver && !is_headless_restore;

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        // 匿名使用统计（ADR-0019）。App Key 由构建环境注入；未配置时传空串，
        // 插件会静默禁用所有事件。这样开源仓库不需要藏 key，本地开发不发数据。
        .plugin(
            tauri_plugin_aptabase::Builder::new(
                option_env!("FLIPCLOCK_APTABASE_KEY").unwrap_or(""),
            )
            .build(),
        );
    if is_normal_start {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            let _ = screensaver::open_settings_window(app);
        }));
    }

    builder
        .setup(move |app| {
            let handle = app.handle().clone();

            let mut loaded = settings::load(&handle);
            let no_arg_mode = loaded.launch.no_arg_mode;
            let debug_windowed = args.debug_windowed;

            // 全新安装默认开启自启，只做一次：标记在 settings.json 里，落盘后不再干预。
            // 老配置（serde 缺省 initialized=true）、用户主动关掉、任务管理器禁用都不碰。
            //
            // 只有真的落定才写标记：写 Run 键失败时把 initialized 留成 false，下次启动
            // 重试。「尽量让时钟开机就在」不该被一次偶发失败永久放弃——记 true 落盘
            // 等于宣布「问过了」，而那次其实没成功。
            if is_normal_start && !loaded.system.autostart_initialized {
                let mut settled = autostart::has_run_value();
                if !settled {
                    match autostart::set_enabled(true) {
                        Ok(()) => settled = true,
                        Err(error) => {
                            eprintln!("FlipClock: 默认开启自启失败，下次启动再试：{error}")
                        }
                    }
                }
                if settled {
                    loaded.system.autostart_initialized = true;
                    if let Err(error) = settings::save(&handle, &loaded) {
                        eprintln!("FlipClock: 自启初始化标记没能落盘：{error}");
                    }
                }
            }

            handle.manage(AppState {
                settings: Mutex::new(loaded),
                session: Mutex::new(None),
            });

            // 卸载器钩子（src-tauri/windows/hooks.nsh）：把屏保选择还给用户，然后立刻退出。
            // 不走托盘、不开窗口、不碰自启——这一趟只做一件事。
            if is_headless_restore {
                if let Err(error) = install::restore(&handle) {
                    eprintln!("FlipClock: 卸载前恢复屏保失败：{error}");
                }
                handle.exit(0);
                return Ok(());
            }

            // 托盘失败不该拖垮整个应用：没有托盘时时钟与屏保照常可用。
            if is_normal_start {
                if let Err(error) = tray::install(&handle) {
                    eprintln!("FlipClock: 托盘创建失败：{error}");
                }
            }
            // 程序被移动过之后，注册表里的旧路径会让自启悄悄失效。
            //
            // 必须限定正常启动：屏保进程的 `current_exe()` 是 `FlipClock.scr`，
            // 让它跑这一步会把 Run 键改写成 `"...\FlipClock.scr" --tray`——
            // 组件一卸载，自启就指向一个不存在的文件，而界面上看不出任何异常。
            if is_normal_start {
                autostart::sync_command_line();
            }

            // 更新只替换主程序，`.scr` 副本会停在上一版；这里把它赶上，
            // 免得 Windows 下次起屏保还是旧画面。丢到后台线程：要比对整份文件，
            // 不该挡住窗口出现。失败不打扰用户——系统页会显示「版本不匹配」。
            if is_normal_start {
                std::thread::spawn(|| match install::refresh_if_outdated() {
                    Ok(true) => eprintln!("FlipClock: 屏保组件已跟随主程序刷新"),
                    Ok(false) => {}
                    Err(error) => eprintln!("FlipClock: 屏保组件没能刷新：{error}"),
                });
            }

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
                // 自启拉起：只驻留托盘，不弹任何窗口。
                ParsedLaunch::Tray => Ok(()),
                // 上面已经提前退出；这一支只是让 match 保持完整。
                ParsedLaunch::RestoreScreensaver => Ok(()),
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
            commands::is_installed_build,
            commands::install_screensaver,
            commands::restore_screensaver,
            commands::open_windows_screensaver_settings,
            commands::open_about_link,
            commands::leave_fullscreen_clock,
            commands::autostart_status,
            commands::set_autostart,
            commands::ui_ready,
            commands::set_locale,
            commands::open_theme_preview,
            commands::get_theme_preview,
            commands::close_theme_preview,
            commands::themes_dir_path,
            commands::import_theme_image,
            commands::remove_theme_image,
            commands::delete_theme_dir,
            commands::fonts_dir_path,
            commands::import_font,
            commands::remove_font,
            commands::export_config_bundle,
            commands::inspect_config_bundle,
            commands::import_config_bundle,
        ])
        .build(tauri::generate_context!())
        .expect("error while running FlipClock")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                // 窗口全关时不要退出：托盘还在。显式 exit(0)（托盘「退出」、屏保结束）
                // 带 code，照常退出。
                if code.is_none() && app.tray_by_id(tray::TRAY_ID).is_some() {
                    api.prevent_exit();
                }
            }
        });
}
