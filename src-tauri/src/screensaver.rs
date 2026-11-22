//! 屏保会话：窗口创建、显示器策略、退出判定、系统预览嵌入。

use std::path::PathBuf;
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
/// 全屏配置窗口（ADR-0021/0022）。标签沿用 `theme-preview`，这是前端契约；
/// 语义是全屏配置，不是屏保预览。
pub const MODE_FULLSCREEN_SETUP: &str = "theme-preview";

/// 全屏配置要编辑的整份快照（JSON，ADR-0022）。
/// 跨窗口传不了 Vue 的响应式对象，这里存一份原始 JSON 给新窗口自取。
static PREVIEW_DRAFT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
pub const MODE_SCREENSAVER: &str = "screensaver";
pub const MODE_CLOCK: &str = "clock";

/// 所有持有 WebView2 的主应用窗口。新增窗口类型时必须加进来——到点释放
/// WebView2（ADR-0032 修订）靠这份清单。
const MAIN_WEBVIEW_WINDOWS: [&str; 3] = [MODE_SETTINGS, MODE_CLOCK, MODE_FULLSCREEN_SETUP];

/// 启动瞬间的指针飘移容忍期。
const SETTLE_GRACE: Duration = Duration::from_millis(700);
/// 判定为「用户移动了鼠标」的物理像素位移。
const JITTER_THRESHOLD: i32 = 3;
const POLL_INTERVAL: Duration = Duration::from_millis(40);
const DISPLAY_POLL_INTERVAL: Duration = Duration::from_millis(1500);
/// 到点前多少秒释放主应用的 WebView2。
const SAVER_RELEASE_LEAD_SECONDS: u64 = 2;
/// 超时极短时也给释放留一秒，避免刚启动就抢在 Windows 前关窗。
const MIN_SAVER_CLOSE_AFTER_SECONDS: u64 = 1;

/// 设置窗口外壳的圆角（逻辑像素）。必须与 `src/styles/tokens.css` 的
/// `--radius-shell` 一致：窗口区域和 CSS 圆角是同一个形状，改一个就得改另一个。
const SHELL_RADIUS: f64 = 13.0;

/// **所有**窗口共用的 WebView2 参数。
///
/// 主要是音效：WebView2 默认要求页面先收到用户输入才允许 AudioContext 出声；
/// 屏保与全屏时钟全程没有输入，不放开这一条，翻页音效永远不会响。
///
/// 屏保进程现在使用独立数据目录（ADR-0032 修订），不再复用主应用的浏览器进程；
/// 同一 profile 内的窗口仍共用这组参数。
///
/// 前三个 `--disable-features` 是 Tauri 自己的默认值，覆盖这个参数时必须一起带上。
const SHARED_BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
     --autoplay-policy=no-user-gesture-required";


/// 主题图片与导入字体的根目录。
///
/// 前端要它才能把「配置里的文件名」拼成可加载的 `asset://` 地址；以前只能等
/// 窗口起来之后问一次 IPC，于是首帧根本没有图片（ADR-0030 修订）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRoots {
    pub themes_dir: Option<String>,
    pub fonts_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootPayload {
    pub mode: &'static str,
    pub exit_on_input: bool,
    pub debug_windowed: bool,
    /// 配置快照：让前端**第一帧**就能画用户选的主题（ADR-0030）。
    ///
    /// 只有沉浸式窗口（屏保、全屏时钟）需要；设置窗口与主题预览窗口为 None。
    /// 填值的地方只有 [`create_window`] 一处——沉浸式窗口都走它，调用方
    /// 不可能漏带这份快照。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<Settings>,
    /// 资产根目录，和 `settings` 一起只给沉浸式窗口，理由同上。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<AssetRoots>,
    /// 窗口在 webview 画出第一帧之前铺的底色（`#RRGGBB`）。
    ///
    /// `index.html` 的 `<head>` 用它落成 CSS 变量，免得先画出写死的近黑再被盖掉。
    /// 见 ADR-0031。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_background: Option<String>,
    /// 当前主题背景图的缩略图（data URL）。`index.html` 在解析阶段就把它铺成
    /// 页面底色，屏幕一亮就是照片的模糊版，见 ADR-0031 与 `theme_preview`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_background_image: Option<String>,
}

impl BootPayload {
    /// 沉浸式窗口的载荷。`settings` / `assets` 留空，由 `create_window` 统一补齐。
    fn immersive(mode: &'static str, exit_on_input: bool, debug_windowed: bool) -> Self {
        Self {
            mode,
            exit_on_input,
            debug_windowed,
            settings: None,
            assets: None,
            window_background: None,
            boot_background_image: None,
        }
    }

    /// 非沉浸式窗口：不带配置快照、资产与首帧底色，等前端自己就绪。
    fn plain(mode: &'static str) -> Self {
        Self {
            mode,
            exit_on_input: false,
            debug_windowed: false,
            settings: None,
            assets: None,
            window_background: None,
            boot_background_image: None,
        }
    }
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

