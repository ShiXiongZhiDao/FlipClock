/**
 * 从设置窗口 / 时钟页打开全屏配置的统一入口。
 *
 * 两个入口原先各写一份「flush → 判错 → 开窗 → notify」，任何一处漏掉 flush，
 * 预览窗口就会读到 200ms 之前的旧配置（ADR-0021 决定 16）。
 */
import { buildStudioPayload, openStudio, type OpenStudioOptions } from "../lib/theme/openStudio";
import type { StudioTab } from "../lib/theme/studio";
import { useSettings } from "./useSettings";

type StudioExtras = Omit<OpenStudioOptions, "settings" | "tab">;

export function useStudioLauncher() {
  const { settings, flush, error } = useSettings();

  /** 失败时抛出原始错误（字符串或命令错误），调用方用 errorText 统一转文案。 */
  async function launch(tab: StudioTab, extras: StudioExtras = {}): Promise<void> {
    await flush();
    if (error.value) throw error.value;
    await openStudio(buildStudioPayload({ settings, tab, ...extras }));
  }

  return { launch };
}
