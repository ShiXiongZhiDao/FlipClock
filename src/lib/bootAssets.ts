/**
 * 主题图片与导入字体共用的「启动载荷根目录 → IPC 兜底」就位逻辑。
 *
 * 沉浸式窗口的启动载荷里已经带着根目录，直接落上；设置窗口等没有载荷时再问一次 IPC。
 */
import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";

import { isTauri } from "./env";

export function bootAssetBase(dir: string | null | undefined, command: string) {
  const base = ref<string | null>(dir ?? null);
  let ready: Promise<void> | null = null;

  function init(): Promise<void> {
    ready ??= (async () => {
      if (!isTauri()) return;
      // 启动载荷已经给过根目录就不必再问一次。
      if (base.value) return;
      base.value = await invoke<string>(command).catch(() => null);
    })();
    return ready;
  }

  return { base, init };
}