    if let Some(existing) = app.get_webview_window(MODE_FULLSCREEN_SETUP) {
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

    // 预览窗口创建即可见；用当前主题的底色铺首帧，避免浅色主题先闪一下默认深色。
    // 只带底色，不注入配置快照：草稿仍由前端从 PREVIEW_DRAFT 接管（ADR-0030）。
    let background = app
        .state::<AppState>()
        .settings
        .lock()
        .ok()
        .map(|settings| crate::theme_preview::background_for(&settings))
        .unwrap_or_else(crate::theme_preview::classic_background);
    let window_background = background.hex;
    let window_color = background.color;

    let window = WebviewWindowBuilder::new(
        app,
        MODE_FULLSCREEN_SETUP,
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
    // 与屏保共用同一组浏览器参数（音效自动播放）；屏保使用独立数据目录，
    // 不再复用这里的 WebView2（ADR-0032 修订）。
    .additional_browser_args(SHARED_BROWSER_ARGS)
    .background_color(window_color)
    // 创建即可见：先 visible(false) 再 show() 是为了避免白闪，但 show 失败时
    // 窗口就永远不出现——用户只会看到「点了没反应」。宁可闪一下。
    .visible(true)
    .initialization_script(boot_script(&BootPayload {
        mode: MODE_FULLSCREEN_SETUP,
        exit_on_input: false,
        debug_windowed: false,
        // 预览窗口带的是自己的草稿（PREVIEW_DRAFT），不吃这份配置快照。
        settings: None,
        assets: None,
        window_background: Some(window_background),
        boot_background_image: None,
    }))
    .build()
    .map_err(|error| AppError::CreateWindowFailed {
        label: String::from(MODE_FULLSCREEN_SETUP),
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
    data_directory: Option<PathBuf>,
) -> Result<WebviewWindow, String> {
    // 首帧主题（ADR-0030）。窗口在 webview 画出第一帧之前就 show()，而前端要等
    // `get_settings` 回来才知道用户选了哪个主题；不预先把配置带过去，屏保一出现
    // 会先闪一下 `defaultSettings()` 的经典碳灰，再切到真主题。
    //
    // 这里注入的是 `AppState` 里那份内存配置，与 `get_settings` 返回的是同一份，
    // 所以前端拿它当初值不会与后端打架。锁中毒时不注入：宁可退回「先默认、后
    // 校正」的老路径，也不要塞一份假的默认配置冒充用户选择。
    let settings = app
        .state::<AppState>()
        .settings
        .lock()
        .ok()
        .map(|loaded| (*loaded).clone());
    // 图片与导入字体同理：根目录现在就知道，给前端省掉一次 IPC，首帧就能带图。
    // 取不到就留空，前端会退回「起来之后再问一次」的路径。
    let assets = Some(AssetRoots {
        themes_dir: crate::settings::themes_dir(app)
            .ok()
            .map(|dir| dir.to_string_lossy().to_string()),
        fonts_dir: crate::settings::fonts_dir(app)
            .ok()
            .map(|dir| dir.to_string_lossy().to_string()),
    });
    // 盖屏那一下铺主题底色，而不是黑（ADR-0031）。窗口在 webview 画出第一帧之前
    // 就 show()，这段时间里能看见的只有窗口底色；给对了，观感就是「屏幕暗下来、
    // 然后钟面浮出来」，给错了就是「黑一下再跳出钟面」。
    let background = match settings.as_ref() {
        Some(loaded) => crate::theme_preview::background_for(loaded),
        // 锁中毒时配置读不出来，按经典碳灰铺底，前端那边同样是回退到默认主题。
        None => crate::theme_preview::classic_background(),
    };
    let window_background = background.hex;
    let window_color = background.color;
    // 带背景图的主题：再带一张 160px 的缩略图。窗口出现后到钟面画出来之间约 0.5s
    // 铺的就是它，否则只有一块纯色（ADR-0031 修订）。文件不在就退回底色。
    let boot_background_image = settings
        .as_ref()
        .and_then(|loaded| crate::theme_preview::data_url(app, loaded));
    let payload = BootPayload {
        settings,
        assets,
        window_background: Some(window_background),
        boot_background_image,
        ..payload
    };
    let debug = payload.debug_windowed;

    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
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
        .background_color(window_color)
        .additional_browser_args(SHARED_BROWSER_ARGS);

    // 屏保会话使用独立 WebView2 数据目录（ADR-0032 修订）：主应用的设置/时钟
    // 窗口还活着时，Windows 拉起的屏保进程共用默认目录会创建失败（0x800700AA）。
    // 两边各用各的浏览器进程；代价是屏保会话不再复用主应用的浏览器进程。
    if let Some(dir) = data_directory {
        builder = builder.data_directory(dir);
    }

    let window = builder
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
/// 会话结束即结束进程；为假时是屏保预览，结束时只关闭会话窗口。
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

    // 只有 Windows 以 `/s` 拉起的独立屏保会话需要独立目录；屏保预览与全屏时钟
    // 留在主应用的 profile 里，避免多起一个 WebView2 环境。
    let saver_data_dir = if standalone {
        Some(crate::paths::saver_data_dir()?)
    } else {
        None
    };

    let mut labels = Vec::with_capacity(bounds.len());
    for (index, rect) in bounds.iter().enumerate() {
        let label = format!("screensaver-{index}");
        create_window(
            app,
            &label,
            *rect,
            BootPayload::immersive(MODE_SCREENSAVER, !debug_windowed, debug_windowed),
            saver_data_dir.clone(),
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

/// 在 Windows 屏保到点前释放主应用的 WebView2。
///
/// 屏保会话使用独立 WebView2 数据目录（ADR-0032 修订），启动不再依赖这一步；
/// 保留它是为了在屏保期间释放主应用的 WebView2（设置窗口实测 552MB）。按
/// 系统真实空闲计时；到点前 2 秒销毁设置、全屏时钟、全屏配置窗口并结束屏保
/// 预览，应用靠托盘继续运行。
pub fn release_main_windows_before_saver(app: AppHandle) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));
            let Some(timeout) = crate::install::active_saver_timeout_seconds() else {
                continue;
            };
            let close_after = timeout
                .saturating_sub(SAVER_RELEASE_LEAD_SECONDS)
                .max(MIN_SAVER_CLOSE_AFTER_SECONDS);
            let Some(idle) = platform::idle_seconds() else {
                continue;
            };
            if idle < close_after {
                continue;
            }

            let preview_active = app
                .state::<AppState>()
                .session
                .lock()
                .map(|session| session.is_some())
                .unwrap_or(false);
            if preview_active {
                end_session(&app);
            }
            for label in MAIN_WEBVIEW_WINDOWS {
                if let Some(window) = app.get_webview_window(label) {
                    let _ = window.destroy();
                }
            }
        }
    });
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

/// 结束会话时的第一步：把所有窗口一起藏掉。独立屏保进程随后直接退出，
/// 不 close；预览会话由 `end_session` 再调 `close_windows`。
///
/// 窗口在 webview 拆掉之后还会亮约 100ms，那段时间它只剩自己的底色——主题底色
/// 常常就是近黑，看起来就是「钟面先变黑屏、然后才回到桌面」（实测 109ms）。
/// 先藏再关，桌面在 hide 那一刻就回来，拆 webview 与退进程都发生在屏幕之外。
///
/// 为什么不逐个「藏+关」：多显示器时会看到它们一块一块消失。
fn hide_windows(app: &AppHandle, labels: &[String]) {
    for label in labels {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.hide();
        }
    }
}

/// 关单个窗口的同一套规矩：先藏再关。退出全屏时钟走这里。
pub fn close_window(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.hide();
        let _ = window.close();
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

    hide_windows(app, &session.labels);
    if session.standalone {
        // 独立屏保进程**不 close**：`close()` 会先拆掉 webview，窗口却还挂在屏幕上
        // 一瞬，那一下只剩窗口底色（主题底色常常就是近黑）——用户看到的就是
        // 「钟面变黑一小下再回桌面」（实测 48ms 变底色、76ms 进程退出）。
        // 直接结束进程，窗口由系统一并销毁，屏幕上留下的还是最后一帧钟面。
        //
        // 也不走 `app.exit(0)`：它要等 WebView2 优雅关闭，实测 1.4 秒才真正退出，
        // 这段时间 DWM 一直把最后一帧留在屏幕上。屏保进程没有要落盘的状态。
        // （ADR-0003「退出时的收尾顺序」）
        std::process::exit(0);
    }
    close_windows(app, &session.labels);
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
        // 与屏保进程共用同一组浏览器参数（音效自动播放）；屏保使用独立数据目录，
        // 不再复用这里的浏览器进程（ADR-0032 修订）。
        .additional_browser_args(SHARED_BROWSER_ARGS)
        // 窗口隐藏到 ui_ready 再显示，WebView 已经画好，不需要预铺窗口底色。
        .visible(false)
        .initialization_script(boot_script(&BootPayload::plain(MODE_SETTINGS)))
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

    // 关窗即销毁窗口，应用留在托盘；真正退出走托盘菜单。设置里关掉这一项就恢复成点 × 直接退出。
    //
    // 这里必须让窗口真正关闭、连同 WebView2 一起销毁，不能 prevent_close + hide：
    // 隐藏的 WebView2 仍会挡住 Windows 拉起的屏保。应用靠托盘常驻，关窗后从托盘
    // 重开设置会创建一个新的 WebView2。
    let close_handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { .. } = event {
            let close_to_tray = close_handle
                .state::<AppState>()
                .settings
                .lock()
                .map(|settings| settings.system.close_to_tray)
                .unwrap_or(false);
            if !close_to_tray || close_handle.tray_by_id(crate::tray::TRAY_ID).is_none() {
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
        BootPayload::immersive(MODE_CLOCK, false, debug_windowed),
        None,
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
