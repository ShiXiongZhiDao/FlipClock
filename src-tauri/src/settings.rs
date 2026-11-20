//! 配置模型与持久化。
//!
//! 存储位置：`<app_config_dir>/settings.json`（由 identifier 决定，与 productName 无关）。
//! 写入采用临时文件 + 原子替换，避免崩溃留下半截 JSON。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::errors::AppError;

pub const SCHEMA_VERSION: u32 = 4;

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

pub fn clamp_brightness(value: f32) -> f32 {
    value.clamp(BRIGHTNESS_MIN, BRIGHTNESS_MAX)
}

pub fn clamp_clock_scale(value: f32) -> f32 {
    value.clamp(CLOCK_SCALE_MIN, CLOCK_SCALE_MAX)
}

pub fn clamp_sound_volume(value: f32) -> f32 {
    value.clamp(SOUND_VOLUME_MIN, SOUND_VOLUME_MAX)
}

/// 界面语言偏好。system 只在启动时由前端归并为 zh/en，Rust 侧不参与判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguagePreference {
    System,
    Zh,
    En,
}

impl Default for LanguagePreference {
    fn default() -> Self {
        Self::System
    }
}

/// 界面外观偏好：设置窗口外壳的明暗。与钟面主题是两条独立的轴（ADR-0025）。
///
/// system 不需要 Rust 去监听系统事件：窗口主题设成 None 时 WebView2 的
/// preferred color scheme 就是 Auto，`prefers-color-scheme` 自己会跟着系统走，
/// 前端只负责把它归并成 light/dark 两个值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppearancePreference {
    System,
    Light,
    Dark,
}

impl Default for AppearancePreference {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSettings {
    #[serde(default)]
    pub language: LanguagePreference,
    #[serde(default)]
    pub appearance: AppearancePreference,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            language: LanguagePreference::default(),
            appearance: AppearancePreference::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub screensaver: ScreenSaverSettings,
    #[serde(default)]
    pub launch: LaunchSettings,
    #[serde(default)]
    pub system: SystemSettings,
    #[serde(default)]
    pub ui: UiSettings,
    #[serde(default)]
    pub registration: Registration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            appearance: Appearance::default(),
            screensaver: ScreenSaverSettings::default(),
            launch: LaunchSettings::default(),
            system: SystemSettings::default(),
            ui: UiSettings::default(),
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
    /// 当前主题 id：五个内置 id 之一，或 custom_themes 里某一条的 id。
    #[serde(default = "default_theme_id")]
    pub theme: String,
    /// 用户自定义主题（ADR-0020）。内置主题是预设，不存这里。
    #[serde(default)]
    pub custom_themes: Vec<ThemeSpec>,
    /// 全局导入字体库（ADR-0026）。字体文件只存一份，按字体 id 被各主题引用。
    #[serde(default)]
    pub fonts: Vec<CustomFont>,
    /// 每个主题自己的钟面配置（ADR-0022）：时间格式、排布、字体、缩放、
    /// 亮度、翻页音。内置与自定义主题共用这一张表，按主题 id 索引。
    #[serde(default = "default_profiles")]
    pub profiles: HashMap<String, ClockProfile>,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: default_theme_id(),
            custom_themes: Vec::new(),
            fonts: Vec::new(),
            profiles: default_profiles(),
        }
    }
}

/// 内置主题出厂时各带一份默认钟面配置；自定义主题在新增时补一份。
fn default_profiles() -> HashMap<String, ClockProfile> {
    BUILTIN_THEME_IDS
        .iter()
        .map(|id| ((*id).to_string(), ClockProfile::default()))
        .collect()
}

fn default_brightness() -> f32 {
    1.0
}

/// 全局字体库里的一款导入字体（ADR-0026）。
///
/// 文件在 `<app_config_dir>/fonts/<id>.<ext>`；`file` 只存文件名，不存路径。
/// `hash` 用于导入时去重，不参与渲染。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFont {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub hash: String,
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

/// 一个主题的钟面配置（ADR-0022）。
///
/// 决定「怎么显示、要不要响」，不决定「长什么样」——后者是 `ThemeSpec`。
/// 两者按主题 id 组合成用户看到的完整钟面；切换主题即整套切换。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockProfile {
    #[serde(default)]
    pub format: TimeFormat,
    #[serde(default)]
    pub show_date: bool,
    #[serde(default)]
    pub show_seconds: bool,
    /// 时、分之间的两个小圆点。关闭时整块不渲染。
    #[serde(default)]
    pub show_colon: bool,
    #[serde(default)]
    pub digit_layout: DigitLayout,
    /// 钟面字体 id：内置 id，或字体库里的 `custom-<uuid>`（ADR-0026）。
    #[serde(default = "default_font_id")]
    pub font: String,
    #[serde(default = "default_clock_scale")]
    pub clock_scale: f32,
    #[serde(default = "default_brightness")]
    pub brightness: f32,
    #[serde(default)]
    pub sound: SoundSettings,
}

