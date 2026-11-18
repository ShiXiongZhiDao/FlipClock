//! 屏保会话：窗口创建、显示器策略、退出判定、系统预览嵌入。

use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::platform;
use crate::settings::{MonitorMode, Settings};

pub const MODE_SETTINGS: &str = "settings";
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
const SHELL_RADIUS: f64 = 18.0;

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
            .map_err(|e| format!("读取主显示器失败：{e}"))?
            .into_iter()
            .collect(),
        MonitorMode::All => app
            .available_monitors()
            .map_err(|e| format!("枚举显示器失败：{e}"))?,
    };

    if monitors.is_empty() {
        return Err("没有可用的显示器".into());
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
        .map_err(|e| format!("创建窗口 {label} 失败：{e}"))?;

    window
        .set_position(PhysicalPosition::new(bounds.x, bounds.y))
        .map_err(|e| format!("定位窗口 {label} 失败：{e}"))?;
    window
        .set_size(PhysicalSize::new(bounds.width, bounds.height))
        .map_err(|e| format!("调整窗口 {label} 尺寸失败：{e}"))?;

    if payload.exit_on_input {
        let _ = window.set_cursor_visible(false);
    }

    window
        .show()
        .map_err(|e| format!("显示窗口 {label} 失败：{e}"))?;
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
        .min_inner_size(width, height)
        .max_inner_size(width, height);

    let builder = match WINDOW_AT.get() {
        Some((x, y)) => builder.position(*x, *y),
        None => builder.center(),
    };

    let window = builder
        // 自绘标题栏：去掉原生边框，交通灯和拖拽区都在前端。
        // 代价是没有边缘缩放和 Win11 贴靠布局，所以窗口直接固定尺寸。
        .decorations(false)
        .resizable(false)
        .maximizable(false)
        .theme(Some(tauri::Theme::Light))
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
        .map_err(|e| format!("打开设置窗口失败：{e}"))?;

    // 圆角由窗口区域裁出来，不依赖透明窗口：Windows 10 的 DWM 透明在
    // 「透明效果」关闭时会合成成黑边，靠不住。见 platform.rs。
    let scale = window
        .scale_factor()
        .map_err(|e| format!("读取窗口缩放失败：{e}"))?;
    let size = window
        .inner_size()
        .map_err(|e| format!("读取窗口尺寸失败：{e}"))?;
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
        .ok_or("没有可用的显示器")?;

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
        return Err("Windows 传入的预览父窗口无效".into());
    }
    Err("系统预览暂未支持：内置 WebView 无法稳定嵌入系统预览框".into())
}
