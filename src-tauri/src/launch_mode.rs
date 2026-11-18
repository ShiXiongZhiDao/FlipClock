//! 命令行启动模式解析。
//!
//! Windows 屏保以 `/s`、`/c`、`/p <HWND>` 启动，参数名大小写不敏感，
//! 前缀可能是 `/`、`-` 或 `--`。这里只做解析，不做决策——
//! 无参数时的形态由用户设置决定，见 [`ParsedLaunch::Unspecified`]。

use std::ffi::OsString;

/// 解析后的启动意图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedLaunch {
    /// `/s`：以屏保身份由 Windows 拉起。
    ScreenSaver,
    /// `/c`、`/a`：配置入口。
    Configure,
    /// `/p <HWND>`：嵌入系统屏保设置里的预览框。
    SystemPreview { parent: isize },
    /// `--fullscreen`：应用内全屏时钟。
    Fullscreen,
    /// 没有可识别的参数，形态交给用户设置。
    Unspecified,
}

#[derive(Debug, Clone)]
pub struct LaunchArgs {
    pub intent: ParsedLaunch,
    /// 调试用：把屏保窗口降级为普通窗口，便于在开发机上验证而不遮住屏幕。
    pub debug_windowed: bool,
    /// 调试用：`--window-at=x,y` 指定设置窗口位置，避免与本机其它窗口重叠。
    pub window_at: Option<(f64, f64)>,
    /// 调试用：`--window-size=w,h` 强制设置窗口尺寸。
    pub window_size: Option<(f64, f64)>,
}

impl LaunchArgs {
    pub fn parse<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut intent = ParsedLaunch::Unspecified;
        let mut debug_windowed = false;
        let mut window_at = None;
        let mut window_size = None;

        let mut it = args.into_iter().map(Into::into).skip(1).peekable();
        while let Some(raw) = it.next() {
            let text = raw.to_string_lossy().to_string();
            let (flag, inline_value) = split_flag(&text);

            match flag.as_str() {
                "s" | "screensaver" => intent = ParsedLaunch::ScreenSaver,
                "c" | "configure" | "a" => intent = ParsedLaunch::Configure,
                "fullscreen" => intent = ParsedLaunch::Fullscreen,
                "debug-windowed" => debug_windowed = true,
                "window-at" => {
                    if let Some((x, y)) = inline_value.as_deref().and_then(parse_point) {
                        window_at = Some((x, y));
                    }
                }
                "window-size" => {
                    if let Some((w, h)) = inline_value.as_deref().and_then(parse_point) {
                        window_size = Some((w, h));
                    }
                }
                "p" | "preview" => {
                    let value = inline_value.or_else(|| {
                        it.next().map(|v: OsString| v.to_string_lossy().to_string())
                    });
                    // HWND 是不可信输入：解析失败就当作没有预览请求，不去猜。
                    if let Some(parent) = value.as_deref().and_then(parse_hwnd) {
                        intent = ParsedLaunch::SystemPreview { parent };
                    }
                }
                _ => {}
            }
        }

        LaunchArgs {
            intent,
            debug_windowed,
            window_at,
            window_size,
        }
    }
}

fn split_flag(text: &str) -> (String, Option<String>) {
    let trimmed = text.trim_start_matches('-');
    let trimmed = trimmed.strip_prefix('/').unwrap_or(trimmed);
    match trimmed.split_once(':').or_else(|| trimmed.split_once('=')) {
        Some((flag, value)) => (
            flag.to_ascii_lowercase(),
            Some(value.trim_matches('"').to_string()),
        ),
        None => (trimmed.to_ascii_lowercase(), None),
    }
}

fn parse_point(raw: &str) -> Option<(f64, f64)> {
    let (x, y) = raw.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

fn parse_hwnd(raw: &str) -> Option<isize> {
    let raw = raw.trim().trim_matches('"');
    let parsed = if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        isize::from_str_radix(hex, 16).ok()
    } else {
        raw.parse::<isize>().ok()
    }?;
    (parsed != 0).then_some(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> LaunchArgs {
        LaunchArgs::parse(args.iter().copied())
    }

    #[test]
    fn recognises_standard_parameters() {
        assert_eq!(parse(&["app.exe", "/s"]).intent, ParsedLaunch::ScreenSaver);
        assert_eq!(parse(&["app.exe", "/S"]).intent, ParsedLaunch::ScreenSaver);
        assert_eq!(parse(&["app.exe", "-s"]).intent, ParsedLaunch::ScreenSaver);
        assert_eq!(parse(&["app.exe", "/c"]).intent, ParsedLaunch::Configure);
        assert_eq!(parse(&["app.exe", "/a"]).intent, ParsedLaunch::Configure);
    }

    #[test]
    fn parses_preview_handle_in_both_forms() {
        assert_eq!(
            parse(&["app.exe", "/p", "12345"]).intent,
            ParsedLaunch::SystemPreview { parent: 12345 }
        );
        assert_eq!(
            parse(&["app.exe", "/p:12345"]).intent,
            ParsedLaunch::SystemPreview { parent: 12345 }
        );
        assert_eq!(
            parse(&["app.exe", "/p", "0x1A2B"]).intent,
            ParsedLaunch::SystemPreview { parent: 0x1A2B }
        );
    }

    #[test]
    fn rejects_untrusted_preview_values() {
        assert_eq!(parse(&["app.exe", "/p", "abc"]).intent, ParsedLaunch::Unspecified);
        assert_eq!(parse(&["app.exe", "/p", "0"]).intent, ParsedLaunch::Unspecified);
        assert_eq!(parse(&["app.exe", "/p"]).intent, ParsedLaunch::Unspecified);
    }

    #[test]
    fn unknown_parameters_fall_back_to_unspecified() {
        assert_eq!(parse(&["app.exe"]).intent, ParsedLaunch::Unspecified);
        assert_eq!(parse(&["app.exe", "/q"]).intent, ParsedLaunch::Unspecified);
    }

    #[test]
    fn debug_flag_can_combine_with_mode() {
        let args = parse(&["app.exe", "/s", "--debug-windowed"]);
        assert_eq!(args.intent, ParsedLaunch::ScreenSaver);
        assert!(args.debug_windowed);
    }
}
