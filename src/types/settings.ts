/** 内置主题的固定 id。自定义主题是 `custom-<uuid>`，不在这个联合里。 */
export type BuiltinThemeId =
  | "classic-dark"
  | "oled-black"
  | "retro-light"
  | "mirror-gold"
  | "black-label";

/**
 * 主题 id：内置 id 或自定义 id。配置里存的就是这个字符串。
 * 用 string 而不是联合同，是因为自定义主题的 id 在运行期才产生。
 */
export type ThemeId = string;

/** 部件表面的光影结构。材质决定明暗怎么分布，颜色只决定色相。 */
export type ThemeMaterial = "matte" | "metallic" | "soft";

/** 背景图的铺法：铺满（可能裁切）或平铺（原尺寸重复）。 */
export type BackgroundFit = "cover" | "tile";

/** 有材质的部件：卡片与数字。 */
export interface MaterialPart {
  material: ThemeMaterial;
  /** #RRGGBB。 */
  color: string;
  /** 不透明度 0–100。下限一律 0：0 表示「不要这个部件」。 */
  opacity: number;
  /**
   * 金属材质的边线色。不设则由主色提亮推导。
   * 黑标典藏金是「近黑卡 + 金边」，边线无法从近黑提亮得来（只会得到灰），
   * 所以边线必须能独立指定。
   */
  lineColor?: string;
}

/** 只有颜色的部件：日期行与分割线。 */
export interface FlatPart {
  color: string;
  opacity: number;
}

export interface BackgroundSpec {
  color: string;
  opacity: number;
  /** 图片文件名（不是路径）。null 表示纯色。 */
  image: string | null;
  fit: BackgroundFit;
  /** 压暗蒙版 0–70，保证亮色图片上的数字仍可读。 */
  dim: number;
}

export interface DigitSpec extends MaterialPart {
  /** 纹理图文件名；铺满每个数字，建议用均匀材质。 */
  image: string | null;
}

/**
 * 一份主题的完整外观。内置主题与自定义主题用同一个结构，
 * 「复制为新主题」就是深拷贝它再换 id 与名字。
 */
export interface ThemeSpec {
  id: ThemeId;
  /** 自定义主题是用户输入；内置主题留空，界面上按 id 查 i18n。 */
  name: string;
  background: BackgroundSpec;
  card: MaterialPart;
  digit: DigitSpec;
  date: FlatPart;
  /** 翻页卡中间那条横向的缝（把上下两片分开的线）。 */
  seam: FlatPart;
  /** 时间数字之间的冒号圆点。 */
  separator: FlatPart;
}

/**
 * 不透明度区间。两者下限都是 0：卡片调到 0 就是「不要卡片」，只留数字；
 * 分割线调到 0 就是把冒号去掉。全透明时翻页叠加的加深问题自然消失
 * （两层都看不见），所以不再需要原来那条 60% 的下限。
 */
export const CARD_OPACITY_MIN = 0;
export const PART_OPACITY_MIN = 0;
export const OPACITY_MAX = 100;

/** 背景压暗上限。再高压原图就几乎看不见了。 */
export const BACKGROUND_DIM_MAX = 70;

/** 自定义主题数量与名字长度上限。 */
export const CUSTOM_THEME_LIMIT = 36;
export const THEME_NAME_MAX = 16;

export type TimeFormat = "24h" | "12h";
/** 界面语言偏好。system = 跟随系统，启动时归并为中文或英文。 */
export type LanguagePreference = "system" | "zh" | "en";
/**
 * 界面外观偏好（ADR-0025）：只决定设置窗口外壳的明暗，不碰钟面。
 * system 由 `prefers-color-scheme` 实时归并，不写盘、不重启。
 */
export type UiAppearance = "system" | "light" | "dark";
export type MonitorMode = "primary" | "all";
export type NoArgMode = "settings" | "fullscreen";
/** 每张翻页卡承载几位数字。per-digit = 一位一卡，per-pair = 两位一卡（方卡）。 */
export type DigitLayout = "per-digit" | "per-pair";

/** 数字字体。三款全部系统自带或随包，不引入新资产。 */
export type DigitFont = "bebas" | "bahnschrift" | "system";

/** 时钟缩放比例：只作用于全屏时钟与屏保；左栏预览按阻尼系数演示，见 useClockScale。 */
export const CLOCK_SCALE_MIN = 0.5;
export const CLOCK_SCALE_MAX = 4.5;
export const CLOCK_SCALE_STEP = 0.05;

