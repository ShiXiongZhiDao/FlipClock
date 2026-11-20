/**
 * 打开全屏配置前的快照构造。
 *
 * 新建 / 复制 / 编辑三个入口都要做同一件事：深拷贝当前设置、把新主题塞进
 * 草稿、按 ADR-0022 继承或复制一份钟面配置。放在这里，避免三个入口各写一遍。
 */
import { invoke } from "@tauri-apps/api/core";

import type { Settings, ThemeSpec } from "../../types/settings";
import { cloneJson } from "../clone";
import { cloneProfile, ensureProfiles, profileFor } from "./profile";
import type { StudioPayload, StudioTab } from "./studio";

export interface OpenStudioOptions {
  settings: Settings;
  tab: StudioTab;
  /** 新建或复制出的主题：会加入草稿，但不会写回配置。 */
  theme?: ThemeSpec;
  /** 新主题的配置从哪个主题继承 / 复制；默认当前主题。 */
  profileFrom?: string;
  /** 打开后直接展开外观编辑的主题 id。 */
  editingThemeId?: string | null;
  /** 本次会话新建的主题 id：放弃时删目录。 */
  newThemeIds?: string[];
}

export function buildStudioPayload(options: OpenStudioOptions): StudioPayload {
  const snapshot = cloneJson(options.settings);
  ensureProfiles(snapshot);

  if (options.theme) {
    const from = options.profileFrom ?? snapshot.appearance.theme;
    snapshot.appearance.customThemes.push(options.theme);
    snapshot.appearance.profiles[options.theme.id] = cloneProfile(profileFor(snapshot, from));
    snapshot.appearance.theme = options.theme.id;
  }

  // 从主题卡「编辑」进来时，编辑对象必须同时成为当前主题，
  // 否则画布显示的是原主题、右侧改的是另一个主题。
  if (options.editingThemeId) {
    snapshot.appearance.theme = options.editingThemeId;
  }

  return {
    settings: snapshot,
    tab: options.tab,
    editingThemeId: options.editingThemeId ?? null,
    newThemeIds: options.newThemeIds ?? [],
  };
}

export async function openStudio(payload: StudioPayload): Promise<void> {
  await invoke("open_theme_preview", { payload: JSON.stringify(payload) });
}
