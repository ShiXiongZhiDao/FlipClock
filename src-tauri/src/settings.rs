//! 配置模型与持久化。
//!
//! 存储位置：`<app_config_dir>/settings.json`（由 identifier 决定，与 productName 无关）。
//! 写入采用临时文件 + 原子替换，避免崩溃留下半截 JSON。

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

pub const SCHEMA_VERSION: u32 = 1;

/// 界面玻璃透明度的唯一范围；前端 `types/settings.ts` 用同一组数字。
pub const GLASS_TRANSPARENCY_MIN: f32 = 0.05;
pub const GLASS_TRANSPARENCY_MAX: f32 = 0.75;

/// 时钟缩放比例的唯一边界；只作用于全屏时钟与屏保。
/// 上限远大于「刚好铺满屏幕」的约 1.57 倍，超出部分会被屏幕裁切，这是用户的选择。
pub const CLOCK_SCALE_MIN: f32 = 0.5;
pub const CLOCK_SCALE_MAX: f32 = 4.5;

/// 翻页音效音量的唯一边界；0 即静音。
pub const SOUND_VOLUME_MIN: f32 = 0.0;
pub const SOUND_VOLUME_MAX: f32 = 1.0;

/// 时钟亮度：只往下压，不往上提。数字本来就在 0.95 附近，再提就削顶成死白。
pub const BRIGHTNESS_MIN: f32 = 0.2;
pub const BRIGHTNESS_MAX: f32 = 1.0;

pub fn clamp_glass(value: f32) -> f32 {
    value.clamp(GLASS_TRANSPARENCY_MIN, GLASS_TRANSPARENCY_MAX)
}

pub fn clamp_brightness(value: f32) -> f32 {
    value.clamp(BRIGHTNESS_MIN, BRIGHTNESS_MAX)
}

pub fn clamp_clock_scale(value: f32) -> f32 {
    value.clamp(CLOCK_SCALE_MIN, CLOCK_SCALE_MAX)
}

pub fn clamp_sound_volume(value: f32) -> f32 {
    value.clamp(SOUND_VOLUME_MIN, SOUND_VOLUME_MAX)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub time: TimeSettings,
    #[serde(default)]
    pub sound: SoundSettings,
    #[serde(default)]
    pub screensaver: ScreenSaverSettings,
    #[serde(default)]
    pub launch: LaunchSettings,
    #[serde(default)]
    pub registration: Registration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            appearance: Appearance::default(),
            time: TimeSettings::default(),
            sound: SoundSettings::default(),
            screensaver: ScreenSaverSettings::default(),
            launch: LaunchSettings::default(),
            registration: Registration::default(),
        }
    }
}

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub glass: Glass,
    /// 时钟缩放比例。数据放在外观段（它改的是"长什么样"），
    /// 控件放在「时间与日期」页（它和秒数开关一样会改整块时钟的排版）。
    #[serde(default = "default_clock_scale")]
    pub clock_scale: f32,
    /// 每张翻页卡承载几位数字。与 clock_scale 同样存在外观段、控件在时间页。
    #[serde(default)]
    pub digit_layout: DigitLayout,
    /// 时钟整体亮度（只压暗）。1.0 = 原样。
    #[serde(default = "default_brightness")]
    pub brightness: f32,
    /// 数字字体。只影响钟面数字，不影响日期行与界面。
    #[serde(default)]
    pub font: DigitFont,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: Theme::ClassicDark,
            glass: Glass::default(),
            clock_scale: default_clock_scale(),
            digit_layout: DigitLayout::default(),
            brightness: default_brightness(),
            font: DigitFont::default(),
        }
    }
}

fn default_brightness() -> f32 {
    1.0
}

/// 数字字体。三款全部是系统自带或已随包，不引入新资产。
///
/// 字宽差异很大（Bebas 极窄、Segoe UI 接近方形），所以前端按
/// 「字体 × 数字排布」两套字号比例分别标定，见 `src/styles/fonts.css`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DigitFont {
    Bebas,
    Bahnschrift,
    System,
}

impl Default for DigitFont {
    fn default() -> Self {
        Self::Bebas
    }
}

/// 每张翻页卡承载几位数字。
///
/// 两种排布共用同一个四层翻页结构，差别只在分组粒度与卡片几何：
/// `per-digit` 时、分各两张竖卡；`per-pair` 各一张方形卡装两位数字。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DigitLayout {
    PerDigit,
    PerPair,
}

impl Default for DigitLayout {
    fn default() -> Self {
        // 默认两位一卡：两张方卡、数字更大，是产品的主推观感。
        Self::PerPair
    }
}

fn default_clock_scale() -> f32 {
    // 225% 是 16:9 屏上「两位一卡铺满约 93% 宽度」的位置，作为默认观感。
    2.25
}

/// 设置窗口里半透明面板的“有多透”。只影响界面，不影响时钟。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Glass {
    /// 0 = 完全不透明，1 = 完全透明。默认 0.48，对应 CSS 里 0.52 的白色不透明度。
    #[serde(default = "default_glass_transparency")]
    pub transparency: f32,
}

