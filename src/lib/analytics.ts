/**
 * 匿名使用统计（ADR-0019）。
 *
 * 只上报开发者判断产品方向需要的少量事实：应用被启动、设置窗口被看过、
 * 哪个功能被切换、有没有查更新，以及启动时用户实际在用什么样的时钟。
 * 没有用户 ID、设备 ID、使用时长，也不上报任何自由文本。
 *
 * 插件本身不自动采集事件、也没有运行时开关；所有事件都从这里手动发出。
 * 调用一律 fire-and-forget：统计失败绝不能影响用户操作，但失败会在控制台留痕。
 */
import { invoke } from "@tauri-apps/api/core";

import { isTauri } from "./env";
import { BUILTIN_THEME_IDS } from "./theme/presets";
import { profileOf } from "./theme/profile";

/** 事件名集中在此，删事件时保留常量标 @deprecated，不要在别处写裸字符串。 */
export const Events = {
  AppStarted: "app_started",
  ConfigurationSnapshot: "configuration_snapshot",
  SettingsPageViewed: "settings_page_viewed",
  FeatureToggled: "feature_toggled",
  UpdateChecked: "update_checked",
} as const;

export type AnalyticsProps = Record<string, string | number>;

/** 属性只允许这些枚举集合；任何别的值都被清洗掉，防止自由文本混进看板。 */
const ALLOWED: Record<string, ReadonlySet<string | number>> = {
  // 从内置主题表派生：新增主题时不必记得同步这里，也不会漏记成 [redacted]。
  theme: new Set<string>(BUILTIN_THEME_IDS),
  digitLayout: new Set(["per-digit", "per-pair"]),
  showSeconds: new Set([0, 1]),
  showDate: new Set([0, 1]),
  feature: new Set(["screensaver", "sound", "language", "uiAppearance"]),
  // 三态原样上报：压成 0/1 会让「浅色」和「深色」在看板上无法区分。
  state: new Set(["system", "light", "dark"]),
  enabled: new Set([0, 1]),
};

/**
 * 清洗属性：只留白名单里的键与枚举值，其余替换为 [redacted]。
 * 与 EdgePomo 前后端同款思路——宁可在看板上看到 [redacted]，
 * 也不要让一个路径或错误原文悄悄进到第三方。
 */
function sanitize(props: AnalyticsProps): AnalyticsProps {
  const out: AnalyticsProps = {};
  for (const [key, value] of Object.entries(props)) {
    const allowed = ALLOWED[key];
    out[key] = allowed && allowed.has(value) ? value : "[redacted]";
  }
  return out;
}

function track(name: string, props?: AnalyticsProps): void {
  if (!isTauri()) return;
  // 不 await、不 throw：统计管线出问题不该在界面上暴露，也不该阻断任何操作。
  void invoke("plugin:aptabase|track_event", {
    name,
    props: props ? sanitize(props) : null,
  }).catch((error) => {
    console.warn("[analytics] 上报失败", name, error);
  });
}

/**
 * 启动事件去重。不能用模块级变量：Tauri 每个窗口是独立 WebView，
 * 设置窗口开着再从托盘点「全屏时钟」时两个 WebView 各有一份模块状态，
 * 会重复上报。localStorage 同源共享，但会跨进程保留，所以再叠一个时间窗：
 * 窗内视为同一次启动，窗外视为新进程。
 */
const START_WINDOW_MS = 5 * 60 * 1000;
const START_STAMP_KEY = "flipclock.analytics.startedAt";

function claimStart(): boolean {
  try {
    const last = Number(localStorage.getItem(START_STAMP_KEY) ?? 0);
    const now = Date.now();
    if (now - last < START_WINDOW_MS) return false;
    localStorage.setItem(START_STAMP_KEY, String(now));
    return true;
  } catch {
    // 存储不可用（隐私模式等）时退化成每次都报：宁可多算，不要一条都不发。
    return true;
  }
}

export function trackAppStarted(clock: import("../types/settings").Settings): void {
  if (!claimStart()) return;
  const profile = profileOf(clock);
  track(Events.AppStarted);
  // 配置快照随启动上报一次：默认配置也计入，反映「用户在用什么时钟」。
  // 运行期改了设置下次启动才体现，这是明确口径（ADR-0019）。
  track(Events.ConfigurationSnapshot, {
    theme: clock.appearance.theme,
    digitLayout: profile.digitLayout,
    showSeconds: profile.showSeconds ? 1 : 0,
    showDate: profile.showDate ? 1 : 0,
  });
}

/**
 * 设置页被打开。ADR-0019 的口径是「一次会话一次」，而设置窗口每次关闭是
 * destroy、重开是新建窗口，所以必须跨窗口去重，理由同 trackAppStarted。
 */
const SETTINGS_VIEW_KEY = "flipclock.analytics.settingsViewedAt";

export function trackSettingsPageViewed(): void {
  try {
    const last = Number(localStorage.getItem(SETTINGS_VIEW_KEY) ?? 0);
    if (Date.now() - last < START_WINDOW_MS) return;
    localStorage.setItem(SETTINGS_VIEW_KEY, String(Date.now()));
  } catch {
    // 存储不可用也照发，理由同上。
  }
  track(Events.SettingsPageViewed);
}

export function trackFeatureToggled(
  feature: "screensaver" | "sound" | "language",
  enabled: boolean,
): void {
  track(Events.FeatureToggled, { feature, enabled: enabled ? 1 : 0 });
}

/** 界面外观是三态，不能套用「开/关」那套布尔埋点。 */
export function trackUiAppearance(appearance: "system" | "light" | "dark"): void {
  track(Events.FeatureToggled, { feature: "uiAppearance", state: appearance });
}

export function trackUpdateChecked(): void {
  track(Events.UpdateChecked);
}
