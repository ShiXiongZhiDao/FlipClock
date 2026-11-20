//! 屏保会话：窗口创建、显示器策略、退出判定、系统预览嵌入。

use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::errors::AppError;
use crate::platform;
use crate::settings::{MonitorMode, Settings};

pub const MODE_SETTINGS: &str = "settings";
/// 主题编辑的独立预览窗口。**创建时就建成全屏**，不做任何运行时窗口状态切换——
/// 这是唯一在本机能稳定生效的窗口尺寸路径。
pub const MODE_THEME_PREVIEW: &str = "theme-preview";

/// 全屏配置要编辑的整份快照（JSON，ADR-0022）。
/// 跨窗口传不了 Vue 的响应式对象，这里存一份原始 JSON 给新窗口自取。
static PREVIEW_DRAFT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
pub const MODE_SCREENSAVER: &str = "screensaver";
pub const MODE_CLOCK: &str = "clock";

/// 启动瞬间的指针飘移容忍期。
const SETTLE_GRACE: Duration = Duration::from_millis(700);
/// 判定为「用户移动了鼠标」的物理像素位移。
const JITTER_THRESHOLD: i32 = 3;
const POLL_INTERVAL: Duration = Duration::from_millis(40);
const DISPLAY_POLL_INTERVAL: Duration = Duration::from_millis(1500);

/// 设置窗口外壳的圆角（逻辑像素）。必须与 `src/styles/tokens.css` 的
/// `--radius-shell` 一致：窗口区域和 CSS 圆角是同一个形状，改一个就得改另一个。
const SHELL_RADIUS: f64 = 13.0;

/// 沉浸窗口的 WebView2 参数。
///
/// WebView2 默认要求页面先收到用户输入，才允许 AudioContext 真正出声；而屏保与
/// 全屏时钟全程没有任何输入，不放开这一条，翻页音效就永远不会响。
/// 前三个 `--disable-features` 是 Tauri 自己的默认值，覆盖这个参数时必须一起带上。
const IMMERSIVE_BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
     --autoplay-policy=no-user-gesture-required";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootPayload {
    pub mode: &'static str,
    pub exit_on_input: bool,
    pub debug_windowed: bool,
}

static WINDOW_AT: std::sync::OnceLock<(f64, f64)> = std::sync::OnceLock::new();
static WINDOW_SIZE: std::sync::OnceLock<(f64, f64)> = std::sync::OnceLock::new();

pub fn set_window_size(size: (f64, f64)) {
    let _ = WINDOW_SIZE.set(size);
}

pub fn set_window_at(pos: (f64, f64)) {
    let _ = WINDOW_AT.set(pos);
}

#[derive(Default)]
pub struct AppState {
    pub settings: Mutex<Settings>,
    pub session: Mutex<Option<SaverSession>>,
}

