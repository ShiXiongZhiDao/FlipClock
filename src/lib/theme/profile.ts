/**
 * 主题与钟面配置的组合（ADR-0022）。
 *
 * `ThemeSpec` 决定长什么样，`ClockProfile` 决定怎么显示、是否发声；
 * 两者按主题 id 配对。所有需要「当前钟面」的地方都应该经过这里，
 * 而不是各自去翻 `settings.appearance.profiles`。
 */
import {
  DEFAULT_CLOCK_PROFILE,
  type ClockProfile,
  type Settings,
} from "../../types/settings";
import { t } from "../i18n";
import { BUILTIN_THEME_IDS } from "./presets";

/** 深拷贝一份配置，避免多个主题共享同一个 sound 对象。 */
export function cloneProfile(profile: ClockProfile = DEFAULT_CLOCK_PROFILE): ClockProfile {
  return { ...profile, sound: { ...profile.sound } };
}

/**
 * 配置缺失时的兜底对象。
 *
 * 它只服务于「读到了坏配置」这一种情况：界面能正常显示，改动留在这份兜底里，
 * 不会污染 `DEFAULT_CLOCK_PROFILE`，也不会被静默写回磁盘（save 走的是
 * `settings.appearance.profiles`）。正常路径由 ensureProfiles 补齐。
 */
const FALLBACK_PROFILE = cloneProfile();

/**
 * 当前主题的钟面配置。
 *
 * 读盘与新增主题时都会补齐，所以正常路径不会走到默认值；真的缺失时返回
 * 兜底对象并留一条警告。
 */
export function profileOf(settings: Settings): ClockProfile {
  const profile = settings.appearance.profiles[settings.appearance.theme];
  if (!profile) {
    console.warn(`missing clock profile for theme ${settings.appearance.theme}; using defaults`);
    return FALLBACK_PROFILE;
  }
  return profile;
}

/** 指定主题的配置；缺失时返回默认值（调用方通常在 ensureProfiles 之后使用）。 */
export function profileFor(settings: Settings, id: string): ClockProfile {
  return settings.appearance.profiles[id] ?? FALLBACK_PROFILE;
}

/** 哪些主题的钟面配置正在使用这款字体；删除字体确认框用。 */
export function themeIdsUsingFont(settings: Settings, fontId: string): string[] {
  return Object.entries(settings.appearance.profiles)
    .filter(([, profile]) => profile.font === fontId)
    .map(([themeId]) => themeId);
}

/** 音量滑杆的统一规则：拖到 0 就是静音，拖回非零顺手打开开关。 */
export function setSoundVolume(profile: ClockProfile, percent: number): void {
  profile.sound.volume = percent / 100;
  profile.sound.enabled = percent > 0;
}

/** 让配置表与主题表一一对应：多出来的删掉，缺的补默认。 */
export function ensureProfiles(settings: Settings): void {
  const ids = new Set<string>([
    ...BUILTIN_THEME_IDS,
    ...settings.appearance.customThemes.map((theme) => theme.id),
  ]);
  for (const id of Object.keys(settings.appearance.profiles)) {
    if (!ids.has(id)) delete settings.appearance.profiles[id];
  }
  for (const id of ids) {
    if (!settings.appearance.profiles[id]) {
      settings.appearance.profiles[id] = cloneProfile();
    }
  }
}

/**
 * 主题卡上的一行摘要：切换主题会连带改变这些，先让用户看见。
 * 顺序固定，避免不同主题卡之间无法横向比较。
 */
export function profileSummary(settings: Settings, themeId: string): string {
  const profile = profileFor(settings, themeId);
  const parts = [profile.format === "12h" ? "12H" : "24H"];
  if (profile.showDate) parts.push(t("profile.date"));
  if (profile.showSeconds) parts.push(t("profile.seconds"));
  if (profile.showColon) parts.push(t("profile.colon"));
  parts.push(
    profile.sound.enabled
      ? t("profile.sound", { volume: Math.round(profile.sound.volume * 100) })
      : t("profile.muted"),
  );
  parts.push(`${Math.round(profile.clockScale * 100)}%`);
  return parts.join(" · ");
}
