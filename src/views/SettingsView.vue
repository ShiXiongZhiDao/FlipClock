<script setup lang="ts">
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";

import FlipClock from "../components/clock/FlipClock.vue";
import AppearancePage from "./pages/AppearancePage.vue";
import ScreenSaverPage from "./pages/ScreenSaverPage.vue";
import SoundPage from "./pages/SoundPage.vue";
import SystemPage from "./pages/SystemPage.vue";
import TimePage from "./pages/TimePage.vue";
import { useClock } from "../composables/useClock";
import { useClockScale } from "../composables/useClockScale";
import { useSettings } from "../composables/useSettings";
import { useStudioLauncher } from "../composables/useStudioLauncher";
import { useTheme } from "../composables/useTheme";
import { useToast } from "../composables/useToast";
import { trackAppStarted, trackSettingsPageViewed } from "../lib/analytics";
import { isTauri } from "../lib/env";
import { errorText, t, type MessageKey } from "../lib/i18n";
import { themeLabel } from "../lib/theme/labels";
import { useUpdater } from "../lib/updater";

const { settings, loaded, error, flush, load } = useSettings();
const { launch } = useStudioLauncher();
const { now } = useClock();
const { current: currentTheme, profile, backgroundUrl, digitUrl } = useTheme();
/** 左栏预览按容器自适应，「时钟缩放」只做阻尼演示，见 useClockScale。 */
const { panelScale } = useClockScale();

/** 版本号只在页脚出现；启动时取一次就够。 */
const versionText = ref("");
const { scheduleAutoCheck } = useUpdater();

type PageId = "appearance" | "time" | "sound" | "screensaver" | "system";

interface PageDef {
  id: PageId;
  /** 页签上的短名：五个要横排在一行里，长了会挤。 */
  label: MessageKey;
  /** 页面标题：给全称，屏保页的简称「屏保」在这里要还原成「屏幕保护程序」。 */
  title: MessageKey;
}

/**
 * 五个页签。顺序就是用户调钟面的顺序：先看外观，再调走时，最后才是
 * 声音与系统集成。预览已经从页签里拿掉——它常驻左栏，每一页都能看见。
 */
const PAGES: PageDef[] = [
  {
    id: "appearance",
    label: "nav.appearance",
    title: "page.appearance",
  },
  {
    id: "time",
    label: "nav.time",
    title: "page.time",
  },
  {
    id: "sound",
    label: "nav.sound",
    title: "page.sound",
  },
  {
    id: "screensaver",
    label: "nav.screensaver",
    title: "page.screensaver",
  },
  {
    id: "system",
    label: "nav.system",
    title: "page.system",
  },
];

/** 默认落在外观：换主题是绝大多数人打开设置要做的事。 */
const page = ref<PageId>("appearance");

/** 滚动容器。切页签要回到顶部——上一个页面滚到一半的位置，换过来就是空白。 */
const pageScroll = ref<HTMLElement | null>(null);
watch(page, () => {
  if (pageScroll.value) pageScroll.value.scrollTop = 0;
});

/** 页标题跟着页签走；页签是简称、标题是全称，两套键分开存。 */
const currentPage = computed(() => PAGES.find((item) => item.id === page.value) ?? PAGES[0]);

const appWindow = isTauri() ? getCurrentWindow() : null;
let unlistenClose: (() => void) | undefined;

function minimize() {
  void appWindow?.minimize();
}

/** 真正的保存与关闭在 onCloseRequested 里做，按钮只负责发起关闭。 */
function closeWindow() {
  void appWindow?.close();
}

const { toast, notify } = useToast(2800);

/** `ResizeDirection` 是 Tauri 的内部类型、没有导出，这里按它的字面量重复一遍。 */
type ResizeSide =
  | "East"
  | "North"
  | "NorthEast"
  | "NorthWest"
  | "South"
  | "SouthEast"
  | "SouthWest"
  | "West";

/** 八个方向的把手。角上的热区要略大，否则很难命中。 */
const RESIZE_EDGES: Array<{ name: string; side: ResizeSide }> = [
  { name: "n", side: "North" },
  { name: "s", side: "South" },
  { name: "e", side: "East" },
  { name: "w", side: "West" },
  { name: "ne", side: "NorthEast" },
  { name: "nw", side: "NorthWest" },
  { name: "se", side: "SouthEast" },
  { name: "sw", side: "SouthWest" },
];

