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
                :scale="profile.clockScale"
              />
            </div>
          </div>
        </div>

        <section class="quick-card">
          <div class="quick-head">{{ t("preview.quickSettings") }}</div>
          <div class="quick-inline">
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
  /* 内边距写成变量：拖拽层要按它取负值铺到窗口边缘，两边必须同源。 */
  --pad-top: 12px;
  --pad-x: 22px;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: var(--pad-top) var(--pad-x) 16px;
  background: var(--ui-surface-sunken);
  border-right: 1px solid var(--ui-line);
}

/* 拖拽层铺满整条但垫在内容下面：把 data-tauri-drag-region 直接放在
   header 上会连按钮的点击一起吞掉（按钮与品牌块都有 z-index: 1，画在它上面）。

   无边框窗口没有系统标题栏，**窗口顶边那一圈必须也能拖**。原先只写 inset: 0，
   而两块面板各有 20px 上内边距，于是最上面 20px 是一条死区——鼠标停在窗口边缘
   拖不动，看起来像「只有栏里面能拖」。负的 inset 正好吃掉这段内边距。 */
.drag-layer {
  position: absolute;
  inset: 0;
}

/* 只有这两条 header 需要吃掉面板内边距；.preview-heading 里那条留在原地。 */
.brand-row > .drag-layer,
.settings-top > .drag-layer {
  inset: calc(-1 * var(--pad-top)) calc(-1 * var(--pad-x)) 0;
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
  background: var(--ui-brand-tile);
  color: var(--ui-brand-ink);
  /* 深色外壳里靠一圈亮边定形，浅色档是 transparent */
  box-shadow: inset 0 0 0 1px var(--ui-tile-ring);
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
     两者之间裂出一道空档。竖屏主屏下舞台会高于栏高，safe center 改为从顶部排，
     整组可在栏内滚动，不会把快捷设置裁掉。 */
  justify-content: center;
  justify-content: safe center;
  overflow-y: auto;
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
  box-shadow: var(--ui-well-inset);
}

/* 钟面是一条居中的「屏幕」带，不是整栏铺满：默认主题本来就深，
   铺满整栏会变成一大块黑，把右边的设置压得看不见。
   宽高比由 .stage 统一跟随主屏（--screen-aspect）；圆角按「内屏」档 4px。 */
.clock-stage {
  flex: none;
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

/* 时间制式与三个勾选块同处一行：勾选靠左、制式靠右。
   左栏最窄 336px，放不下时整行换行（宁可换行，也不要挤成两截）。
   间距比原来紧一档：省下的横向空间刚好让两者在最窄栏里并排。 */
.quick-inline {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex-wrap: wrap;
}

.quick-segment {
  display: inline-flex;
  gap: 1px;
  padding: 2px;
  background: var(--ui-track);
  border-radius: var(--radius-control);
}

.quick-segment button {
  padding: 5px 8px;
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
  gap: 2px;
}

/* 勾选块而不是开关：三项都是「加一块内容」，勾选框比一排绿色开关更轻，
   一屏三个开关会把注意力从钟面抢走。 */
.quick-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 7px 4px 5px;
  border-radius: 5px;
  font-size: 11px;
  color: var(--ui-text-secondary);
  transition: background 0.12s ease, color 0.12s ease;
}

.quick-toggle:hover {
  background: var(--ui-hover);
}

.quick-toggle::before {
  content: "";
  width: 14px;
  height: 14px;
  border: 1px solid var(--ui-check-line);
  border-radius: 2px;
  background: var(--ui-check-bg);
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
  color: var(--ui-accent-ink);
  font-size: 10px;
  font-weight: 700;
}

.settings-panel {
  --pad-top: 12px;
  --pad-x: 26px;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: var(--pad-top) var(--pad-x) 10px;
  background: var(--ui-surface);
}

.settings-top {
  position: relative;
  /* 这一行压在拖拽层之上：右上角的最小化/关闭必须收得到点击。
     （窗口已固定尺寸，不再有边缘缩放热区。） */
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

/* 动作按钮与交通灯同层：必须盖在拖拽层上才收得到点击。
   左侧品牌块留在 --pad-x（22px）与内容列对齐，右侧这几个贴边（15px）——
   不对称是刻意的：左边是产品名，右边是窗口控件。
   右边距取负值把它贴到窗口边缘（26 - 12 = 14px）：这几个是窗口控件，
   该像窗口控件那样贴边，而不是跟下面的内容列对齐——否则右侧会空出一大块，
   和 12px 的上边距摆在一起显得整条栏偏左。 */
.top-actions {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  /* 热区连成一条，中间不留点不到的死缝；视觉间隔由图标/圆点自身的留白给。
     负边距把窗口控件拉到距窗口 15px：26（面板内边距）− 12 = 14，再加壳体 1px 描边。
     写成 calc 是为了跟着 --pad-x 走，改内边距时不会重新分叉。 */
  gap: 0;
  margin-right: calc(-1 * (var(--pad-x) - 14px));
}

/* 点击面积按「窗口控件」给：46×34，和 Edge 的窗口按钮同一量级——
   比 26px 的标题栏还高出 4px 上下，宽度也远超图标本身。
   视觉大小与点击大小分开：14px 的图标/圆点好认，46px 的长条好点。
   宁可让按钮溢出标题栏，也不为了塞进去把热区压回 26px。 */
.icon-action {
  display: grid;
  place-items: center;
  width: 46px;
  height: 34px;
  padding: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--ui-icon);
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
  /* 钮与钮之间不留缝：视觉间隔由圆点两侧的透明留白给，热区连成一片。 */
  gap: 0;
  margin-left: 0;
}

/* 交通灯：46×34 的透明热区 + 居中的 14px 圆点。
   圆点用伪元素画，颜色由 --light 给——按钮本身必须透明，
   否则点一下就是整块色块。 */
.light {
  display: grid;
  place-items: center;
  width: 46px;
  height: 34px;
  padding: 0;
  border-radius: 5px;
  background: transparent;
  transition: background 0.12s ease;
}

.light::before {
  content: "";
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--light);
  box-shadow: inset 0 0 0 0.5px rgba(0, 0, 0, 0.14);
  transition: filter 0.12s ease;
}

.light:hover {
  background: var(--ui-surface-2);
}

.light:hover::before {
  filter: brightness(0.9);
}

.light:active::before {
  filter: brightness(0.78);
}

.light-close {
  --light: var(--ui-traffic-close);
}

.light-min {
  --light: var(--ui-traffic-min);
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
  color: var(--ui-text-faint);
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
  background: var(--ui-toast-bg);
  color: var(--ui-toast-ink);
  box-shadow: var(--ui-shadow-toast);
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
