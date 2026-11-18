<script setup lang="ts">
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";

import AppearancePage from "./pages/AppearancePage.vue";
import ClockPage from "./pages/ClockPage.vue";
import ScreenSaverPage from "./pages/ScreenSaverPage.vue";
import SoundPage from "./pages/SoundPage.vue";
import SystemPage from "./pages/SystemPage.vue";
import TimePage from "./pages/TimePage.vue";
import { useSettings } from "../composables/useSettings";
import { isTauri } from "../lib/env";
import { useUpdater } from "../lib/updater";
import {
  GLASS_TRANSPARENCY_MAX,
  GLASS_TRANSPARENCY_MIN,
} from "../types/settings";

const { settings, loaded, error, flush, load, reset } = useSettings();

/** 版本号只在「关于」与侧栏出现；启动时取一次就够。 */
const versionText = ref("");
const { scheduleAutoCheck } = useUpdater();

/** 滑杆存的是“透明度”，CSS 要的是“不透明度”；范围与滑杆、后端共用同一组常量。 */
const glassAlpha = computed(
  () =>
    1 -
    Math.min(
      GLASS_TRANSPARENCY_MAX,
      Math.max(GLASS_TRANSPARENCY_MIN, settings.appearance.glass.transparency),
    ),
);

type PageId = "clock" | "appearance" | "time" | "sound" | "screensaver" | "system";

interface PageDef {
  id: PageId;
  label: string;
  icon: string;
  title: string;
}

const PAGES: PageDef[] = [
  {
    id: "clock",
    label: "时钟预览",
    icon: "◷",
    title: "时钟预览",
  },
  {
    id: "appearance",
    label: "外观主题",
    icon: "◈",
    title: "外观主题",
  },
  {
    id: "time",
    label: "时间与日期",
    icon: "◔",
    title: "时间与日期",
  },
  {
    id: "sound",
    label: "声音",
    icon: "♪",
    title: "声音",
  },
  {
    id: "screensaver",
    label: "屏幕保护程序",
    icon: "▣",
    title: "屏幕保护程序",
  },
  {
    id: "system",
    label: "系统",
    icon: "⚙",
    title: "系统",
  },
];

const page = ref<PageId>("clock");
const current = computed(() => PAGES.find((item) => item.id === page.value) ?? PAGES[0]);

const appWindow = isTauri() ? getCurrentWindow() : null;
let unlistenClose: (() => void) | undefined;

function minimize() {
  void appWindow?.minimize();
}

/** 真正的保存与关闭在 onCloseRequested 里做，按钮只负责发起关闭。 */
function closeWindow() {
  void appWindow?.close();
}

const toast = ref<string | null>(null);
let toastTimer: ReturnType<typeof setTimeout> | undefined;

function notify(text: string) {
  toast.value = text;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (toast.value = null), 2800);
}

// 保存成功不再占用页面文字；失败仍然要让人看见。
watch(error, (value) => {
  if (value) notify(`保存失败：${value}`);
});

async function preview() {
  try {
    await invoke("preview_screensaver");
  } catch (e) {
    notify(String(e));
  }
}

function restoreDefaults() {
  reset();
  notify("已恢复默认设置");
}

onMounted(async () => {
  await load();

  if (isTauri()) {
    versionText.value = await getVersion().catch(() => "");
    // 设置窗口出现 5 秒后再查更新（ADR-0013）：不跟启动抢带宽，
    // 查到也只在「系统 → 关于」里亮一行，不弹窗打断。
    scheduleAutoCheck();
  }

  // 关窗前把防抖队列里的最后一次修改落盘；保存失败就不关，别静默丢设置。
  if (appWindow) {
    const win = appWindow;
    unlistenClose = await win.onCloseRequested((event) => {
      event.preventDefault();
      void (async () => {
        // 配置没读出来就没什么可存的；失败的是读取，不该把窗口锁死。
        if (loaded.value) {
          await flush();
          if (error.value) {
            notify(`保存失败：${error.value}`);
            return;
          }
        }
        // destroy 不会再触发 CloseRequested，避免二次拦截。
        await win.destroy();
      })();
    });
  }

  // 窗口由 Rust 以 hidden 创建，这里读完配置、挂好关闭钩子再让它出现。
  await invoke("ui_ready").catch(() => {});
});
onBeforeUnmount(() => {
  clearTimeout(toastTimer);
  unlistenClose?.();
});
</script>

