/**
 * 主题的读取入口：把配置里的 id 解析成规格，并给出图片地址。
 *
 * 所有需要画钟面的地方都用它，保证「当前主题」只有一种解释：
 * 外观来自 `ThemeSpec`，显示与声音来自 `ClockProfile`（ADR-0022）。
 */
import { computed } from "vue";

import { themeAssetUrl } from "../lib/theme/assets";
import { BUILTIN_THEME_IDS, BUILTIN_THEMES, resolveTheme } from "../lib/theme/presets";
import { profileOf } from "../lib/theme/profile";
import type { ThemeSpec } from "../types/settings";
import { useSettings } from "./useSettings";

export function useTheme() {
  const { settings } = useSettings();

  /** 全部可选主题：内置预设在前，自定义在后。 */
  const all = computed<ThemeSpec[]>(() => [
    ...BUILTIN_THEME_IDS.map((id) => BUILTIN_THEMES[id]),
    ...settings.appearance.customThemes,
  ]);

  /** 当前主题。id 找不到时退回经典碳灰，不会返回空。 */
  const current = computed<ThemeSpec>(() =>
    resolveTheme(settings.appearance.theme, settings.appearance.customThemes),
  );

  /** 当前主题的钟面配置：时间格式、排布、字体、缩放、亮度、翻页音。 */
  const profile = computed(() => profileOf(settings));

  const backgroundUrl = computed(() =>
    themeAssetUrl(current.value.id, current.value.background.image),
  );
  const digitUrl = computed(() =>
    themeAssetUrl(current.value.id, current.value.digit.image),
  );

  return { all, current, profile, backgroundUrl, digitUrl };
}
