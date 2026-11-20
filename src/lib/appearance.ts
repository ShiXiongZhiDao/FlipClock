/**
 * 界面外观（ADR-0025）：把三态偏好落成两个实际值，写到 `<html>` 上。
 *
 * 只有一件事：`data-appearance="light|dark"`——CSS 只认这两个值，
 * 两套色板因此不需要在样式表里再写一遍 media query。
 *
 * **为什么不推给窗口主题**：`tauri::WebviewWindow::set_theme()` 在运行期只转给
 * tao（改原生边框），不碰 WebView2 的 preferred color scheme（wry 只在建窗时调
 * `SetPreferredColorScheme`）。也就是说运行期改窗口主题改不动 `prefers-color-scheme`，
 * 反而会在「深色 → 跟随系统」时把媒体查询永久钉在深色。窗口主题保持默认的 Auto，
 * 媒体查询就一直是系统的真实值；显式浅/深由 CSS 属性负责，
 * 原生滚动条由同一份 `color-scheme` 负责。
 */
import type { UiAppearance } from "../types/settings";

export type ResolvedAppearance = "light" | "dark";

const media = window.matchMedia("(prefers-color-scheme: dark)");

/** 纯函数，便于测试与在其它窗口里推导同一个值。 */
export function resolveAppearance(pref: UiAppearance, systemDark: boolean): ResolvedAppearance {
  if (pref === "dark") return "dark";
  if (pref === "light") return "light";
  return systemDark ? "dark" : "light";
}

function paint(pref: UiAppearance): void {
  const resolved = resolveAppearance(pref, media.matches);
  document.documentElement.dataset.appearance = resolved;
  // 原生滚动条、下拉与输入控件按 color-scheme 渲染。样式表里也写了同一条，
  // 这里再写一次是为了首帧：属性刚落到 <html> 上时，CSS 规则可能还没生效。
  document.documentElement.style.colorScheme = resolved;
}

let current: UiAppearance | null = null;

function followSystem(): void {
  if (current) paint(current);
}

/**
 * 应用一份偏好。可重复调用：监听器用的是同一个函数引用，重复 add 不会叠加。
 * 设置窗口与全屏配置窗口各调一次——它们不共享 JS 上下文。
 */
export function applyAppearance(pref: UiAppearance): void {
  current = pref;
  paint(pref);
  if (pref === "system") media.addEventListener("change", followSystem);
  else media.removeEventListener("change", followSystem);
}
