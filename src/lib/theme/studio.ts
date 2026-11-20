/**
 * 全屏配置的会话状态（ADR-0021 / ADR-0022）。
 *
 * 编辑草稿由设置窗口序列化后交给 Rust，预览窗口启动时再取回（见 ThemePreviewView）。
 * 两个 WebView 不共享 JS 上下文，所以这里只服务当前窗口。
 */
import { ref } from "vue";

import type { Settings } from "../../types/settings";

export type StudioTab = "theme" | "clock" | "sound";

export interface StudioPayload {
  /** 打开窗口时的整份设置快照。 */
  settings: Settings;
  /** 初始页签：主题卡「编辑」进 theme，「全屏调整」进 clock。 */
  tab: StudioTab;
  /** 进入后直接展开外观编辑的自定义主题 id；null 表示停在主题墙。 */
  editingThemeId: string | null;
  /** 本次会话新建的主题 id：放弃时连带删除它们的图片目录。 */
  newThemeIds: string[];
}

export const studioDraft = ref<Settings | null>(null);
/** 打开时的 JSON 快照，用于判断「有没有未保存改动」与收尾清理。 */
export const studioInitialJson = ref("");
export const studioTab = ref<StudioTab>("theme");
export const studioEditingThemeId = ref<string | null>(null);
export const studioNewThemeIds = ref<string[]>([]);

export function loadStudio(payload: StudioPayload): void {
  studioDraft.value = payload.settings;
  studioInitialJson.value = JSON.stringify(payload.settings);
  studioTab.value = payload.tab;
  studioEditingThemeId.value = payload.editingThemeId;
  studioNewThemeIds.value = payload.newThemeIds;
}

export function resetStudio(): void {
  studioDraft.value = null;
  studioInitialJson.value = "";
  studioTab.value = "theme";
  studioEditingThemeId.value = null;
  studioNewThemeIds.value = [];
}

/** 草稿与打开时是否不同。JSON 序列化在这里足够：配置全是可 JSON 化的字段。 */
export function studioIsDirty(): boolean {
  if (!studioDraft.value) return false;
  return JSON.stringify(studioDraft.value) !== studioInitialJson.value;
}