pub struct SaverSession {
    pub labels: Vec<String>,
    pub bounds: Vec<Bounds>,
    pub standalone: bool,
    pub debug_windowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Bounds {
    fn debug_window() -> Self {
        Self {
            x: 120,
            y: 80,
            width: 960,
            height: 540,
        }
    }
}

/// 设置窗口的理想尺寸，按主显示器可用区域收缩。
fn settings_window_size(app: &AppHandle) -> (f64, f64) {
    const IDEAL: (f64, f64) = (1180.0, 740.0);

    let Some(monitor) = app.primary_monitor().ok().flatten() else {
        return IDEAL;
    };
    let scale = monitor.scale_factor();
    let logical_w = monitor.size().width as f64 / scale;
    // 任务栏 + 标题栏留白，按 90% / 86% 估算可用高度。
    let logical_h = monitor.size().height as f64 / scale - 48.0;

    (
        IDEAL.0.min((logical_w * 0.94).round()),
        IDEAL.1.min((logical_h * 0.94).round()),
    )
}

/// 打开全屏配置窗口：整屏、无边框、置顶、不占任务栏。
///
/// 尺寸在**创建时**给定，之后不再调整。前面几版都试图在运行时把设置窗口
/// 撑成全屏（set_fullscreen / SetWindowPos / maximize），在你的机器上全部
/// 「报告成功但画面不变」；而窗口创建参数从来没失败过。
pub fn open_theme_preview(app: &AppHandle, payload: String) -> Result<(), AppError> {
    // GUI 子系统没有控制台，eprintln 会被静默丢弃。预览窗口打不开是用户
    // 能感知、开发者却看不到的问题，调试版留一份落盘痕迹（正式版不写）。
    preview_diag(format!("called payload_len={}", payload.len()));

    if let Some(existing) = app.get_webview_window(MODE_THEME_PREVIEW) {
        let _ = existing.close();
    }
    if let Ok(mut slot) = PREVIEW_DRAFT.lock() {
        *slot = Some(payload);
    }

    let monitor = app
        .primary_monitor()
        .map_err(|error| AppError::ReadPrimaryMonitorFailed {
            detail: error.to_string(),
        })?
        .ok_or(AppError::NoMonitor)?;
    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    let origin = monitor.position().to_logical::<f64>(scale);

    let window = WebviewWindowBuilder::new(
        app,
        MODE_THEME_PREVIEW,
        WebviewUrl::App("index.html".into()),
    )
    .title("FlipClock")
    .decorations(false)
    .inner_size(size.width, size.height)
    .position(origin.x, origin.y)
    // 创建时就置顶：这是盖住任务栏的唯一可靠时机。
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .maximizable(false)
    .shadow(false)
    // 创建即可见：先 visible(false) 再 show() 是为了避免白闪，但 show 失败时
    // 窗口就永远不出现——用户只会看到「点了没反应」。宁可闪一下。
    .visible(true)
    .initialization_script(boot_script(&BootPayload {
        mode: MODE_THEME_PREVIEW,
        exit_on_input: false,
        debug_windowed: false,
    }))
    .build()
    .map_err(|error| AppError::CreateWindowFailed {
        label: String::from(MODE_THEME_PREVIEW),
        detail: error.to_string(),
    })?;

    let _ = window.set_focus();
    preview_diag(format!(
        "built size={:?} visible={:?}",
        window.inner_size().ok(),
        window.is_visible().ok()
    ));
    Ok(())
}

/// 预览窗口的诊断落盘。只在调试构建里生效，正式版是空函数。
#[cfg(debug_assertions)]
fn preview_diag(message: String) {
    let _ = std::fs::write(
        std::env::temp_dir().join("fc-open-preview.log"),
        message,
    );
}

#[cfg(not(debug_assertions))]
fn preview_diag(_message: String) {}

/// 取走预览草稿。只给新窗口读一次——读完即清，避免第二次打开读到旧主题。
pub fn take_theme_preview() -> Option<String> {
    PREVIEW_DRAFT.lock().ok().and_then(|mut slot| slot.take())
}

fn boot_script(payload: &BootPayload) -> String {
    let json = serde_json::to_string(payload).unwrap_or_else(|_| "{}".into());
    format!("window.__FLIPCLOCK_BOOT__ = {json};")
}

/// 按显示器策略算出每个屏保窗口应该占据的矩形（物理像素）。
pub fn compute_bounds(app: &AppHandle, mode: MonitorMode, debug: bool) -> Result<Vec<Bounds>, String> {
    if debug {
        return Ok(vec![Bounds::debug_window()]);
    }

    let monitors = match mode {
        MonitorMode::Primary => app
            .primary_monitor()
            .map_err(|e| AppError::ReadPrimaryMonitorFailed {
                detail: e.to_string(),
            })?
            .into_iter()
            .collect(),
        MonitorMode::All => app
            .available_monitors()
            .map_err(|e| AppError::EnumerateMonitorsFailed {
                detail: e.to_string(),
            })?,
    };

    if monitors.is_empty() {
        return Err(String::from(AppError::NoMonitor));
    }

    Ok(monitors
        .iter()
        .map(|m| Bounds {
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
        })
        .collect())
}

fn create_window(
    app: &AppHandle,
    label: &str,
    bounds: Bounds,
    payload: BootPayload,
) -> Result<WebviewWindow, String> {
    let debug = payload.debug_windowed;

    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("FlipClock")
        .decorations(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .closable(true)
        .visible(false)
        .skip_taskbar(!debug)
        .always_on_top(!debug)
        .focused(true)
        .shadow(false)
        .additional_browser_args(IMMERSIVE_BROWSER_ARGS)
        .initialization_script(boot_script(&payload))
        .build()
        .map_err(|e| AppError::CreateWindowFailed {
            label: label.to_string(),
            detail: e.to_string(),
        })?;

    window
        .set_position(PhysicalPosition::new(bounds.x, bounds.y))
        .map_err(|e| AppError::LocateWindowFailed {
            label: label.to_string(),
            detail: e.to_string(),
        })?;
    window
        .set_size(PhysicalSize::new(bounds.width, bounds.height))
        .map_err(|e| AppError::ResizeWindowFailed {
            label: label.to_string(),
            detail: e.to_string(),
        })?;

    if payload.exit_on_input {
        let _ = window.set_cursor_visible(false);
    }

    window
        .show()
        .map_err(|e| AppError::ShowWindowFailed {
            label: label.to_string(),
            detail: e.to_string(),
        })?;
    let _ = window.set_focus();
    Ok(window)
}

/// 启动一次屏保会话。`standalone` 为真时表示进程本身就是屏保（`/s`），
/// 会话结束即结束进程；为假时是应用内预览，结束时只关闭会话窗口。
pub fn start_session(
    app: &AppHandle,
    standalone: bool,
    debug_windowed: bool,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state.session.lock().map(|s| s.is_some()).unwrap_or(false) {
        return Ok(());
    }

    let mode = state
        .settings
        .lock()
        .map(|s| s.screensaver.monitor_mode)
        .unwrap_or(MonitorMode::Primary);
    let bounds = compute_bounds(app, mode, debug_windowed)?;

    let mut labels = Vec::with_capacity(bounds.len());
    for (index, rect) in bounds.iter().enumerate() {
        let label = format!("screensaver-{index}");
        create_window(
            app,
            &label,
            *rect,
            BootPayload {
                mode: MODE_SCREENSAVER,
                exit_on_input: !debug_windowed,
                debug_windowed,
            },
        )?;
        labels.push(label);
    }

    if let Ok(mut guard) = state.session.lock() {
        *guard = Some(SaverSession {
            labels,
            bounds,
            standalone,
            debug_windowed,
        });
    }

    if !debug_windowed {
        spawn_input_watcher(app.clone());
        spawn_display_watcher(app.clone());
    }
    Ok(())
}

fn spawn_input_watcher(app: AppHandle) {
    thread::spawn(move || {
        let started = Instant::now();
        let mut baseline_tick = platform::last_input_tick();
        let mut baseline_cursor = platform::cursor_pos();

        loop {
            thread::sleep(POLL_INTERVAL);

            let alive = app
                .state::<AppState>()
                .session
                .lock()
                .map(|s| s.is_some())
                .unwrap_or(false);
            if !alive {
                return;
            }

            let tick = platform::last_input_tick();
            let cursor = platform::cursor_pos();

            let new_input = matches!((baseline_tick, tick), (Some(before), Some(now)) if now != before);
            let moved = match (baseline_cursor, cursor) {
                (Some(before), Some(now)) => {
                    (now.0 - before.0).abs() > JITTER_THRESHOLD
                        || (now.1 - before.1).abs() > JITTER_THRESHOLD
                }
                _ => false,
            };

            // 启动瞬间 Windows 常会带一点指针飘移，或者用户刚松开「预览」按钮的鼠标。
            // 这段时间只刷新基准，不判定退出。
            if started.elapsed() < SETTLE_GRACE {
                baseline_tick = tick;
                baseline_cursor = cursor;
                continue;
            }

            let down = platform::mouse_button_down();
            if new_input || moved || down {
                end_session(&app);
                return;
            }
        }
    });
}

/// 显示器增减或分辨率变化时重建窗口，避免窗口留在不可见坐标上。
fn spawn_display_watcher(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(DISPLAY_POLL_INTERVAL);

        let snapshot = {
            let state = app.state::<AppState>();
            let guard = match state.session.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            match guard.as_ref() {
                None => return,
                Some(session) => Some((
                    session.standalone,
                    session.debug_windowed,
                    session.bounds.clone(),
                )),
            }
        };

        let Some((standalone, debug_windowed, previous)) = snapshot else {
            return;
        };

        let mode = app
            .state::<AppState>()
            .settings
            .lock()
            .map(|s| s.screensaver.monitor_mode)
            .unwrap_or(MonitorMode::Primary);

        let Ok(current) = compute_bounds(&app, mode, debug_windowed) else {
            continue;
        };
        if current == previous {
            continue;
        }

        // 先清空会话，避免关窗时把重建误判成用户退出。
        // 如果这里取不到会话，说明用户刚刚自己退出了，别再把它拉起来。
        let labels = {
            let state = app.state::<AppState>();
            let mut guard = match state.session.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            match guard.take() {
                Some(session) => session.labels,
                None => return,
            }
        };
        close_windows(&app, &labels);
        if start_session(&app, standalone, debug_windowed).is_err() {
            end_session(&app);
            return;
        }
    });
}

