export type Theme = "classic-dark" | "oled-black" | "retro-light";
export type TimeFormat = "24h" | "12h";
export type MonitorMode = "primary" | "all";
export type NoArgMode = "settings" | "fullscreen";
/** 每张翻页卡承载几位数字。per-digit = 一位一卡，per-pair = 两位一卡（方卡）。 */
export type DigitLayout = "per-digit" | "per-pair";

/** 数字字体。三款全部系统自带或随包，不引入新资产。 */
export type DigitFont = "bebas" | "bahnschrift" | "system";

/** 界面玻璃透明度（0 = 实心，1 = 全透）。前后端共用这一组边界。 */
export const GLASS_TRANSPARENCY_MIN = 0.05;
export const GLASS_TRANSPARENCY_MAX = 0.75;

/** 时钟缩放比例：只作用于全屏时钟与屏保；预览卡用它演示相对大小。 */
export const CLOCK_SCALE_MIN = 0.5;
export const CLOCK_SCALE_MAX = 4.5;
export const CLOCK_SCALE_STEP = 0.05;

/**
 * 16:9 屏幕上「时钟刚好占满屏幕宽度」的缩放值。
 * 只用标定预览卡：超过它，真实时钟和预览卡都会开始被裁切。
 * 两位一卡的卡宽是按行宽对齐构造的，所以两种排布共用这一个铺满点。
 */
export const CLOCK_FIT_SCALE = 1.57;

/** 翻页音效音量（0 = 静音，1 = 最大）。 */
export const SOUND_VOLUME_MIN = 0;
export const SOUND_VOLUME_MAX = 1;

/** 时钟亮度：只压暗，不提亮。 */
export const BRIGHTNESS_MIN = 0.2;
export const BRIGHTNESS_MAX = 1;

export interface Settings {
  schemaVersion: number;
  appearance: {
    theme: Theme;
    glass: { transparency: number };
    clockScale: number;
    digitLayout: DigitLayout;
    brightness: number;
    font: DigitFont;
  };
  time: { format: TimeFormat; showDate: boolean; showSeconds: boolean; showColon: boolean };
  sound: { enabled: boolean; volume: number };
  screensaver: { monitorMode: MonitorMode };
  launch: { noArgMode: NoArgMode };
  registration?: {
    rollbackScrnsaveExe: string | null;
    rollbackScreenSaveActive: string | null;
    installedPath: string | null;
  };
}

export interface SettingsPatch {
  appearance: Settings["appearance"];
  time: Settings["time"];
  sound: Settings["sound"];
  screensaver: Settings["screensaver"];
  launch: Settings["launch"];
}

export type RegistrationState = "notInstalled" | "installed" | "missing" | "outdated";

export interface ScreenSaverStatus {
  state: RegistrationState;
  targetPath: string;
  activePath: string | null;
  rollbackAvailable: boolean;
}

export const THEME_LABELS: Record<Theme, string> = {
  "classic-dark": "经典碳灰",
  "oled-black": "OLED 纯黑",
  "retro-light": "复古暖白",
};

export const FONT_LABELS: Record<DigitFont, string> = {
  bebas: "Bebas Neue",
  bahnschrift: "Bahnschrift",
  system: "系统默认",
};

export function defaultSettings(): Settings {
  return {
    schemaVersion: 1,
    appearance: {
      theme: "classic-dark",
      glass: { transparency: 0.48 },
      clockScale: 2.25,
      digitLayout: "per-pair",
      brightness: 1,
      font: "bebas",
    },
    time: { format: "24h", showDate: false, showSeconds: false, showColon: false },
    sound: { enabled: true, volume: 0.6 },
    screensaver: { monitorMode: "primary" },
    launch: { noArgMode: "settings" },
  };
}
