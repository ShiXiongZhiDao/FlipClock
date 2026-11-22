//! 主题背景图的启动缩略图（ADR-0031 修订）。
//!
//! 屏保窗口出现到钟面画出来之间约 0.5s，铺的是主题底色。带背景图的主题在这段里
//! 只有一块纯色——用户看到的就是「黑一下」。这里把背景图缩到 160px 宽存成
//! `preview.jpg`，建窗口时随启动载荷下发成 data URL，`index.html` 在解析阶段
//! 就铺上：屏幕一亮就是这张图的模糊版，钟面再淡入盖上去。
//!
//! 它是**派生文件**：固定名字、和主题图片同目录。缺了退回底色；换图会重新生成；
//! 删背景图会一起删掉。生成失败只留痕，不让导入或启动失败。

use std::path::Path;

use base64::Engine;
use tauri::{window::Color, AppHandle};

use crate::settings::{self, Settings};

pub const PREVIEW_FILE: &str = "preview.jpg";
/// 缩略图宽度。160px 铺到 1080p 能看清构图与颜色，又小到能塞进启动载荷。
const PREVIEW_WIDTH: u32 = 160;

pub(crate) const CLASSIC_DARK_HEX: &str = "#111315";
const CLASSIC_DARK: Color = Color(0x11, 0x13, 0x15, 255);
const FALLBACK_BACKGROUND: Color = Color(0x0d, 0x0f, 0x10, 255);

/// 内置主题的首帧底色，与 `src/lib/theme/presets.ts` 的 background.color 对应。
/// 新增内置主题时同步 `docs/development.md` 的「新增内置主题的同步点」。
const BUILTIN_BACKGROUNDS: &[(&str, &str)] = &[
    ("classic-dark", CLASSIC_DARK_HEX),
    ("oled-black", "#000000"),
    ("retro-light", "#ddd4c5"),
    ("mirror-gold", "#100b06"),
    ("black-label", "#070706"),
];

pub(crate) struct ThemeBackground {
    pub hex: String,
    pub color: Color,
}

pub(crate) fn classic_background() -> ThemeBackground {
    ThemeBackground {
        hex: String::from(CLASSIC_DARK_HEX),
        color: CLASSIC_DARK,
    }
}

/// 当前主题的底色：自定义主题优先，其次是内置表，悬空 id 退回经典碳灰。
pub(crate) fn background_for(settings: &Settings) -> ThemeBackground {
    let current = settings.appearance.theme.as_str();
    let hex = settings
        .custom_theme(current)
        .map(|spec| spec.background.color.clone())
        .or_else(|| {
            BUILTIN_BACKGROUNDS
                .iter()
                .find(|(id, _)| *id == current)
                .map(|(_, color)| (*color).to_string())
        })
        .unwrap_or_else(|| String::from(CLASSIC_DARK_HEX));
    let color = parse_hex_color(&hex).unwrap_or(FALLBACK_BACKGROUND);
    ThemeBackground { hex, color }
}

/// `#RRGGBB` → Color。只认六位：配置里存的就是这个格式，多一种写法多一处漂移。
fn parse_hex_color(hex: &str) -> Option<Color> {
    let raw = hex.trim().trim_start_matches('#');
    if raw.len() != 6 || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&raw[range], 16).ok();
    Some(Color(
        channel(0..2)?,
        channel(2..4)?,
        channel(4..6)?,
        255,
    ))
}

/// 生成缩略图。`image_path` 是主题目录里的背景图，`theme_dir` 是它的目录。
pub fn write(image_path: &Path, theme_dir: &Path) -> Result<(), String> {
    let image = image::open(image_path).map_err(|error| error.to_string())?;
    // 高度给 u32::MAX：让宽度成为约束，缩略图保持原图比例。
    let thumb = image.thumbnail(PREVIEW_WIDTH.min(image.width()), u32::MAX);
    std::fs::create_dir_all(theme_dir).map_err(|error| error.to_string())?;
    thumb
        .save_with_format(theme_dir.join(PREVIEW_FILE), image::ImageFormat::Jpeg)
        .map_err(|error| error.to_string())
}

/// 当前主题背景图的缩略图，做成 data URL；没有就 None（前端退回主题底色）。
pub fn data_url(app: &AppHandle, settings: &Settings) -> Option<String> {
    let current = settings.appearance.theme.as_str();
    // 只有自定义主题带图；内置主题没有背景图，也不需要缩略图。
    let spec = settings.custom_theme(current)?;
    spec.background.image.as_ref()?;
    let path = settings::themes_dir(app).ok()?.join(current).join(PREVIEW_FILE);
    let bytes = std::fs::read(path).ok()?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:image/jpeg;base64,{encoded}"))
}