impl Default for ClockProfile {
    fn default() -> Self {
        Self {
            format: TimeFormat::H24,
            // 默认只留钟面：日期行、秒数、冒号都关。
            show_date: false,
            show_seconds: false,
            show_colon: false,
            digit_layout: DigitLayout::PerPair,
            font: default_font_id(),
            clock_scale: default_clock_scale(),
            brightness: default_brightness(),
            sound: SoundSettings::default(),
        }
    }
}

impl ClockProfile {
    /// 边界只来自 `clamp_*`，前后端共用同一组。
    pub fn clamp(&mut self) {
        self.clock_scale = clamp_clock_scale(self.clock_scale);
        self.brightness = clamp_brightness(self.brightness);
        self.sound.volume = clamp_sound_volume(self.sound.volume);
    }
}

/* ------------------------------- 主题（ADR-0020） ------------------------------ */

/// 自定义主题上限。超过要删旧的——这是为了界面可管理，不是技术限制。
pub const CUSTOM_THEME_LIMIT: usize = 36;
pub const THEME_NAME_MAX_CHARS: usize = 16;
/// 导入字体上限（ADR-0026）。用户明确从 16 提到 32；界面靠懒加载控制内存。
pub const CUSTOM_FONT_LIMIT: usize = 32;
pub const FONT_NAME_MAX_CHARS: usize = 32;
/// 内置钟面字体 id。前端 `src/lib/fontRegistry.ts` 里是同一组字符串；
/// 这是跨语言契约，改名要两边一起改。
pub const BUILTIN_FONT_IDS: [&str; 3] = ["bebas", "bahnschrift", "system"];
/// 不透明度下限。0 表示「完全不要这个部件」：卡片为 0 只剩数字，
/// 分割线为 0 等于去掉冒号。全透明时翻页两层叠加都不可见，不存在加深问题。
pub const CARD_OPACITY_MIN: f32 = 0.0;
pub const PART_OPACITY_MIN: f32 = 0.0;
pub const BACKGROUND_DIM_MAX: f32 = 70.0;

/// 内置主题 id。前端 `presets.ts` 里是同一组字符串，改名要两边一起改。
pub const BUILTIN_THEME_IDS: [&str; 5] = [
    "classic-dark",
    "oled-black",
    "retro-light",
    "mirror-gold",
    "black-label",
];

fn default_theme_id() -> String {
    String::from("classic-dark")
}

fn default_font_id() -> String {
    String::from("bebas")
}

fn default_opacity() -> f32 {
    100.0
}

fn default_seam() -> FlatPart {
    FlatPart {
        color: String::from("#000000"),
        opacity: 72.0,
    }
}

