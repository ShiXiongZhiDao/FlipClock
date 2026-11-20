/**
 * 主题样卡的底色与文字色。
 *
 * 底色取该主题「卡片顶面」最终渲染出来的颜色（含材质提亮），名字与摘要取数字色——
 * 这样深色主题上的文字仍然是亮色、浅金卡上是深棕，不依赖界面自己的灰阶配色。
 * 设置页主题墙与全屏配置主题墙共用这一份，两处不能再各写一遍（曾经就是各写一遍，
 * 改一处另一处不跟）。
 */
import type { ThemeSpec } from "../../types/settings";
import { buildThemeVars } from "./material";

export function swatchStyle(theme: ThemeSpec): Record<string, string> {
  const vars = buildThemeVars(theme);
  return {
    background: vars["--card-top"] ?? theme.card.color,
    color: vars["--digit"] ?? theme.digit.color,
  };
}
