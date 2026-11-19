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
  // 图标按钮没有文字说明，误点代价高：确认一次再清全部设置。
  if (!window.confirm("恢复默认设置？当前的外观、时间与声音设置会被清除。")) return;
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
        <div class="product-name">FlipClock</div>
        <div class="titlebar-actions">
          <button
            class="icon-action"
            type="button"
            title="恢复默认设置"
            aria-label="恢复默认设置"
            @click="restoreDefaults"
          >
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M13 8a5 5 0 1 1-1.6-3.7" />
              <path d="M13 2.5V5h-2.5" />
            </svg>
          </button>
          <button
            class="icon-action"
            type="button"
            title="预览屏保"
            aria-label="预览屏保"
            @click="preview"
          >
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M2.5 5.5v-3h3M10.5 2.5h3v3M13.5 10.5v3h-3M5.5 13.5h-3v-3" />
            </svg>
          </button>
        </div>
        <div class="traffic">
          <button class="light light-min" type="button" title="最小化" aria-label="最小化" @click="minimize"></button>
          <button class="light light-close" type="button" title="关闭" aria-label="关闭" @click="closeWindow"></button>
        </div>
      </header>

      <aside class="sidebar">
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
        <div class="page">
          <Transition name="page">
            <ClockPage v-if="page === 'clock'" key="clock" />
            <AppearancePage v-else-if="page === 'appearance'" key="appearance" />
            <TimePage v-else-if="page === 'time'" key="time" />
            <SoundPage v-else-if="page === 'sound'" key="sound" />
            <SystemPage v-else-if="page === 'system'" key="system" />
            <ScreenSaverPage v-else key="screensaver" />
          </Transition>
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

/* 自绘标题栏：左侧产品名称，两颗灯靠右。整条仍是拖拽区，文字垫在
   拖拽层上面但不拦鼠标，按钮靠 z-index 盖住拖拽层接点击。
   拖拽只走 data-tauri-drag-region（原生 WM_NCLBUTTONDOWN），不要再加
   -webkit-app-region: drag：Chromium 那套拖动循环会和它抢鼠标，实测窗口
   拖动全程不动、松开鼠标才跳到终点。 */
.titlebar {
  position: relative;
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 2px;
}

/* 拖拽层铺满标题栏但垫在文字和交通灯下面；
   把 data-tauri-drag-region 放在 header 上会连按钮的点击一起吞掉。 */
.titlebar-drag {
  position: absolute;
  inset: 0;
}

.product-name {
  position: relative;
  z-index: 1;
  font-size: 14px;
  font-weight: 650;
  letter-spacing: 0.04em;
  color: var(--ui-text-muted);
}

/* 操作按钮与交通灯同层：必须盖在拖拽层上才收得到点击。
   margin-left:auto 把它推到右侧，与最外面的两颗灯之间留一段呼吸。 */
.titlebar-actions {
  position: relative;
  z-index: 1;
  display: flex;
  gap: 7px;
  align-items: center;
  margin-left: auto;
  margin-right: 14px;
}

.icon-action {
  display: grid;
  place-items: center;
  width: 26px;
  height: 23px;
  padding: 0;
  border: 1px solid var(--ui-accent-line);
  border-radius: 7px;
  background: var(--ui-surface-2);
  color: var(--ui-text);
  transition: border-color 0.16s ease, background 0.16s ease;
}

.icon-action:hover:not(:disabled) {
  border-color: var(--ui-accent-line-strong);
  background: var(--ui-surface);
}

.icon-action svg {
  display: block;
  width: 15px;
  height: 15px;
}

.traffic {
  position: relative;
  z-index: 1;
  display: flex;
  gap: 8px;
  align-items: center;
}

.light {
  width: 15px;
  height: 15px;
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
  /* 侧栏没有品牌块；顶部留白按原来 brand 到导航的间距留。 */
  padding: 34px 12px 20px;
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

.page {
  flex: 1;
  min-height: 0;
  display: flex;
  position: relative;
}

/* 交叉淡入淡出：新旧两页同时在场，绝对定位叠在一起才不会把容器撑高。
   只动透明度——页面里卡片多，位移过渡会晃眼。 */
.page-enter-active,
.page-leave-active {
  position: absolute;
  inset: 0;
  transition: opacity 0.18s ease;
}

.page-enter-active {
  z-index: 1;
}

.page-enter-from,
.page-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .page-enter-active,
  .page-leave-active {
    transition: none;
  }
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