<template>
  <div class="shell" :style="{ '--ui-glass': glassAlpha }">
    <div class="shell-inner">
      <header class="titlebar">
        <div class="titlebar-drag" data-tauri-drag-region></div>
        <div class="traffic">
          <button
            class="light light-zoom"
            type="button"
            disabled
            :title="'窗口尺寸固定，不可缩放'"
            aria-label="不可缩放"
          ></button>
          <button class="light light-min" type="button" title="最小化" aria-label="最小化" @click="minimize"></button>
          <button class="light light-close" type="button" title="关闭" aria-label="关闭" @click="closeWindow"></button>
        </div>
      </header>

      <aside class="sidebar">
        <div class="brand">
          <div class="brand-mark">F</div>
          <div class="brand-copy">
            <div class="brand-name">FlipClock</div>
          </div>
        </div>

        <div class="nav-block">
          <nav class="nav">
            <button
              v-for="item in PAGES"
              :key="item.id"
              type="button"
              :class="{ active: page === item.id }"
              @click="page = item.id"
            >
              <span class="ico">{{ item.icon }}</span>{{ item.label }}
            </button>
          </nav>
        </div>

        <div class="sidebar-bottom">
          <div class="version">FLIPCLOCK · V{{ versionText || "—" }}</div>
        </div>
      </aside>

      <main class="content">
        <header class="topbar">
          <div class="title-block">
            <h1>{{ current.title }}</h1>
          </div>
          <div class="top-actions">
            <button class="secondary" type="button" @click="restoreDefaults">恢复默认</button>
            <button class="primary" type="button" @click="preview">⛶ 预览屏保</button>
          </div>
        </header>

        <div class="page">
          <ClockPage v-if="page === 'clock'" />
          <AppearancePage v-else-if="page === 'appearance'" />
          <TimePage v-else-if="page === 'time'" />
          <SoundPage v-else-if="page === 'sound'" />
          <SystemPage v-else-if="page === 'system'" />
          <ScreenSaverPage v-else />
        </div>
      </main>
    </div>

    <Transition name="toast">
      <div v-if="toast" class="toast">{{ toast }}</div>
    </Transition>
  </div>
</template>

<style scoped>
.shell {
  position: relative;
  height: 100vh;
  /* 窗口形状由 Rust 侧的窗口区域裁（SHELL_RADIUS，18px）；这里的圆角让
     壳体背景与那条裁剪弧对齐，否则区域边缘会露出一圈 body 底色。 */
  border-radius: var(--radius-shell);
  overflow: hidden;
  background: var(--ui-shell-bg);
}

.shell-inner {
  height: 100%;
  display: grid;
  grid-template-columns: 214px minmax(0, 1fr);
  grid-template-rows: 34px minmax(0, 1fr);
  gap: 10px 12px;
  padding: 6px 12px 12px;
}

/* 自绘标题栏：整条都是拖拽区，三个灯靠右。
   拖拽只走 data-tauri-drag-region（原生 WM_NCLBUTTONDOWN），不要再加
   -webkit-app-region: drag：Chromium 那套拖动循环会和它抢鼠标，实测窗口
   拖动全程不动、松开鼠标才跳到终点。 */
.titlebar {
  position: relative;
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding: 0 2px;
}

/* 拖拽层铺满标题栏但垫在交通灯下面；
   把 data-tauri-drag-region 放在 header 上会连按钮的点击一起吞掉。 */
.titlebar-drag {
  position: absolute;
  inset: 0;
}

.traffic {
  position: relative;
  z-index: 1;
}

.traffic {
  display: flex;
  gap: 8px;
  align-items: center;
}

.light {
  width: 12px;
  height: 12px;
  padding: 0;
  border-radius: 50%;
  box-shadow: inset 0 0 0 0.5px rgba(0, 0, 0, 0.14);
  transition: filter 0.14s ease, opacity 0.14s ease;
}

