import { getCurrentWindow, primaryMonitor } from "@tauri-apps/api/window";

import { isTauri } from "./env";

/**
 * 预览舞台（主题样卡、左栏预览）按**主屏宽高比**画。
 *
 * 全屏时钟与屏保用视口短边（vmin）推导几何；预览卡要「就是一块小屏」，
 * 就必须和主屏同比例，否则换一台不同比例的显示器后，两侧可见范围会和真屏对不上。
 * 主屏查询失败时沿用窗口所在屏；都拿不到时 CSS 里兜底 16:9。
 */
export async function applyStageAspect(): Promise<void> {
  const root = document.documentElement;
  const set = (width: number, height: number) => {
    if (width > 0 && height > 0) {
      root.style.setProperty("--screen-aspect", `${width} / ${height}`);
    }
  };

  // 先同步给一个值，避免首帧用错比例；Tauri 里再用主屏校正。
  set(window.screen.width, window.screen.height);
  if (!isTauri()) return;
  try {
    const monitor = await primaryMonitor();
    if (monitor) set(monitor.size.width, monitor.size.height);
  } catch {
    // 查询失败就沿用窗口所在屏的比例。
  }
}

let watching = false;

/**
 * 显示器比例可能在运行期变化（改分辨率、切换主屏）。
 * 窗口尺寸/位置一变就重读，防抖合并连续事件；只在需要舞台的窗口调用。
 */
export function watchStageAspect(): void {
  if (watching) return;
  watching = true;

  let timer: ReturnType<typeof setTimeout> | undefined;
  const refresh = () => {
    clearTimeout(timer);
    timer = setTimeout(() => void applyStageAspect(), 200);
  };

  void applyStageAspect();
  window.addEventListener("resize", refresh);
  if (!isTauri()) return;

  const win = getCurrentWindow();
  void win.onResized(refresh).catch(() => {});
  void win.onMoved(refresh).catch(() => {});
}