/**
 * 16:9 屏幕上「时钟刚好占满屏幕宽度」的缩放值。
 *
 * 设置窗口左栏与主题卡都用它换算占屏比例（`clockScale / CLOCK_FIT_SCALE`），
 * 同时也标在缩放滑杆上当参照点：「铺满 16:9 约 241%」。
 *
 * 推导（一位一卡，不带秒）：一行 = 4×16.5 + **2**×1.15 + 2×2.3 + 0.75 = 73.65u
 *（每组两张卡，卡缝只有两个），u = 1vmin × scale；16:9 屏宽 = 177.78vmin。
 * 令 73.65 × scale = 177.78 得 2.414；两位一卡是 73.5u → 2.419。
 * 两个值差 0.2%，取小者 2.41 让两种排布都不出屏，所以两套排布共用一个铺满点；
 * 带秒时 unit 会被 --row-fit 收回，行宽仍回到 73.65u，同一个值继续成立。
 *
 * 旧值 1.234 是从「面板行宽占容器宽 0.4806」反推的，而面板本身是自适应尺寸，
 * 这个比值不构成参照——它让提示说「123% 铺满」，实际 123% 只占屏宽 44%，
 * 用户按提示调到 123% 会看到明显偏小的钟。
 */
export const CLOCK_FIT_SCALE = 2.41;

/** 翻页音效音量（0 = 静音，1 = 最大）。 */
export const SOUND_VOLUME_MIN = 0;
export const SOUND_VOLUME_MAX = 1;

/** 时钟亮度：只压暗，不提亮。 */
export const BRIGHTNESS_MIN = 0.2;
export const BRIGHTNESS_MAX = 1;

/**
 * 一个主题的钟面配置（ADR-0022）。
 *
 * 决定「怎么显示、要不要响」，不决定「长什么样」——后者是 `ThemeSpec`。
 * 两者按主题 id 组合成用户看到的完整钟面；切换主题即整套切换。
 */
export interface ClockProfile {
  format: TimeFormat;
  showDate: boolean;
  showSeconds: boolean;
  showColon: boolean;
  digitLayout: DigitLayout;
  font: DigitFont;
  clockScale: number;
  brightness: number;
  sound: { enabled: boolean; volume: number };
}

/** 出厂默认：只留钟面，声音开、音量 0.6。 */
export const DEFAULT_CLOCK_PROFILE: ClockProfile = {
  format: "24h",
  showDate: false,
  showSeconds: false,
  showColon: false,
  digitLayout: "per-pair",
  font: "bebas",
  clockScale: 2.25,
  brightness: 1,
  sound: { enabled: true, volume: 0.6 },
};

export interface Settings {
  schemaVersion: number;
  appearance: {
    /** 当前主题 id：内置 id 或 customThemes 里某一条的 id。 */
    theme: ThemeId;
    /** 用户自定义的主题（ADR-0020）。上限 CUSTOM_THEME_LIMIT。 */
    customThemes: ThemeSpec[];
    /** 每个主题自己的钟面配置（ADR-0022），按主题 id 索引。 */
    profiles: Record<ThemeId, ClockProfile>;
  };
  screensaver: { monitorMode: MonitorMode };
  /** 系统集成。开机自启的状态不在这里，它以注册表为准。 */
  system: { closeToTray: boolean };
  /** 界面文字语言。只影响文字，不影响钟面数字的字体。 */
  ui: { language: LanguagePreference; appearance: UiAppearance };
  launch: { noArgMode: NoArgMode };
  registration?: {
    rollbackScrnsaveExe: string | null;
    rollbackScreenSaveActive: string | null;
    installedPath: string | null;
  };
}

export interface SettingsPatch {
  appearance: Settings["appearance"];
  screensaver: Settings["screensaver"];
  launch: Settings["launch"];
  system: Settings["system"];
  ui: Settings["ui"];
}

export type RegistrationState = "notInstalled" | "installed" | "missing" | "outdated";

export interface ScreenSaverStatus {
  state: RegistrationState;
  targetPath: string;
  activePath: string | null;
  rollbackAvailable: boolean;
}

export const THEME_LABELS: Record<BuiltinThemeId, import("../lib/i18n").MessageKey> = {
  "classic-dark": "appearance.theme.classicDark",
  "oled-black": "appearance.theme.oledBlack",
  "retro-light": "appearance.theme.retroLight",
  "mirror-gold": "appearance.theme.mirrorGold",
  "black-label": "appearance.theme.blackLabel",
};

export const FONT_LABELS: Record<DigitFont, import("../lib/i18n").MessageKey> = {
  bebas: "appearance.font.bebas",
  bahnschrift: "appearance.font.bahnschrift",
  system: "appearance.font.systemDefault",
};

export function defaultSettings(): Settings {
  // 五个内置主题出厂各带一份默认钟面配置，与 Rust 侧 default_profiles 对齐。
  const profiles: Record<string, ClockProfile> = {};
  for (const id of Object.keys(THEME_LABELS) as BuiltinThemeId[]) {
    profiles[id] = { ...DEFAULT_CLOCK_PROFILE, sound: { ...DEFAULT_CLOCK_PROFILE.sound } };
  }
  return {
    schemaVersion: 3,
    appearance: {
      theme: "classic-dark",
      customThemes: [],
      profiles,
    },
    screensaver: { monitorMode: "primary" },
    launch: { noArgMode: "settings" },
    system: { closeToTray: true },
    ui: { language: "system", appearance: "system" },
  };
}