.light-close {
  background: #ff5f57;
}

.light-min {
  background: #febc2e;
}

/* 窗口尺寸固定，最大化这颗按 macOS 的规矩留白并置灰 */
.light-zoom {
  background: #28c840;
  opacity: 0.32;
  cursor: default;
}

.light:hover:not(:disabled) {
  filter: brightness(0.9);
}

.light:active:not(:disabled) {
  filter: brightness(0.78);
}

.sidebar {
  display: flex;
  flex-direction: column;
  gap: 26px;
  min-height: 0;
  padding: 20px 12px;
  border: 1px solid rgba(255, 255, 255, 0.8);
  border-radius: var(--radius-sidebar);
  background-color: var(--ui-sidebar);
  /* 纵向渐变：顶部受光最亮，到底部逐渐沉下去，
     侧栏因此像一块竖着立的玻璃板，而不是平涂的色块。
     各档亮度按 0.52 的默认不透明度折算，跟滑杆一起增减。 */
  background-image: linear-gradient(
    180deg,
    rgba(255, 255, 255, calc(var(--ui-glass) * 1.769)) 0%,
    rgba(255, 255, 255, calc(var(--ui-glass) * 0.962)) 34%,
    rgba(255, 255, 255, calc(var(--ui-glass) * 0.538)) 68%,
    rgba(255, 255, 255, calc(var(--ui-glass) * 0.808)) 100%
  );
  box-shadow: var(--ui-card-shadow);
}

.brand {
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 0 6px;
}

.brand-mark {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  border-radius: 10px;
  background: linear-gradient(145deg, var(--ui-brand-from), var(--ui-brand-to));
  color: #161512;
  font-family: var(--font-digit);
  font-size: 20px;
  box-shadow: 0 5px 16px var(--ui-accent-halo);
}

.brand-copy {
  line-height: 1.25;
}

.brand-name {
  font-size: 16px;
  font-weight: 750;
  letter-spacing: 0.01em;
}

.nav {
  display: grid;
  gap: 4px;
}

.nav button {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 10px 11px;
  border-radius: 9px;
  font-size: 12px;
  color: var(--ui-text-secondary);
  text-align: left;
  transition: background 0.16s ease, color 0.16s ease;
}

.nav button .ico {
  width: 21px;
  flex: none;
  text-align: center;
  font-size: 17px;
  line-height: 1;
  opacity: 0.9;
}

.nav button:hover {
  background: var(--ui-surface);
  color: var(--ui-text);
}

.nav button.active {
  background: var(--ui-accent-soft);
  color: var(--ui-accent);
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.9);
}

.sidebar-bottom {
  margin-top: auto;
  border-top: 1px solid var(--ui-divider);
  padding: 14px 10px 0;
  font-size: 10px;
  color: var(--ui-text-muted);
  line-height: 1.7;
}

.version {
  font-size: 9px;
  color: var(--ui-text-muted);
  opacity: 0.7;
  letter-spacing: 0.08em;
  margin-top: 4px;
}

.content {
  /* 内容区到窗口的四周距离由 shell-inner 的 12px 统一给；
     这里再补 padding 会让右侧/底部比侧栏卡片多出几像素。 */
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 18px;
  min-width: 0;
  min-height: 0;
}

.topbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

h1 {
  font-size: 22px;
  font-weight: 650;
  letter-spacing: -0.01em;
  line-height: 1.2;
}

.top-actions {
  display: flex;
  gap: 8px;
  align-items: center;
  flex: none;
  padding-top: 4px;
}

.page {
  flex: 1;
  min-height: 0;
  display: flex;
}

.toast {
  position: fixed;
  left: 50%;
  bottom: 34px;
  transform: translateX(-50%);
  padding: 9px 14px;
  font-size: 11px;
  border-radius: 8px;
  background: var(--ui-surface-2);
  border: 1px solid var(--ui-divider);
  color: var(--ui-text);
  box-shadow: 0 18px 40px rgba(0, 0, 0, 0.45);
  z-index: 40;
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translate(-50%, 8px);
}
</style>