/// 部件表面的光影结构。决定明暗怎么分布，颜色只决定色相。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMaterial {
    #[default]
    Matte,
    Metallic,
    Soft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum BackgroundFit {
    #[default]
    Cover,
    Tile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialPart {
    #[serde(default)]
    pub material: ThemeMaterial,
    pub color: String,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// 金属材质的边线色。黑标是「近黑卡 + 金边」，边线无法从主色推导。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlatPart {
    pub color: String,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundSpec {
    pub color: String,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// 图片文件名，不是路径。文件在 `<app_config_dir>/themes/<主题id>/` 下。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default)]
    pub fit: BackgroundFit,
    #[serde(default)]
    pub dim: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DigitSpec {
    #[serde(default)]
    pub material: ThemeMaterial,
    pub color: String,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSpec {
    pub id: String,
    /// 自定义主题是用户输入；内置主题留空。
    #[serde(default)]
    pub name: String,
    pub background: BackgroundSpec,
    pub card: MaterialPart,
    pub digit: DigitSpec,
    pub date: FlatPart,
    /// 翻页卡中间那条横向的缝。默认近黑 72%，与旧版从卡片色推导的观感接近。
    #[serde(default = "default_seam")]
    pub seam: FlatPart,
    pub separator: FlatPart,
}

fn is_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

/// 图片文件名白名单：只允许小写字母、数字、连字符加允许的扩展名。
/// 这条同时也是防路径穿越的唯一闸——`..`、`/`、反斜杠都进不来。
pub fn is_image_name(value: &str) -> bool {
    let Some((stem, ext)) = value.rsplit_once('.') else {
        return false;
    };
    if !matches!(ext, "png" | "jpg" | "jpeg" | "webp") {
        return false;
    }
    !stem.is_empty()
        && stem.len() <= 64
        && stem
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn clamp_opacity(value: f32, min: f32) -> f32 {
    value.clamp(min, 100.0)
}

/// 主题 / 字体 id 会被拼进文件路径，必须只允许小写字母、数字与连字符：
/// 这条同时挡住 `..`、斜杠与盘符。导入图片、导入字体与读配置都走同一份判据。
pub fn is_safe_theme_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// 导入字体 id 必须是 `custom-` 前缀；内置 id 不能被字体库占用。
pub fn is_safe_font_id(id: &str) -> bool {
    id.starts_with("custom-") && is_safe_theme_id(id)
}

/// 字体文件名白名单：`<id>.<ext>`，扩展名只允许 webview 能加载的四种。
pub fn is_font_name(value: &str) -> bool {
    let Some((stem, ext)) = value.rsplit_once('.') else {
        return false;
    };
    matches!(ext, "ttf" | "otf" | "woff" | "woff2")
        && !stem.is_empty()
        && stem.len() <= 64
        && stem
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// 字体文件必须属于它自己的 id，防止手改配置把 A 字体指向 B 文件。
pub fn is_font_file_for_id(id: &str, file: &str) -> bool {
    is_safe_font_id(id)
        && is_font_name(file)
        && file
            .strip_prefix(id)
            .map(|rest| rest.starts_with('.'))
            .unwrap_or(false)
}

fn font_is_valid(font: &CustomFont) -> bool {
    let name = font.name.trim();
    is_font_file_for_id(&font.id, &font.file)
        && !name.is_empty()
        && name.chars().count() <= FONT_NAME_MAX_CHARS
        && font.hash.chars().all(|c| c.is_ascii_hexdigit())
        && font.hash.len() <= 64
}

/// 主题是否完整可用。不合法就整条丢弃——静默修补出一个"看起来能用但已经变形"
/// 的主题，比直接不显示更糟。
fn theme_is_valid(theme: &ThemeSpec) -> bool {
    if !is_safe_theme_id(&theme.id) {
        return false;
    }
    let name = theme.name.trim();
    if name.is_empty() || name.chars().count() > THEME_NAME_MAX_CHARS {
        return false;
    }
    if !is_hex_color(&theme.background.color)
        || !is_hex_color(&theme.card.color)
        || !is_hex_color(&theme.digit.color)
        || !is_hex_color(&theme.date.color)
        || !is_hex_color(&theme.seam.color)
        || !is_hex_color(&theme.separator.color)
    {
        return false;
    }
    for line in [&theme.card.line_color, &theme.digit.line_color]
        .into_iter()
        .flatten()
    {
        if !is_hex_color(line) {
            return false;
        }
    }
    for image in [&theme.background.image, &theme.digit.image]
        .into_iter()
        .flatten()
    {
        if !is_image_name(image) {
            return false;
        }
    }
    true
}

/// 给重名主题加序号，并保证名字仍不超过上限——超长的名字下一轮会被
/// `theme_is_valid` 当成非法主题整条丢掉，等于用「改名」换「丢数据」。
fn numbered_name_with_limit(base: &str, taken: &[String], limit: usize) -> String {
    for index in 2..1000 {
        let suffix = format!(" {index}");
        let keep = limit.saturating_sub(suffix.chars().count());
        let stem: String = base.chars().take(keep).collect();
        let candidate = format!("{stem}{suffix}");
        if !taken.iter().any(|name| name == &candidate) {
            return candidate;
        }
    }
    base.to_string()
}

fn numbered_name(base: &str, taken: &[String]) -> String {
    numbered_name_with_limit(base, taken, THEME_NAME_MAX_CHARS)
}

/// 字体重名同样加序号；上限与主题不同（32 字符）。
fn numbered_font_name(base: &str, taken: &[String]) -> String {
    numbered_name_with_limit(base, taken, FONT_NAME_MAX_CHARS)
}

/// 把外观段收敛到合法状态：截断数量、丢弃非法主题、clamp 不透明度、修正悬空 id。
pub fn normalize_appearance(appearance: &mut Appearance) {
    // 字体库先收敛：名字 trim / 截断后，再校验 id、文件名与去重。
    // 名字为空时退回 id，避免界面上出现一条看不见的字体。
    for font in &mut appearance.fonts {
        font.name = font.name.trim().to_string();
        if font.name.is_empty() {
            font.name = font.id.clone();
        }
        font.name = font.name.chars().take(FONT_NAME_MAX_CHARS).collect();
        font.hash = font.hash.trim().to_ascii_lowercase();
    }
    let mut seen_font_ids: Vec<String> = Vec::new();
    let mut seen_font_files: Vec<String> = Vec::new();
    appearance.fonts.retain(|font| {
        if !font_is_valid(font)
            || seen_font_ids.contains(&font.id)
            || seen_font_files.contains(&font.file)
        {
            eprintln!("FlipClock: 丢弃非法字体 {:?}", font.id);
            return false;
        }
        seen_font_ids.push(font.id.clone());
        seen_font_files.push(font.file.clone());
        true
    });
    appearance.fonts.truncate(CUSTOM_FONT_LIMIT);

    let mut font_names: Vec<String> = Vec::new();
    for font in &mut appearance.fonts {
        if font_names.iter().any(|name| name == &font.name) {
            font.name = numbered_font_name(&font.name, &font_names);
        }
        font_names.push(font.name.clone());
    }

    let mut seen: Vec<String> = Vec::new();
    appearance.custom_themes.retain(|theme| {
        // 内置 id 不能出现在自定义列表里，否则同一个 id 会有两份定义。
        if BUILTIN_THEME_IDS.contains(&theme.id.as_str()) || seen.contains(&theme.id) {
            return false;
        }
        if !theme_is_valid(theme) {
            eprintln!("FlipClock: 丢弃非法主题 {:?}", theme.id);
            return false;
        }
        seen.push(theme.id.clone());
        true
    });
    appearance.custom_themes.truncate(CUSTOM_THEME_LIMIT);

    let mut names: Vec<String> = Vec::new();
    for theme in &mut appearance.custom_themes {
        theme.name = theme.name.trim().to_string();
        // 重名时加序号而不是丢弃：手改配置的人可能确实想要两个相近的主题，
        // 但界面上两行一模一样就没法分辨该删哪个。
        if names.iter().any(|name| name == &theme.name) {
            theme.name = numbered_name(&theme.name, &names);
        }
        names.push(theme.name.clone());
        theme.card.opacity = clamp_opacity(theme.card.opacity, CARD_OPACITY_MIN);
        theme.digit.opacity = clamp_opacity(theme.digit.opacity, PART_OPACITY_MIN);
        theme.date.opacity = clamp_opacity(theme.date.opacity, PART_OPACITY_MIN);
        theme.seam.opacity = clamp_opacity(theme.seam.opacity, PART_OPACITY_MIN);
        theme.separator.opacity = clamp_opacity(theme.separator.opacity, PART_OPACITY_MIN);
        theme.background.opacity = clamp_opacity(theme.background.opacity, PART_OPACITY_MIN);
        theme.background.dim = theme.background.dim.clamp(0.0, BACKGROUND_DIM_MAX);
    }

    // 配置表与主题表必须一一对应：多出来的删掉，缺的补默认（ADR-0022）。
    // 缺配置不能让钟面退回「没有秒数也没有声音」的空白状态。
    let mut known_ids: Vec<String> = BUILTIN_THEME_IDS
        .iter()
        .map(|id| (*id).to_string())
        .collect();
    known_ids.extend(appearance.custom_themes.iter().map(|theme| theme.id.clone()));
    appearance.profiles.retain(|id, _| known_ids.contains(id));
    for id in &known_ids {
        appearance.profiles.entry(id.clone()).or_default();
    }
    let mut known_fonts: Vec<String> = BUILTIN_FONT_IDS
        .iter()
        .map(|id| (*id).to_string())
        .collect();
    known_fonts.extend(appearance.fonts.iter().map(|font| font.id.clone()));
    for profile in appearance.profiles.values_mut() {
        if !known_fonts.iter().any(|id| id == &profile.font) {
            eprintln!("FlipClock: 字体 {:?} 不存在，回退到 bebas", profile.font);
            profile.font = default_font_id();
        }
        profile.clamp();
    }

    // 当前主题指向一个不存在的 id（删掉了、或配置被手改过）→ 回退到经典碳灰。
    let known = known_ids.contains(&appearance.theme);
    if !known {
        eprintln!("FlipClock: 主题 {:?} 不存在，回退到经典碳灰", appearance.theme);
        appearance.theme = default_theme_id();
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

/// 系统集成。开机自启的**状态**不在这里存——它以注册表为准，
/// 否则用户在任务管理器里禁用之后，配置里还会写着「开启」。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSettings {
    /// 关闭设置窗口时隐藏到托盘，而不是退出整个应用。
    #[serde(default = "yes")]
    pub close_to_tray: bool,
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self {
            close_to_tray: true,
        }
    }
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

/// 主题图片根目录。放在 settings.json 旁边，备份配置时把 themes/ 一起带走即可。
pub fn themes_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|_| AppError::ConfigDirUnavailable)?
        .join("themes");
    fs::create_dir_all(&dir).map_err(|error| AppError::ThemeDirUnavailable {
        detail: error.to_string(),
    })?;
    Ok(dir)
}

/// 导入字体根目录。与 settings.json、themes/ 并列；备份配置时一起带走。
pub fn fonts_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|_| AppError::ConfigDirUnavailable)?
        .join("fonts");
    fs::create_dir_all(&dir).map_err(|error| AppError::FontDirUnavailable {
        detail: error.to_string(),
    })?;
    Ok(dir)
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
    // 主题 id 现在是字符串，serde 不会拒绝「neon-pink」这种未知值。
    // 读盘时必须归一一次，否则手改配置或删掉自定义主题后，钟面会套用空主题。
    // 归一同时补齐/收敛每个主题的钟面配置（ADR-0022）。
    normalize_appearance(&mut settings.appearance);
    settings.screensaver = section(&value, "screensaver").unwrap_or_default();
    settings.launch = section(&value, "launch").unwrap_or_default();
    // 这一段曾经漏过 system，造成「保存成功、重启回默认」。新段必须显式加载，
    // 哪怕默认值就是想要的——漏了就是静默失败。
    settings.ui = section(&value, "ui").unwrap_or_default();
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
    use crate::errors::AppError;

    let path = config_path(app).ok_or(String::from(AppError::ConfigDirUnavailable))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::CreateConfigDirFailed {
            detail: e.to_string(),
        })?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| {
        AppError::SerializeConfigFailed {
            detail: e.to_string(),
        }
    })?;

    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| AppError::WriteConfigFailed {
        detail: e.to_string(),
    })?;
    fs::rename(&tmp, &path).map_err(|e| AppError::ReplaceConfigFailed {
        detail: e.to_string(),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_agreed_baseline() {
        let s = Settings::default();
        assert_eq!(s.appearance.theme, "classic-dark");
        assert!(s.appearance.custom_themes.is_empty());
        assert!(s.appearance.fonts.is_empty());
        let profile = &s.appearance.profiles["classic-dark"];
        assert!((profile.clock_scale - 2.25).abs() < f32::EPSILON);
        assert_eq!(profile.digit_layout, DigitLayout::PerPair);
        assert!((profile.brightness - 1.0).abs() < f32::EPSILON);
        assert_eq!(profile.font, "bebas");
        assert!(profile.sound.enabled);
        assert!((profile.sound.volume - 0.6).abs() < f32::EPSILON);
        assert_eq!(profile.format, TimeFormat::H24);
        assert!(!profile.show_date);
        assert!(!profile.show_seconds);
        assert!(!profile.show_colon);
        // 五个内置主题出厂时各有一份配置（ADR-0022）。
        assert_eq!(s.appearance.profiles.len(), BUILTIN_THEME_IDS.len());
        assert_eq!(s.screensaver.monitor_mode, MonitorMode::Primary);
        assert_eq!(s.launch.no_arg_mode, NoArgMode::Settings);
        assert!(s.system.close_to_tray);
        assert_eq!(s.ui.language, LanguagePreference::System);
    }

    #[test]
    fn ui_language_is_loaded_by_the_top_level_loader() {
        // 回归测试：system 段曾因 load() 漏掉而「保存后再读回默认」。
        // 这里直接调 load() 的逐段逻辑，钉住 ui 段不会被同样漏掉。
        let raw = serde_json::json!({
            "schemaVersion": 1,
            "ui": { "language": "en" }
        });
        let mut settings = Settings::default();
        settings.ui = section(&raw, "ui").unwrap_or_default();
        assert_eq!(settings.ui.language, LanguagePreference::En);

        let raw = serde_json::json!({ "schemaVersion": 1 });
        let settings: Settings = serde_json::from_value(raw).expect("缺 ui 段应能反序列化");
        assert_eq!(settings.ui.language, LanguagePreference::System);
    }

    #[test]
    fn ui_appearance_round_trips_and_defaults_to_system() {
        // 新增枚举字段必须能存能读：漏了 Default 或 serde 属性，表现就是
        // 「选完深色、重启静默回默认」——界面上看不出任何错误。
        let mut s = Settings::default();
        assert_eq!(s.ui.appearance, AppearancePreference::System);
        s.ui.appearance = AppearancePreference::Dark;
        let json = serde_json::to_string(&s).expect("序列化");
        assert!(json.contains("\"appearance\":\"dark\""), "{json}");
        let back: Settings = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back.ui.appearance, AppearancePreference::Dark);

        // 旧配置没有这个字段：按 system 运行，不报错也不写回。
        let raw = serde_json::json!({ "ui": { "language": "en" } });
        let ui: UiSettings = section(&raw, "ui").unwrap_or_default();
        assert_eq!(ui.appearance, AppearancePreference::System);
    }

    #[test]
    fn legacy_glass_field_is_ignored_on_load() {
        // 玻璃滑杆连同 appearance.glass 一起删掉了（ADR-0023）。旧配置里
        // 这个字段还在，必须被安静忽略，不能让整段 appearance 回落到默认值。
        let raw = serde_json::json!({
            "appearance": {
                "theme": "oled-black",
                "glass": { "transparency": 0.2 }
            }
        });
        let appearance: Appearance = section(&raw, "appearance").unwrap();
        assert_eq!(appearance.theme, "oled-black");
        // 旧配置没有配置表：serde 默认值给内置主题各补一份，再由 normalize 收敛。
        assert_eq!(appearance.profiles.len(), BUILTIN_THEME_IDS.len());
    }

    #[test]
    fn clock_profile_reads_brightness_and_font() {
        let raw = serde_json::json!({
            "brightness": 0.45,
            "font": "bahnschrift",
            "sound": { "enabled": false, "volume": 0.3 }
        });
        let profile: ClockProfile = serde_json::from_value(raw).unwrap();
        assert!((profile.brightness - 0.45).abs() < f32::EPSILON);
        assert_eq!(profile.font, "bahnschrift");
        assert!(!profile.sound.enabled);
        assert!((profile.sound.volume - 0.3).abs() < f32::EPSILON);

        assert!((clamp_brightness(0.05) - BRIGHTNESS_MIN).abs() < f32::EPSILON);
        assert!((clamp_brightness(1.8) - BRIGHTNESS_MAX).abs() < f32::EPSILON);
        assert!((clamp_brightness(0.7) - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn digit_layout_reads_kebab_case_ids() {
        let profile: ClockProfile =
            serde_json::from_value(serde_json::json!({ "digitLayout": "per-pair" })).unwrap();
        assert_eq!(profile.digit_layout, DigitLayout::PerPair);

        let profile: ClockProfile =
            serde_json::from_value(serde_json::json!({ "digitLayout": "per-digit" })).unwrap();
        assert_eq!(profile.digit_layout, DigitLayout::PerDigit);
    }

    fn sample_theme_json(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "name": "我的主题",
            "background": { "color": "#100b06", "opacity": 100, "fit": "cover", "dim": 0 },
            "card": { "material": "metallic", "color": "#c18a37", "opacity": 100 },
            "digit": { "material": "metallic", "color": "#2a1a08", "opacity": 100 },
            "date": { "color": "#d2b77f", "opacity": 100 },
            "seam": { "color": "#4a2d0e", "opacity": 100 },
            "separator": { "color": "#2a1a08", "opacity": 62 }
        })
    }

    fn sample_font_json(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "name": "测试字体",
            "file": format!("{id}.ttf"),
            "hash": "abc123"
        })
    }

    /// 自定义主题必须能被 serde 完整记住并原样读回，包括材质与边线色。
    /// 少一个字段就会静默退回默认值——用户改完主题、重启后颜色全变了。
    #[test]
    fn custom_theme_round_trips_with_materials() {
        let raw = serde_json::json!({
            "appearance": { "theme": "custom-abc", "customThemes": [sample_theme_json("custom-abc")] }
        });
        let appearance: Appearance = section(&raw, "appearance").unwrap();
        assert_eq!(appearance.custom_themes.len(), 1);
        assert_eq!(
            appearance.custom_themes[0].card.material,
            ThemeMaterial::Metallic
        );
        assert_eq!(appearance.custom_themes[0].digit.color, "#2a1a08");

        let saved = serde_json::to_value(&appearance).expect("应能序列化");
        assert_eq!(saved["customThemes"][0]["card"]["material"], "metallic");
        assert_eq!(saved["customThemes"][0]["id"], "custom-abc");
    }

    #[test]
    fn black_label_line_color_survives_a_round_trip() {
        let mut theme = sample_theme_json("custom-gold");
        theme["card"]["lineColor"] = serde_json::json!("#d2aa5b");
        let mut appearance = Appearance::default();
        appearance.custom_themes = vec![serde_json::from_value(theme).unwrap()];

        let saved = serde_json::to_value(&appearance).expect("应能序列化");
        assert_eq!(saved["customThemes"][0]["card"]["lineColor"], "#d2aa5b");
    }

    /// 非法主题整条丢弃：一个颜色字段写错不该让整份配置读不出来，
    /// 但也不该被悄悄修补成另一个样子。
    #[test]
    fn normalize_drops_invalid_themes() {
        let mut appearance = Appearance::default();
        let mut bad = sample_theme_json("custom-bad");
        bad["card"]["color"] = serde_json::json!("red");
        appearance.custom_themes = vec![
            serde_json::from_value(bad).unwrap(),
            serde_json::from_value(sample_theme_json("custom-good")).unwrap(),
        ];

        normalize_appearance(&mut appearance);
        assert_eq!(appearance.custom_themes.len(), 1);
        assert_eq!(appearance.custom_themes[0].id, "custom-good");
    }

    /// 当前主题指向一个不存在的 id 时必须回退，否则钟面会套用空主题。
    #[test]
    fn normalize_falls_back_when_current_theme_is_gone() {
        let mut appearance = Appearance::default();
        appearance.theme = String::from("custom-deleted");
        normalize_appearance(&mut appearance);
        assert_eq!(appearance.theme, "classic-dark");
    }

    /// 内置 id 不能出现在自定义列表里，否则同一个 id 会有两份定义。
    #[test]
    fn normalize_rejects_builtin_ids_in_custom_list() {        let mut appearance = Appearance::default();
        appearance.custom_themes =
            vec![serde_json::from_value(sample_theme_json("classic-dark")).unwrap()];
        normalize_appearance(&mut appearance);
        assert!(appearance.custom_themes.is_empty());
    }

    /// 配置表与主题表必须一一对应：自定义主题补默认，孤儿配置删掉（ADR-0022）。
    #[test]
    fn normalize_keeps_profiles_in_step_with_themes() {
        let mut appearance = Appearance::default();
        appearance.custom_themes =
            vec![serde_json::from_value(sample_theme_json("custom-a")).unwrap()];
        appearance
            .profiles
            .insert(String::from("custom-gone"), ClockProfile::default());

        normalize_appearance(&mut appearance);

        assert!(appearance.profiles.contains_key("custom-a"));
        assert!(!appearance.profiles.contains_key("custom-gone"));
        assert_eq!(appearance.profiles.len(), BUILTIN_THEME_IDS.len() + 1);
    }

    /// 每个主题的配置都要在保存/读盘时收敛边界，手改配置不会留下越界值。
    #[test]
    fn normalize_clamps_every_profile() {
        let mut appearance = Appearance::default();
        let mut profile = ClockProfile::default();
        profile.clock_scale = 99.0;
        profile.brightness = 0.0;
        profile.sound.volume = 8.0;
        appearance
            .profiles
            .insert(String::from("classic-dark"), profile);

        normalize_appearance(&mut appearance);

        let profile = &appearance.profiles["classic-dark"];
        assert!((profile.clock_scale - CLOCK_SCALE_MAX).abs() < f32::EPSILON);
        assert!((profile.brightness - BRIGHTNESS_MIN).abs() < f32::EPSILON);
        assert!((profile.sound.volume - SOUND_VOLUME_MAX).abs() < f32::EPSILON);
    }

    #[test]
    fn normalize_truncates_to_the_limit() {
        let mut appearance = Appearance::default();
        appearance.custom_themes = (0..40)
            .map(|index| {
                serde_json::from_value(sample_theme_json(&format!("custom-{index}"))).unwrap()
            })
            .collect();
        normalize_appearance(&mut appearance);
        assert_eq!(appearance.custom_themes.len(), CUSTOM_THEME_LIMIT);
    }

    #[test]
    fn custom_fonts_normalize_and_profiles_keep_them() {
        let mut appearance = Appearance::default();
        appearance.fonts = vec![serde_json::from_value(sample_font_json("custom-a")).unwrap()];
        appearance.profiles.get_mut("classic-dark").unwrap().font = String::from("custom-a");

        normalize_appearance(&mut appearance);

        assert_eq!(appearance.fonts.len(), 1);
        assert_eq!(appearance.profiles["classic-dark"].font, "custom-a");
    }

    #[test]
    fn dangling_font_falls_back_to_bebas() {
        let mut appearance = Appearance::default();
        appearance.profiles.get_mut("classic-dark").unwrap().font = String::from("custom-gone");

        normalize_appearance(&mut appearance);

        assert_eq!(appearance.profiles["classic-dark"].font, "bebas");
    }

    #[test]
    fn normalize_drops_invalid_and_duplicate_fonts() {
        let mut appearance = Appearance::default();
        let mut bad_file = sample_font_json("custom-b");
        bad_file["file"] = serde_json::json!("custom-other.ttf");
        let mut blank = sample_font_json("custom-c");
        blank["name"] = serde_json::json!("   ");
        appearance.fonts = vec![
            serde_json::from_value(sample_font_json("custom-a")).unwrap(),
            serde_json::from_value(sample_font_json("custom-a")).unwrap(),
            serde_json::from_value(bad_file).unwrap(),
            serde_json::from_value(blank).unwrap(),
            serde_json::from_value(sample_font_json("bebas")).unwrap(),
            serde_json::from_value(sample_font_json("custom-../evil")).unwrap(),
        ];

        normalize_appearance(&mut appearance);

        assert_eq!(appearance.fonts.len(), 2);
        assert_eq!(appearance.fonts[0].id, "custom-a");
        assert_eq!(appearance.fonts[1].id, "custom-c");
        assert_eq!(appearance.fonts[1].name, "custom-c");
    }

    #[test]
    fn normalize_truncates_fonts_to_the_limit() {
        let mut appearance = Appearance::default();
        appearance.fonts = (0..40)
            .map(|index| {
                serde_json::from_value(sample_font_json(&format!("custom-{index}"))).unwrap()
            })
            .collect();

        normalize_appearance(&mut appearance);

        assert_eq!(appearance.fonts.len(), CUSTOM_FONT_LIMIT);
    }

    #[test]
    fn font_names_reject_path_traversal() {
        assert!(is_font_name("custom-a.ttf"));
        assert!(is_font_name("custom-a.woff2"));
        assert!(!is_font_name("../../settings.json"));
        assert!(!is_font_name("custom-a.exe"));
        assert!(!is_font_name("Custom-A.ttf"));
        assert!(!is_font_name("custom-a/b.ttf"));
        assert!(!is_font_name(".ttf"));
        assert!(is_font_file_for_id("custom-a", "custom-a.otf"));
        assert!(!is_font_file_for_id("custom-a", "custom-b.otf"));
    }

    #[test]
    fn builtin_ids_are_not_valid_import_font_ids() {
        assert!(is_safe_font_id("custom-a"));
        assert!(!is_safe_font_id("bebas"));
        assert!(!is_safe_font_id("bahnschrift"));
        assert!(!is_safe_font_id("custom-../evil"));
        assert!(!is_font_file_for_id("bebas", "bebas.ttf"));
    }

    #[test]
    fn clock_profile_font_accepts_custom_ids() {
        let profile: ClockProfile =
            serde_json::from_value(serde_json::json!({ "font": "custom-a" })).unwrap();
        assert_eq!(profile.font, "custom-a");
    }

    /// id 会被拼进文件路径：大写、斜杠、点都应整条拒绝，而不是留给导入时再挡。
    #[test]
    fn normalize_rejects_unsafe_theme_ids() {
        let mut appearance = Appearance::default();
        appearance.custom_themes = vec![
            serde_json::from_value(sample_theme_json("custom-../evil")).unwrap(),
            serde_json::from_value(sample_theme_json("Custom-Upper")).unwrap(),
            serde_json::from_value(sample_theme_json("custom-ok")).unwrap(),
        ];
        normalize_appearance(&mut appearance);
        assert_eq!(appearance.custom_themes.len(), 1);
        assert_eq!(appearance.custom_themes[0].id, "custom-ok");
    }

    /// 空名字与纯空白名字都不是可用主题：界面上会变成一行看不见的条目。
    #[test]
    fn normalize_drops_themes_without_a_name() {
        let mut appearance = Appearance::default();
        let mut blank = sample_theme_json("custom-blank");
        blank["name"] = serde_json::json!("   ");
        appearance.custom_themes = vec![serde_json::from_value(blank).unwrap()];
        normalize_appearance(&mut appearance);
        assert!(appearance.custom_themes.is_empty());
    }

    /// 重名不丢数据，改成带序号的名字，且不能把长度顶出上限。
    #[test]
    fn normalize_renames_duplicate_names() {
        let mut appearance = Appearance::default();
        appearance.custom_themes = vec![
            serde_json::from_value(sample_theme_json("custom-a")).unwrap(),
            serde_json::from_value(sample_theme_json("custom-b")).unwrap(),
        ];
        normalize_appearance(&mut appearance);
        assert_eq!(appearance.custom_themes[0].name, "我的主题");
        assert_eq!(appearance.custom_themes[1].name, "我的主题 2");
        assert!(appearance
            .custom_themes
            .iter()
            .all(|theme| theme.name.chars().count() <= THEME_NAME_MAX_CHARS));
    }

    /// 下限是 0（0 表示「不要这个部件」），所以越界只可能来自上限或负数。
    /// 两端都要钳：负数会让 rgba 的 alpha 变成非法值。
    #[test]
    fn normalize_clamps_opacity_on_both_ends() {
        let mut appearance = Appearance::default();
        let mut theme = sample_theme_json("custom-dim");
        theme["card"]["opacity"] = serde_json::json!(150);
        theme["digit"]["opacity"] = serde_json::json!(-20);
        appearance.custom_themes = vec![serde_json::from_value(theme).unwrap()];

        normalize_appearance(&mut appearance);
        assert!((appearance.custom_themes[0].card.opacity - 100.0).abs() < f32::EPSILON);
        assert!((appearance.custom_themes[0].digit.opacity - 0.0).abs() < f32::EPSILON);
        assert_eq!(CARD_OPACITY_MIN, 0.0);
        assert_eq!(PART_OPACITY_MIN, 0.0);
    }

    /// 图片文件名是唯一会被拼进路径的外部输入，必须挡住路径穿越。
    #[test]
    fn image_names_reject_path_traversal() {
        assert!(is_image_name("background-1700000000.png"));
        assert!(is_image_name("digit-1.webp"));
        assert!(!is_image_name("../../settings.json"));
        assert!(!is_image_name("background-1.exe"));
        assert!(!is_image_name("Background-1.png"));
        assert!(!is_image_name("a/b.png"));
        assert!(!is_image_name(".png"));
    }

    #[test]
    fn old_settings_without_system_section_default_to_close_to_tray() {
        let raw = serde_json::json!({ "appearance": { "theme": "oled-black" } });
        let settings: Settings = serde_json::from_value(raw).expect("老配置应能反序列化");
        // 老配置没有 system 段时，缺省行为必须是「关窗退到托盘」，否则托盘成了孤儿。
        assert!(settings.system.close_to_tray);
    }

    #[test]
    fn clock_profile_without_show_date_falls_back_to_hidden() {
        // 逐字段 serde 默认必须与 ClockProfile::default() 一致，否则缺字段的配置
        // （旧版本、手改文件）会把日期行重新打开，与「默认只留钟面」冲突。
        let raw = serde_json::json!({ "showSeconds": true });
        let profile: ClockProfile = serde_json::from_value(raw).unwrap();
        assert!(!profile.show_date);
        assert!(profile.show_seconds);
    }

    #[test]
    fn clock_profile_without_sound_falls_back_to_defaults() {
        let profile: ClockProfile =
            serde_json::from_value(serde_json::json!({ "format": "24h" })).unwrap();
        assert!(profile.sound.enabled);
        assert!((profile.sound.volume - 0.6).abs() < f32::EPSILON);
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
    fn unknown_theme_id_is_normalized_back_to_default() {
        let raw = serde_json::json!({
            "schemaVersion": 1,
            "appearance": { "theme": "neon-pink" },
            "time": { "format": "12h", "showDate": false, "showSeconds": true }
        });
        let mut settings = Settings::default();
        settings.appearance = section(&raw, "appearance").unwrap_or_default();

        // 字符串形式的 theme 能通过 serde，所以收敛必须发生在 normalize 里。
        assert_eq!(settings.appearance.theme, "neon-pink");
        normalize_appearance(&mut settings.appearance);
        assert_eq!(settings.appearance.theme, "classic-dark");
        // v1 的 time 段不再读取；回退后的主题仍有一份默认配置。
        assert!(settings.appearance.profiles.contains_key("classic-dark"));
        assert_eq!(
            settings.appearance.profiles["classic-dark"].format,
            TimeFormat::H24
        );
    }

    #[test]
    fn missing_appearance_section_gets_defaults_and_full_profiles() {
        let raw = serde_json::json!({ "schemaVersion": 1 });
        let settings: Settings = serde_json::from_value(raw).unwrap();
        assert_eq!(settings.appearance.theme, "classic-dark");
        assert_eq!(settings.appearance.profiles.len(), BUILTIN_THEME_IDS.len());
    }
}