impl Default for Glass {
    fn default() -> Self {
        Self {
            transparency: default_glass_transparency(),
        }
    }
}

fn default_glass_transparency() -> f32 {
    0.48
}

/// 翻页音效。默认开启、音量 0.6；只影响全屏时钟与屏保，设置窗口本身不出声。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundSettings {
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "default_sound_volume")]
    pub volume: f32,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: default_sound_volume(),
        }
    }
}

fn default_sound_volume() -> f32 {
    0.6
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    ClassicDark,
    OledBlack,
    RetroLight,
}

impl Default for Theme {
    fn default() -> Self {
        Self::ClassicDark
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSettings {
    #[serde(default)]
    pub format: TimeFormat,
    #[serde(default)]
    pub show_date: bool,
    #[serde(default)]
    pub show_seconds: bool,
    /// 时、分之间的两个小圆点。默认关：安静的钟面更接近极简屏保。
    #[serde(default)]
    pub show_colon: bool,
}

impl Default for TimeSettings {
    fn default() -> Self {
        Self {
            format: TimeFormat::H24,
            // 默认只留钟面：日期行、秒数、冒号都关。
            show_date: false,
            show_seconds: false,
            show_colon: false,
        }
    }
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimeFormat {
    #[serde(rename = "24h")]
    H24,
    #[serde(rename = "12h")]
    H12,
}

impl Default for TimeFormat {
    fn default() -> Self {
        Self::H24
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenSaverSettings {
    #[serde(default)]
    pub monitor_mode: MonitorMode,
}

impl Default for ScreenSaverSettings {
    fn default() -> Self {
        Self {
            monitor_mode: MonitorMode::Primary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MonitorMode {
    Primary,
    All,
}

impl Default for MonitorMode {
    fn default() -> Self {
        Self::Primary
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchSettings {
    #[serde(default)]
    pub no_arg_mode: NoArgMode,
}

impl Default for LaunchSettings {
    fn default() -> Self {
        Self {
            no_arg_mode: NoArgMode::Settings,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoArgMode {
    Settings,
    Fullscreen,
}

impl Default for NoArgMode {
    fn default() -> Self {
        Self::Settings
    }
}

/// 安装屏保组件时留下的回滚点。不存隐私信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registration {
    #[serde(default)]
    pub rollback_scrnsave_exe: Option<String>,
    #[serde(default)]
    pub rollback_screen_save_active: Option<String>,
    #[serde(default)]
    pub installed_path: Option<String>,
}

pub fn config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("settings.json"))
}

/// 读取配置。任何一段损坏都只回退那一段，不拖垮整份配置。
pub fn load(app: &AppHandle) -> Settings {
    let Some(path) = config_path(app) else {
        return Settings::default();
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return Settings::default();
    };

    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => {
            backup_corrupt(&path);
            return Settings::default();
        }
    };

    let mut settings = Settings::default();
    if let Some(v) = value.get("schemaVersion").and_then(|v| v.as_u64()) {
        settings.schema_version = v as u32;
    }
    settings.appearance = section(&value, "appearance").unwrap_or_default();
    settings.appearance.glass.transparency = clamp_glass(settings.appearance.glass.transparency);
    settings.appearance.clock_scale = clamp_clock_scale(settings.appearance.clock_scale);
    settings.appearance.brightness = clamp_brightness(settings.appearance.brightness);
    settings.time = section(&value, "time").unwrap_or_default();
    settings.sound = section(&value, "sound").unwrap_or_default();
    settings.sound.volume = clamp_sound_volume(settings.sound.volume);
    settings.screensaver = section(&value, "screensaver").unwrap_or_default();
    settings.launch = section(&value, "launch").unwrap_or_default();
    settings.registration = section(&value, "registration").unwrap_or_default();
    settings.schema_version = SCHEMA_VERSION;
    settings
}

fn section<T>(value: &serde_json::Value, key: &str) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(value.get(key)?.clone()).ok()
}

fn backup_corrupt(path: &PathBuf) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = path.with_file_name(format!("settings.corrupt.{stamp}.json"));
    let _ = fs::rename(path, backup);
}

/// 原子写入：先写临时文件，再替换目标。
pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = config_path(app).ok_or("无法定位配置目录")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败：{e}"))?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| format!("序列化配置失败：{e}"))?;

    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("写入临时配置失败：{e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("替换配置失败：{e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_agreed_baseline() {
        let s = Settings::default();
        assert_eq!(s.appearance.theme, Theme::ClassicDark);
        assert!((s.appearance.glass.transparency - 0.48).abs() < f32::EPSILON);
        assert!((s.appearance.clock_scale - 2.25).abs() < f32::EPSILON);
        assert_eq!(s.appearance.digit_layout, DigitLayout::PerPair);
        assert!((s.appearance.brightness - 1.0).abs() < f32::EPSILON);
        assert_eq!(s.appearance.font, DigitFont::Bebas);
        assert!(s.sound.enabled);
        assert!((s.sound.volume - 0.6).abs() < f32::EPSILON);
        assert_eq!(s.time.format, TimeFormat::H24);
        assert!(!s.time.show_date);
        assert!(!s.time.show_seconds);
        assert!(!s.time.show_colon);
        assert_eq!(s.screensaver.monitor_mode, MonitorMode::Primary);
        assert_eq!(s.launch.no_arg_mode, NoArgMode::Settings);
    }

    #[test]
    fn glass_transparency_is_clamped_to_the_shared_range() {
        assert!((clamp_glass(0.9) - GLASS_TRANSPARENCY_MAX).abs() < f32::EPSILON);
        assert!((clamp_glass(0.0) - GLASS_TRANSPARENCY_MIN).abs() < f32::EPSILON);
        assert!((clamp_glass(0.4) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn old_appearance_without_glass_gets_the_default() {
        let raw = serde_json::json!({ "appearance": { "theme": "oled-black" } });
        let appearance: Appearance = section(&raw, "appearance").unwrap();
        assert_eq!(appearance.theme, Theme::OledBlack);
        assert!((appearance.glass.transparency - 0.48).abs() < f32::EPSILON);
        assert!((appearance.clock_scale - 2.25).abs() < f32::EPSILON);
        assert_eq!(appearance.digit_layout, DigitLayout::PerPair);
        assert!((appearance.brightness - 1.0).abs() < f32::EPSILON);
        assert_eq!(appearance.font, DigitFont::Bebas);
    }

    #[test]
    fn brightness_and_font_are_read_from_the_appearance_section() {
        let raw = serde_json::json!({
            "appearance": { "brightness": 0.45, "font": "bahnschrift" }
        });
        let appearance: Appearance = section(&raw, "appearance").unwrap();
        assert!((appearance.brightness - 0.45).abs() < f32::EPSILON);
        assert_eq!(appearance.font, DigitFont::Bahnschrift);

        assert!((clamp_brightness(0.05) - BRIGHTNESS_MIN).abs() < f32::EPSILON);
        assert!((clamp_brightness(1.8) - BRIGHTNESS_MAX).abs() < f32::EPSILON);
        assert!((clamp_brightness(0.7) - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn digit_layout_reads_kebab_case_ids() {
        let raw = serde_json::json!({ "appearance": { "digitLayout": "per-pair" } });
        let appearance: Appearance = section(&raw, "appearance").unwrap();
        assert_eq!(appearance.digit_layout, DigitLayout::PerPair);

        let raw = serde_json::json!({ "appearance": { "digitLayout": "per-digit" } });
        let appearance: Appearance = section(&raw, "appearance").unwrap();
        assert_eq!(appearance.digit_layout, DigitLayout::PerDigit);
    }

    #[test]
    fn time_section_without_show_date_falls_back_to_hidden() {
        // 逐字段 serde 默认必须与 TimeSettings::default() 一致，否则缺字段的配置
        // （旧版本、手改文件）会把日期行重新打开，与「默认只留钟面」冲突。
        let raw = serde_json::json!({ "time": { "showSeconds": true } });
        let time: TimeSettings = section(&raw, "time").unwrap();
        assert!(!time.show_date);
        assert!(time.show_seconds);
    }

    #[test]
    fn old_settings_without_sound_fall_back_to_defaults() {
        let raw = serde_json::json!({ "schemaVersion": 1, "appearance": {}, "time": {} });
        assert!(section::<SoundSettings>(&raw, "sound").is_none());
        let sound = SoundSettings::default();
        assert!(sound.enabled);
        assert!((sound.volume - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn clock_scale_and_volume_are_clamped_to_the_shared_range() {
        assert!((clamp_clock_scale(99.0) - CLOCK_SCALE_MAX).abs() < f32::EPSILON);
        assert!((clamp_clock_scale(0.1) - CLOCK_SCALE_MIN).abs() < f32::EPSILON);
        assert!((clamp_clock_scale(1.2) - 1.2).abs() < f32::EPSILON);

        assert!((clamp_sound_volume(2.0) - SOUND_VOLUME_MAX).abs() < f32::EPSILON);
        assert!((clamp_sound_volume(-1.0) - SOUND_VOLUME_MIN).abs() < f32::EPSILON);
    }

    #[test]
    fn unknown_values_in_one_section_do_not_break_the_others() {
        let raw = serde_json::json!({
            "schemaVersion": 1,
            "appearance": { "theme": "neon-pink" },
            "time": { "format": "12h", "showDate": false, "showSeconds": true }
        });
        let mut settings = Settings::default();
        settings.appearance = section(&raw, "appearance").unwrap_or_default();
        settings.time = section(&raw, "time").unwrap_or_default();

        assert_eq!(settings.appearance.theme, Theme::ClassicDark);
        assert_eq!(settings.time.format, TimeFormat::H12);
        assert!(settings.time.show_seconds);
    }

    #[test]
    fn missing_sections_fall_back_to_defaults() {
        let raw = serde_json::json!({ "schemaVersion": 1 });
        assert!(section::<TimeSettings>(&raw, "time").is_none());
    }
}