function startResize(side: ResizeSide) {
  void getCurrentWindow().startResizeDragging(side).catch((error: unknown) => {
    console.warn("start resize dragging failed", error);
  });
}

// 保存成功不再占用页面文字；失败仍然要让人看见。
watch(error, (value) => {
  if (value) notify(t("settings.saveFailed", { error: errorText(value) }));
});

async function preview() {
  try {
    await invoke("preview_screensaver");
  } catch (e) {
    notify(errorText(e));
  }
}

/** 打开全屏配置（ADR-0021）。先落盘由 launch 统一负责。 */
async function openFullscreenSetup() {
  try {
    await launch("clock");
  } catch (e) {
    notify(errorText(e));
  }
}

onMounted(async () => {
  await load();

  // 统计在配置就绪后发：快照要带用户当前的配置。去重由 analytics.ts 管
  // （localStorage + 时间窗），因为多个 WebView 各有自己的模块状态。
  trackAppStarted(settings);
  trackSettingsPageViewed();

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
            notify(t("settings.saveFailed", { error: errorText(error.value) }));
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
  unlistenClose?.();
});
</script>

<template>
  <div class="shell">
    <!-- 自绘标题栏去掉了原生边框，Windows 就不再提供边缘缩放热区。
         没有这八个把手，resizable(true) 等于摆设——窗口看起来「被锁住了」。 -->
    <div class="resize-frame" aria-hidden="true">
      <div v-for="dir in RESIZE_EDGES" :key="dir.name" class="rh" :class="dir.name" @pointerdown="startResize(dir.side)" />
    </div>

    <div class="app">
      <aside class="preview-panel">
        <header class="brand-row">
          <div class="drag-layer" data-tauri-drag-region></div>
          <div class="brand">
            <span class="brand-icon">F</span>
            <span class="brand-name">FlipClock</span>
          </div>
        </header>

        <div class="preview-center">
          <div class="preview-heading">
            <div class="drag-layer" data-tauri-drag-region></div>
            <span class="preview-title">{{ t("preview.title") }}</span>
            <span class="theme-name">{{ themeLabel(currentTheme) }}</span>
          </div>
          <div
            class="stage-well"
            :title="t('studio.dblclickHint')"
            @dblclick="openFullscreenSetup"
          >
            <div class="stage clock-stage">
              <FlipClock
                :now="now"
                :format="profile.format"
                :show-date="profile.showDate"
                :show-seconds="profile.showSeconds"
                :theme="currentTheme"
                :background-url="backgroundUrl"
                :digit-url="digitUrl"
                :layout="profile.digitLayout"
                :show-colon="profile.showColon"
                :font="profile.font"
                :brightness="profile.brightness"
                variant="panel"
                :preview-scale="panelScale"
              />
            </div>
          </div>
        </div>

        <section class="quick-card">
          <div class="quick-head">{{ t("preview.quickSettings") }}</div>
          <div class="quick-row">
            <span class="quick-label">{{ t("time.format") }}</span>
            <div class="quick-segment">
              <button
                type="button"
                :class="{ selected: profile.format === '24h' }"
                @click="profile.format = '24h'"
              >
                {{ t("time.format24") }}
              </button>
              <button
                type="button"
                :class="{ selected: profile.format === '12h' }"
                @click="profile.format = '12h'"
              >
                {{ t("time.format12") }}
              </button>
            </div>
          </div>
          <div class="quick-toggles">
            <button
              type="button"
              class="quick-toggle"
              :class="{ active: profile.showDate }"
              @click="profile.showDate = !profile.showDate"
            >
              {{ t("profile.date") }}
            </button>
            <button
              type="button"
              class="quick-toggle"
              :class="{ active: profile.showSeconds }"
              @click="profile.showSeconds = !profile.showSeconds"
            >
              {{ t("profile.seconds") }}
            </button>
            <button
              type="button"
              class="quick-toggle"
              :class="{ active: profile.showColon }"
              @click="profile.showColon = !profile.showColon"
            >
              {{ t("profile.colon") }}
            </button>
          </div>
        </section>
      </aside>

      <main class="settings-panel">
        <header class="settings-top">
          <div class="drag-layer" data-tauri-drag-region></div>
          <span class="settings-label">{{ t("settings.title") }}</span>
          <div class="top-actions">
            <button
              class="icon-action"
              type="button"
              :title="t('studio.open')"
              :aria-label="t('studio.open')"
              @click="openFullscreenSetup"
            >
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M3 5h10M3 11h10" />
                <circle cx="6" cy="5" r="1.6" />
                <circle cx="10" cy="11" r="1.6" />
              </svg>
            </button>
            <button
              class="icon-action"
              type="button"
              :title="t('titlebar.previewSaver')"
              :aria-label="t('titlebar.previewSaver')"
              @click="preview"
            >
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M2.5 5.5v-3h3M10.5 2.5h3v3M13.5 10.5v3h-3M5.5 13.5h-3v-3" />
              </svg>
            </button>

            <div class="traffic">
              <button
                class="light light-min"
                type="button"
                :title="t('titlebar.minimize')"
                :aria-label="t('titlebar.minimize')"
                @click="minimize"
              ></button>
              <button
                class="light light-close"
                type="button"
                :title="t('titlebar.close')"
                :aria-label="t('titlebar.close')"
                @click="closeWindow"
              ></button>
            </div>
          </div>
        </header>

        <nav class="tabs">
          <button
            v-for="item in PAGES"
            :key="item.id"
            type="button"
            class="tab"
            :class="{ active: page === item.id }"
            @click="page = item.id"
          >
            {{ t(item.label) }}
          </button>
        </nav>

        <div ref="pageScroll" class="page-scroll">
          <div class="page-heading">
            <h1>{{ t(currentPage.title) }}</h1>
          </div>

          <AppearancePage v-if="page === 'appearance'" />
          <TimePage v-else-if="page === 'time'" />
          <SoundPage v-else-if="page === 'sound'" />
          <ScreenSaverPage v-else-if="page === 'screensaver'" />
          <SystemPage v-else />
        </div>

        <footer class="bottom-meta">
          <span>SHIXIONGZHIDAO</span>
          <span>{{ versionText || "—" }}</span>
        </footer>
      </main>
    </div>

    <Transition name="toast">
      <div v-if="toast" class="toast">{{ toast }}</div>
    </Transition>
  </div>