/// 补齐缺失的缩略图：这次改动之前导入的背景图没有 `preview.jpg`。
///
/// 启动时在后台跑一次，逐个主题看有没有图、有没有缩略图。失败只留痕——大不了
/// 那一次屏保还是铺底色，不能因此影响启动。
pub fn ensure_missing(themes_dir: &Path, settings: &Settings) {
    for spec in &settings.appearance.custom_themes {
        let Some(image) = spec.background.image.as_deref() else {
            continue;
        };
        let dir = themes_dir.join(&spec.id);
        if dir.join(PREVIEW_FILE).exists() {
            continue;
        }
        let source = dir.join(image);
        if !source.exists() {
            continue;
        }
        if let Err(error) = write(&source, &dir) {
            eprintln!("FlipClock: 主题缩略图生成失败 {}: {error}", spec.id);
        }
    }
}

/// 背景图被删掉时顺手删缩略图，避免下次启动铺一张已经不存在的图。
pub fn forget(theme_dir: &Path) {
    let _ = std::fs::remove_file(theme_dir.join(PREVIEW_FILE));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ThemeSpec;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("fc-preview-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn writes_a_bounded_thumbnail() {
        let dir = temp_dir("write");
        let source = dir.join("background-1.png");
        // 一张 640×360 的纯色图，缩略图应该按宽度收到 160。
        let canvas = image::RgbImage::from_pixel(640, 360, image::Rgb([30, 60, 90]));
        canvas.save(&source).expect("source");
        write(&source, &dir).expect("preview");
        let thumb = image::open(dir.join(PREVIEW_FILE)).expect("read preview");
        assert_eq!(thumb.width(), 160);
        assert_eq!(thumb.height(), 90);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_small_images_without_upscaling() {
        let dir = temp_dir("small");
        let source = dir.join("background-2.png");
        image::RgbImage::from_pixel(40, 20, image::Rgb([1, 2, 3]))
            .save(&source)
            .expect("source");
        write(&source, &dir).expect("preview");
        assert_eq!(image::open(dir.join(PREVIEW_FILE)).expect("read").width(), 40);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 只关心底色，其余部件给一份合法的最小值。
    fn custom_theme(id: &str, background: &str) -> ThemeSpec {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "background": {
                "color": background,
                "opacity": 100.0,
                "image": null,
                "fit": "cover",
                "dim": 0.0
            },
            "card": { "material": "matte", "color": "#303437", "opacity": 100.0 },
            "digit": {
                "material": "matte",
                "color": "#f1f0e9",
                "opacity": 100.0,
                "image": null
            },
            "date": { "color": "#9ba19f", "opacity": 100.0 },
            "seam": { "color": "#000000", "opacity": 72.0 },
            "separator": { "color": "#f1f0e9", "opacity": 62.0 }
        }))
        .expect("theme spec")
    }

    /// 只能证明「每个内置 id 都能从表里查到」，查不到就会落到经典碳灰的兜底。
    /// 表里的 hex 是否与 presets.ts 一致，靠的是新增主题时的同步清单，测试管不了。
    #[test]
    fn builtin_theme_background_lookup() {
        for (id, expected) in BUILTIN_BACKGROUNDS {
            let mut settings = Settings::default();
            settings.appearance.theme = (*id).to_string();
            assert_eq!(
                background_for(&settings).hex,
                *expected,
                "内置主题 {id} 没查到底色"
            );
        }
    }

    #[test]
    fn custom_theme_background_wins() {
        let mut settings = Settings::default();
        settings.appearance.theme = String::from("custom-1");
        settings
            .appearance
            .custom_themes
            .push(custom_theme("custom-1", "#123456"));
        assert_eq!(background_for(&settings).hex, "#123456");
    }

    #[test]
    fn dangling_theme_falls_back_to_classic_dark() {
        let mut settings = Settings::default();
        settings.appearance.theme = String::from("neon-pink");
        assert_eq!(background_for(&settings).hex, CLASSIC_DARK_HEX);
    }

    #[test]
    fn hex_parsing_rejects_garbage() {
        assert_eq!(parse_hex_color("#111315"), Some(Color(0x11, 0x13, 0x15, 255)));
        assert_eq!(parse_hex_color("111315"), Some(Color(0x11, 0x13, 0x15, 255)));
        assert_eq!(parse_hex_color("#fff"), None);
        assert_eq!(parse_hex_color("rgba(0,0,0,1)"), None);
        assert_eq!(parse_hex_color("#11131g"), None);
    }
}
