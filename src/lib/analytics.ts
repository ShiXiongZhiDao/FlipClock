/**
 * 匿名使用统计（ADR-0019）。
 *
 * 只上报三件事：应用被启动、启动时用户实际在用什么样的时钟、有没有安装屏保。
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
  FeatureToggled: "feature_toggled",
  /** @deprecated 2026-11-23 精简：设置窗口打开率无决策价值，与启动数重复。 */
  SettingsPageViewed: "settings_page_viewed",
  /** @deprecated 2026-11-23 精简：更新检查属运维指标，不服务产品方向。 */
  UpdateChecked: "update_checked",
} as const;

export type AnalyticsProps = Record<string, string | number>;

/** 属性只允许这些枚举集合；任何别的值都被清洗掉，防止自由文本混进看板。 */
const ALLOWED: Record<string, ReadonlySet<string | number>> = {
  // 从内置主题表派生：新增主题时不必记得同步这里，也不会漏记成 [redacted]。
  theme: new Set<string>(BUILTIN_THEME_IDS),
  digitLayout: new Set(["per-digit", "per-pair"]),
  feature: new Set(["screensaver"]),
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
  });
}

/** 目前只剩屏保安装这一个功能事件；参数仍留联合类型，新增时不必改调用签名。 */
export function trackFeatureToggled(feature: "screensaver", enabled: boolean): void {
  track(Events.FeatureToggled, { feature, enabled: enabled ? 1 : 0 });
}