</template>

<style scoped>
.resize-frame {
  position: fixed;
  inset: 0;
  z-index: 400;
  pointer-events: none;
}

.rh {
  position: absolute;
  pointer-events: auto;
}

.rh.n,
.rh.s {
  left: 10px;
  right: 10px;
  height: 5px;
  cursor: ns-resize;
}
.rh.n { top: 0; }
.rh.s { bottom: 0; }

.rh.e,
.rh.w {
  top: 10px;
  bottom: 10px;
  width: 5px;
  cursor: ew-resize;
}
.rh.e { right: 0; }
.rh.w { left: 0; }

.rh.ne,
.rh.nw,
.rh.se,
.rh.sw {
  width: 12px;
  height: 12px;
}
.rh.ne { top: 0; right: 0; cursor: nesw-resize; }
.rh.nw { top: 0; left: 0; cursor: nwse-resize; }
.rh.se { bottom: 0; right: 0; cursor: nwse-resize; }
.rh.sw { bottom: 0; left: 0; cursor: nesw-resize; }

.shell {
  position: relative;
  height: 100vh;
  /* 壳体圆角由 Rust 侧的窗口区域裁（SHELL_RADIUS，13px）；这里的圆角让
     壳体背景与那条裁剪弧对齐，否则区域边缘会露出一圈 body 底色。 */
  border-radius: var(--radius-shell);
  overflow: hidden;
  background: var(--ui-surface);
  border: 1px solid var(--ui-line);
}

/* 两栏：左预览按比例、右设置吃满剩余。预览常驻，不做成可折叠面板——
   收起来之后，所有「这个值会怎么变」都只能靠猜。 */
