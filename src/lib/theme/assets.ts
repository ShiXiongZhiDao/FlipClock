/**
 * 主题图片的本地地址。
 *
 * 图片存在 `<app_config_dir>/themes/<主题id>/<文件名>`，配置里只存文件名。
 * 需要把绝对路径转成 WebView 能加载的地址，所以启动时向 Rust 取一次根目录。
 */
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { ref } from "vue";

import { isTauri } from "../env";

// 用 ref 而不是普通变量：路径是异步取的，模板里的 themeAssetUrl 要能追踪到它，
// 否则路径到位后画面不会重算，图片永远不显示。
const base = ref<string | null>(null);
let ready: Promise<void> | null = null;
const cache = new Map<string, string>();

/**
 * 取一次主题图片根目录。所有窗口形态（设置 / 全屏 / 屏保）都要调，
 * 所以放在 useSettings 的 load 里，而不是某个视图的 onMounted。
 */
export function initThemeAssets(): Promise<void> {
  ready ??= (async () => {
    if (!isTauri()) return;
    base.value = await invoke<string>("themes_dir_path").catch(() => null);
  })();
  return ready;
}

/**
 * 主题里某张图的可加载地址；没有图或还没初始化就返回 null。
 * 同步返回：调用方是 computed，不能等 Promise。
 */
export function themeAssetUrl(
  themeId: string,
  filename: string | null | undefined,
): string | null {
  const root = base.value;
  if (!filename || !root) return null;
  const key = `${themeId}/${filename}`;
  const hit = cache.get(key);
  if (hit) return hit;
  // 这里用正斜杠拼：Rust 的 PathBuf 在 Windows 上给的是反斜杠，
  // 但 asset:// 走 URL 语义，统一成正斜杠更稳。
  const url = convertFileSrc(`${root.replace(/\\/g, "/")}/${key}`);
  cache.set(key, url);
  return url;
}

/** 删除主题后要把缓存清掉，否则同一个主题 id 复用时可能指向旧文件。 */
export function forgetThemeAssets(themeId: string): void {
  for (const key of [...cache.keys()]) {
    if (key.startsWith(`${themeId}/`)) cache.delete(key);
  }
}