fn close_windows(app: &AppHandle, labels: &[String]) {
    for label in labels {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.close();
        }
    }
}

/// 结束屏保会话。幂等：重复调用不会重复关窗或重复退出进程。
pub fn end_session(app: &AppHandle) {
    let session = app
        .state::<AppState>()
        .session
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());

    let Some(session) = session else {
        return;
    };

    close_windows(app, &session.labels);
    if session.standalone {
        app.exit(0);
    }
}

pub fn open_settings_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(MODE_SETTINGS) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return Ok(());
    }

    // 设计稿是 1180×740，但小屏（1366×768、或 1080p 开 125% 缩放）放不下，
    // 所以按可用工作区收缩，保证窗口一定完整可见。
    let (width, height) = WINDOW_SIZE
        .get()
        .copied()
        .unwrap_or_else(|| settings_window_size(app));

    let builder = WebviewWindowBuilder::new(app, MODE_SETTINGS, WebviewUrl::App("index.html".into()))
        .title("FlipClock")
        .inner_size(width, height)
        // ADR-0004 决定 1：固定尺寸 = resizable(false) + min == max。
        .min_inner_size(width, height)
        .max_inner_size(width, height);

    let builder = match WINDOW_AT.get() {
        Some((x, y)) => builder.position(*x, *y),
        None => builder.center(),
    };

    let window = builder
        // 自绘标题栏：去掉原生边框，交通灯和拖拽区都在前端。
        // 代价是没有 Win11 贴靠布局，所以也不做最大化按钮。
        .decorations(false)
        // 尺寸固定：两栏按比例分配（左栏 minmax(336px, .82fr)），一拉伸就会让
        // 左栏预览与右侧主题墙各自变形，界面的疏密也就不再是设计时的那一套。
        // 要在整屏下看效果，走「全屏配置」那个独立窗口，不靠把设置页拉大。
        .resizable(false)
        .maximizable(false)
        // 不设窗口主题：保持默认的 Auto，WebView2 的 prefers-color-scheme 就一直是
        // 系统的真实值。显式浅/深由前端的 data-appearance + color-scheme 负责——
        // 运行期 set_theme 改不动 webview 的配色方案（见 lib/appearance.ts 的说明）。
        // 无边框窗口的原生影子不是 DWM 直接画的，而是 tao 把窗口向外扩 8px、
        // 在 client 之外留出一圈给 DWM 投影。WebView 只覆盖 client，那 8px
        // 就会露出一圈半透明灰边；圆角一裁，四角外侧还多一圈方形投影（虚影）。
        // 关掉影子后 client == 整个窗口。
        .shadow(false)
        .visible(false)
        .initialization_script(boot_script(&BootPayload {
            mode: MODE_SETTINGS,
            exit_on_input: false,
            debug_windowed: false,
        }))
        .build()
        .map_err(|e| AppError::OpenSettingsWindowFailed {
            detail: e.to_string(),
        })?;

    // 圆角由窗口区域裁出来，不依赖透明窗口：Windows 10 的 DWM 透明在
    // 「透明效果」关闭时会合成成黑边，靠不住。见 platform.rs。
    let scale = window
        .scale_factor()
        .map_err(|e| AppError::ReadWindowScaleFailed {
            detail: e.to_string(),
        })?;
    let size = window
        .inner_size()
        .map_err(|e| AppError::ReadWindowSizeFailed {
            detail: e.to_string(),
        })?;
    platform::set_round_window_region(&window, size.width, size.height, scale, SHELL_RADIUS)?;

    // 跨到不同 DPI 的显示器时物理尺寸会变，区域要按新尺寸重建。
    let region_handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::ScaleFactorChanged {
            scale_factor,
            new_inner_size,
            ..
        } = event
        {
            if let Some(window) = region_handle.get_webview_window(MODE_SETTINGS) {
                let _ = platform::set_round_window_region(
                    &window,
                    new_inner_size.width,
                    new_inner_size.height,
                    *scale_factor,
                    SHELL_RADIUS,
                );
            }
        }
    });

    // 关窗即退到托盘；真正退出走托盘菜单。设置里关掉这一项就恢复成点 × 直接退出。
    let close_handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            let close_to_tray = close_handle
                .state::<AppState>()
                .settings
                .lock()
                .map(|settings| settings.system.close_to_tray)
                .unwrap_or(false);
            if close_to_tray && close_handle.tray_by_id(crate::tray::TRAY_ID).is_some() {
                api.prevent_close();
                if let Some(window) = close_handle.get_webview_window(MODE_SETTINGS) {
                    let _ = window.hide();
                }
            } else {
                close_handle.exit(0);
            }
        }
    });

    // 窗口先不显示：前端读完配置后再调 ui_ready 显示，避免先闪一下未渲染的底色。
    // 这里留一个兜底，避免前端出问题时窗口永远不出现。
    let handle = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(2500));
        if let Some(window) = handle.get_webview_window(MODE_SETTINGS) {
            let _ = window.show();
        }
    });
    Ok(())
}