.app {
  height: 100%;
  display: grid;
  /* 按比例分而不是按百分比：窗口收窄时左栏跟着收，别把右侧设置挤得只剩半屏。
     336px 是下限——再窄，钟面的四位数字就要互相贴住了。 */
  grid-template-columns: minmax(336px, 0.82fr) minmax(0, 1.18fr);
}

.preview-panel {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 20px 22px 16px;
  background: var(--ui-surface-sunken);
  border-right: 1px solid var(--ui-line);
}

/* 拖拽层铺满整条但垫在内容下面：把 data-tauri-drag-region 直接放在
   header 上会连按钮的点击一起吞掉。 */
.drag-layer {
  position: absolute;
  inset: 0;
}

.brand-row {
  position: relative;
  flex: none;
  display: flex;
  align-items: center;
  height: 26px;
}

.brand {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 9px;
  /* 品牌块本身没有交互，让鼠标事件落到拖拽层上 */
  pointer-events: none;
}

.brand-icon {
  width: 28px;
  height: 28px;
  border-radius: 5px;
  display: grid;
  place-items: center;
  background: #292a2d;
  color: #fff;
  font-size: 14px;
  font-weight: 650;
}

.brand-name {
  font-size: 13px;
  font-weight: 620;
  letter-spacing: -0.1px;
  color: var(--ui-text);
}

.preview-center {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  /* 标题与钟面作为一组居中：只在钟面上加 margin:auto 会把标题顶到栏口，
     两者之间裂出一道空档。 */
  justify-content: center;
  padding: 12px 0 14px;
}

.preview-heading {
  position: relative;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 9px;
  padding: 0 1px;
  font-size: 11px;
  color: var(--ui-text-muted);
}

.preview-title {
  position: relative;
  z-index: 1;
  pointer-events: none;
}

