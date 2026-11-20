<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { onMounted, ref } from "vue";

import FullscreenSetupOverlay from "../components/theme/FullscreenSetupOverlay.vue";
import { initThemeAssets } from "../lib/theme/assets";
import { previewLanguage } from "../lib/i18n";
import { applyAppearance } from "../lib/appearance";
import { ensureProfiles } from "../lib/theme/profile";
import { loadStudio, type StudioPayload } from "../lib/theme/studio";

/**
 * 全屏配置的独立窗口（ADR-0021）。
 *
 * 窗口由 Rust 在创建时直接建成整屏（decorations(false) + 显示器尺寸 + 置顶），
 * 这里只负责把设置快照取出来交给界面。取不到就自己关窗——说明这个窗口是被
 * 直接重新打开或快照已被消费，留着一个空窗口没有意义。
 *
 * 这个窗口**不加载 useSettings 的全局状态**：草稿必须与会触发防抖落盘的那份
 * 配置隔离开，否则「放弃」之前配置已经被写回去了。
 */
const ready = ref(false);

onMounted(async () => {
  // 主题图片的根目录路径要先拿到，草稿里的背景与纹理才渲染得出来。
  void initThemeAssets();
  try {
    const raw = await invoke<string | null>("get_theme_preview");
    if (!raw) {
      await invoke("close_theme_preview");
      return;
    }
    const payload = JSON.parse(raw) as StudioPayload;
    if (!payload?.settings?.appearance) {
      await invoke("close_theme_preview");
      return;
    }
    // 本窗口的文字与日期行跟随草稿语言；托盘与持久化留到保存时。
    previewLanguage(payload.settings.ui.language);
    // 外观同理：这个窗口不加载 useSettings（草稿不能被防抖写回），
    // 所以必须自己把偏好落到 <html> 上，否则深色外壳里抽屉仍是浅色。
    applyAppearance(payload.settings.ui.appearance ?? "system");
    // 主窗口构造快照时已经补齐过；这里再兜一次，手改载荷也不会出现空配置。
    ensureProfiles(payload.settings);
    loadStudio(payload);
    ready.value = true;
  } catch (error) {
    console.warn("fullscreen setup bootstrap failed", error);
    await invoke("close_theme_preview");
  }
});
</script>

<template>
  <FullscreenSetupOverlay v-if="ready" />
</template>