/// 应用内全屏时钟（无参数启动策略选了「全屏时钟」时使用）。
pub fn open_fullscreen_clock(app: &AppHandle, debug_windowed: bool) -> Result<(), String> {
    let bounds = compute_bounds(app, MonitorMode::Primary, debug_windowed)?
        .into_iter()
        .next()
        .ok_or(String::from(AppError::NoMonitor))?;

    let window = create_window(
        app,
        "clock",
        bounds,
        BootPayload {
            mode: MODE_CLOCK,
            exit_on_input: false,
            debug_windowed,
        },
    )?;
    let _ = window.set_always_on_top(!debug_windowed);
    Ok(())
}

/// 系统预览（`/p <HWND>`）。
///
/// 已实测失败：窗口能按父窗口客户区尺寸创建、也确认是无边框的，
/// 但在纯 Win32 宿主（记事本、画图）和 WebView2 宿主上都绘制不出内容。
/// 结论是内置 WebView 无法稳定嵌入系统预览框。这里选择不创建任何窗口直接退出——
/// 系统设置里的预览框保持空白，同时不会留下一个看不见的进程。
pub fn start_system_preview(_app: &AppHandle, parent: isize) -> Result<(), String> {
    if !platform::is_valid_window(parent) {
        return Err(String::from(AppError::InvalidPreviewParent));
    }
    Err(String::from(AppError::SystemPreviewUnsupported))
}