.theme-name {
  position: relative;
  z-index: 1;
  pointer-events: none;
  max-width: 50%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 凹槽：钟面像一块嵌进面板的屏，而不是贴在墙上的海报。
   5px 的暖灰边框 + 内投影，成本极低，却是整个界面里最「实物」的一处。 */
.stage-well {
  flex: none;
  padding: 5px;
  border-radius: 8px;
  background: var(--ui-well);
  box-shadow: inset 0 1px 2px rgba(60, 55, 45, 0.09);
}

/* 钟面是一条居中的「屏幕」带，不是整栏铺满：默认主题本来就深，
   铺满整栏会变成一大块黑，把右边的设置压得看不见。
   高度随窗口长一点，但封顶 280px，再高就只是浪费灰色留白。 */
.clock-stage {
  flex: none;
  height: clamp(176px, 40vh, 280px);
  border: 0;
  border-radius: 4px;
}

.quick-card {
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 11px;
  padding-top: 14px;
  border-top: 1px solid var(--ui-divider-strong);
}

.quick-head {
  font-size: 12px;
  font-weight: 620;
  color: var(--ui-text);
}

.quick-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.quick-label {
  font-size: 11px;
  color: var(--ui-text-muted);
}

.quick-segment {
  display: inline-flex;
  gap: 1px;
  padding: 2px;
  background: var(--ui-track);
  border-radius: var(--radius-control);
}

.quick-segment button {
  padding: 5px 10px;
  border-radius: 3px;
  font-size: 10.5px;
  color: var(--ui-text-secondary);
  transition: background 0.12s ease, color 0.12s ease;
}

.quick-segment button.selected {
  background: var(--ui-thumb);
  color: var(--ui-text);
  font-weight: 600;
  box-shadow: var(--ui-thumb-shadow);
}

.quick-toggles {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

/* 勾选块而不是开关：三项都是「加一块内容」，勾选框比一排绿色开关更轻，
   一屏三个开关会把注意力从钟面抢走。 */
.quick-toggle {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 4px 8px 4px 5px;
  border-radius: 5px;
  font-size: 11px;
  color: var(--ui-text-secondary);
  transition: background 0.12s ease, color 0.12s ease;
}

.quick-toggle:hover {
  background: #ebebee;
}

.quick-toggle::before {
  content: "";
  width: 14px;
  height: 14px;
  border: 1px solid #b6b3ac;
  border-radius: 2px;
  background: #fff;
}

.quick-toggle.active {
  color: var(--ui-text);
}

.quick-toggle.active::before {
  content: "✓";
  display: grid;
  place-items: center;
  background: var(--ui-accent);
  border-color: var(--ui-accent);
  color: #fff;
  font-size: 10px;
  font-weight: 700;
}

.settings-panel {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 20px 26px 10px;
  background: var(--ui-surface);
}

.settings-top {
  position: relative;
  /* 缩放热区盖在整窗边缘之上（z-index 400），这一行必须更高一层，
     否则右上角的最小化/关闭会被边角热区抢走点击。 */
  z-index: 500;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 26px;
  margin-bottom: 12px;
}

.settings-label {
  position: relative;
  z-index: 1;
  font-size: 13px;
  font-weight: 620;
  color: var(--ui-text);
  pointer-events: none;
}

/* 动作按钮与交通灯同层：必须盖在拖拽层上才收得到点击。 */
.top-actions {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 4px;
}

.icon-action {
  display: grid;
  place-items: center;
  width: 24px;
  height: 22px;
  padding: 0;
  border-radius: 4px;
  background: transparent;
  color: #6f6c65;
  transition: background 0.14s ease, color 0.14s ease;
}

.icon-action:hover {
  background: var(--ui-surface-2);
  color: var(--ui-text);
}

.icon-action svg {
  display: block;
  width: 14px;
  height: 14px;
}

.traffic {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: 10px;
}

.light {
  width: 14px;
  height: 14px;
  padding: 0;
  border-radius: 50%;
  box-shadow: inset 0 0 0 0.5px rgba(0, 0, 0, 0.14);
  transition: filter 0.14s ease;
}

.light-close {
  background: #ff5f57;
}

.light-min {
  background: #febc2e;
}

.light:hover {
  filter: brightness(0.9);
}

.light:active {
  filter: brightness(0.78);
}

/* 横向页签：只有文字 + 一条墨色细线，没有图标也没有底色块。
   图标排成一行是「仪表盘」的做法；五个词足够说清这是哪五页。 */
.tabs {
  flex: none;
  display: flex;
  align-items: stretch;
  gap: 22px;
  margin-bottom: 15px;
  border-bottom: 1px solid var(--ui-divider);
}

.tab {
  position: relative;
  display: flex;
  align-items: center;
  height: 34px;
  padding: 0 1px 7px;
  font-size: 11.5px;
  letter-spacing: 0.01em;
  color: var(--ui-text-muted);
  white-space: nowrap;
  transition: color 0.12s ease;
}

.tab:hover {
  color: var(--ui-text);
}

.tab.active {
  color: var(--ui-text);
  font-weight: 620;
}

.tab.active::after {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  bottom: -1px;
  height: 1.5px;
  background: var(--ui-accent);
}

.page-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 4px 4px 0;
  scrollbar-width: thin;
  scrollbar-color: var(--ui-scroll) transparent;
}

.page-scroll::-webkit-scrollbar {
  width: 6px;
}

.page-scroll::-webkit-scrollbar-thumb {
  background: var(--ui-scroll);
  border-radius: 6px;
}

.page-scroll::-webkit-scrollbar-track {
  background: transparent;
}

/* 页标题只比段落名大一档半：这是桌面工具的设置页，不是落地页。
   22px 的大标题会把它变成「产品介绍」。 */
.page-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin: 0 1px 14px;
}

.page-heading h1 {
  font-size: 18px;
  font-weight: 620;
  letter-spacing: -0.2px;
  color: var(--ui-text);
}

.bottom-meta {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 8px;
  padding: 9px 2px 0;
  border-top: 1px solid var(--ui-divider);
  font-size: 9px;
  letter-spacing: 0.2px;
  color: #9a978f;
}

.toast {
  position: fixed;
  left: 50%;
  bottom: 30px;
  transform: translateX(-50%);
  z-index: 40;
  padding: 8px 13px;
  font-size: 11px;
  border-radius: 4px;
  background: #1b1b1a;
  color: #fff;
  box-shadow: 0 3px 12px rgba(0, 0, 0, 0.2);
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.16s ease, transform 0.16s ease;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translate(-50%, 6px);
}
</style>
