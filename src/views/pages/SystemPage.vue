<script setup lang="ts">
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";

import ConfigBundleDialog from "../../components/settings/ConfigBundleDialog.vue";
import { useSettings } from "../../composables/useSettings";
import { isTauri } from "../../lib/env";
import { errorText, locale, t, type MessageKey } from "../../lib/i18n";
import { useUpdater } from "../../lib/updater";
import type {
  ConfigBundleSelection,
  ConfigBundleSummary,
  ConfigImportResult,
  ConfigImportSkip,
} from "../../types/configBundle";
import type { NoArgMode, UiAppearance } from "../../types/settings";

/**
 * 二维码与公众号横幅走公开仓库，不进安装包：换图不必重发版本，
 * 包体也不为几张码变大。镜像目录结构必须一致，失败时露出纯文字兜底。
 */
const QR_MIRRORS = [
  "https://raw.githubusercontent.com/ShiXiongZhiDao/BrandAssets/main/images/about/",
  "https://gitee.com/ShiXiongZhiDao/BrandAssets/raw/main/images/about/",
];
/**
 * 关于页的外链只传 key：白名单在后端，前端拼不出任意 URL。
 * 浏览器里预览版式时没有 Tauri 命令，退回 window.open 方便调试。
 */
const ABOUT_LINK_FALLBACK: Record<string, string> = {
  website: "https://shixiongzhidao.cn",
  issues: "https://github.com/ShiXiongZhiDao/FlipClock/issues",
  projects: "https://github.com/ShiXiongZhiDao",
  screensaver: "https://github.com/ShiXiongZhiDao/FlipClock",
};

async function openAboutLink(key: string) {
  if (!isTauri()) {
    window.open(ABOUT_LINK_FALLBACK[key], "_blank", "noopener,noreferrer");
    return;
  }
  try {
    await invoke("open_about_link", { key });
  } catch (error) {
    notify("err", errorText(error));
  }
}

/**
 * 顺序回退：GitHub 不通换 Gitee，都试过就藏图露字。
 * 不做 Promise.any 竞速——那套状态机在这里没有测试框架兜底，简单更可信。
 */
function loadQr(img: HTMLImageElement) {
  const attempt = Number(img.dataset.try ?? "0");
  if (attempt >= QR_MIRRORS.length) {
    img.hidden = true;
    const slot = img.closest(".qr-slot");
    if (slot) slot.classList.add("failed");
    return;
  }
  img.dataset.try = String(attempt + 1);
  img.onerror = () => loadQr(img);
  img.src = QR_MIRRORS[attempt] + (img.dataset.qr ?? "");
}

/** 页面挂载后再取图：模板还没渲染时 querySelector 拿不到节点。 */
function wireQrImages() {
  for (const img of document.querySelectorAll<HTMLImageElement>("#about-cards img[data-qr]")) {
    loadQr(img);
  }
}

const { settings, reset, flush, reload } = useSettings();
const tauri = isTauri();

/** 三个语言选项。跟随系统的实际语言由 i18n 在启动时算，不在 UI 上展开地区变体。 */
const LANGUAGE_OPTIONS: { value: "system" | "zh" | "en"; label: MessageKey }[] = [
  { value: "system", label: "language.system" },
  { value: "zh", label: "language.zh" },
  { value: "en", label: "language.en" },
];

/**
 * 界面外观（ADR-0025）。三态而不是开关：纯布尔值表达不了「跟随系统」，
 * 而在浅色系统上想固定深色的人也需要一个明确选项。
 */
const APPEARANCE_OPTIONS: { value: UiAppearance; label: MessageKey }[] = [
  { value: "system", label: "uiAppearance.system" },
  { value: "light", label: "uiAppearance.light" },
  { value: "dark", label: "uiAppearance.dark" },
];

/** 自启状态不来自配置文件，而是每次读注册表——任务管理器里禁用也要能反映出来。 */
const autostart = ref(false);
const busy = ref(false);
const version = ref("");
const message = ref<{ tone: "ok" | "err"; text: string } | null>(null);
let messageTimer: ReturnType<typeof setTimeout> | undefined;
let unlisten: (() => void) | undefined;

const {
  state: updateState,
  installed: installedBuild,
  loadInstallKind,
  checkForUpdate,
  installUpdate,
} = useUpdater();

function notify(tone: "ok" | "err", text: string) {
  message.value = { tone, text };
  clearTimeout(messageTimer);
  messageTimer = setTimeout(() => (message.value = null), 4200);
}

async function refreshAutostart() {
  if (!isTauri()) return;
  autostart.value = await invoke<boolean>("autostart_status");
}

async function toggleAutostart() {
  if (!isTauri() || busy.value) return;
  busy.value = true;
  const next = !autostart.value;
  try {
    await invoke("set_autostart", { enabled: next });
    autostart.value = next;
    notify("ok", t(next ? "system.autostartOn" : "system.autostartOff"));
  } catch (error) {
    // 写注册表失败（组策略、权限）时要让用户看见原因，开关保持原状。
    notify("err", errorText(error));
    await refreshAutostart();
  } finally {
    busy.value = false;
  }
}

function setNoArgMode(mode: NoArgMode) {
  settings.launch.noArgMode = mode;
}

/* -------------------------------- 配置迁移（ADR-0027） -------------------------------- */

const importSummary = ref<ConfigBundleSummary | null>(null);
const importPath = ref<string | null>(null);
const importBusy = ref(false);
const exportBusy = ref(false);

function bundleFileName(): string {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  return `FlipClock-config-${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}.zip`;
}

async function exportConfig(): Promise<void> {
  if (!tauri || exportBusy.value) return;
  exportBusy.value = true;
  try {
    const path = await save({
      defaultPath: bundleFileName(),
      filters: [{ name: t("system.configExportFilter"), extensions: ["zip"] }],
    });
    if (!path) return;
    await flush();
    await invoke("export_config_bundle", { path });
    notify("ok", t("system.configExportSuccess"));
  } catch (error) {
    notify("err", errorText(error));
  } finally {
    exportBusy.value = false;
  }
}

async function startImport(): Promise<void> {
  if (!tauri || importBusy.value) return;
  importBusy.value = true;
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: t("system.configExportFilter"), extensions: ["zip"] }],
    });
    const path = Array.isArray(selected) ? selected[0] : selected;
    if (!path) return;
    await flush();
    const summary = await invoke<ConfigBundleSummary>("inspect_config_bundle", { path });
    importPath.value = path;
    importSummary.value = summary;
  } catch (error) {
    notify("err", errorText(error));
  } finally {
    importBusy.value = false;
  }
}

function cancelImport(): void {
  if (importBusy.value) return;
  importSummary.value = null;
  importPath.value = null;
}

const SKIP_REASON_KEYS: Record<ConfigImportSkip["reason"], MessageKey> = {
  themeLimit: "system.configSkipThemeLimit",
  fontLimit: "system.configSkipFontLimit",
  fontMissing: "system.configSkipFontMissing",
};

function skipReasonLabel(skip: ConfigImportSkip): string {
  return t(SKIP_REASON_KEYS[skip.reason], { name: skip.name });
}

async function confirmImport(selection: ConfigBundleSelection): Promise<void> {
  const path = importPath.value;
  if (!path) return;
  importBusy.value = true;
  try {
    const result = await invoke<ConfigImportResult>("import_config_bundle", {
      path,
      selection,
    });
    importSummary.value = null;
    importPath.value = null;
    await reload();
    const skipped = result.skipped
      .map(skipReasonLabel)
      .join(locale.value === "zh" ? "、" : ", ");
    const summary = t("system.configImportResult", {
      themes: result.themes,
      fonts: result.fonts,
      skipped: result.skipped.length,
    });
    notify(
      "ok",
      result.skipped.length
        ? `${summary} ${t("system.configImportSkipped", { items: skipped })}`
        : summary,
    );
  } catch (error) {
    notify("err", errorText(error));
  } finally {
    importBusy.value = false;
  }
}

/**
 * 恢复默认是全局破坏性操作：图标按钮换成系统页里带清单的按钮后，
 * 弹窗必须说清「会清什么、不会清什么」。reset() 清外观/屏保/启动/系统/语言，
 * 但注册信息与回滚点（registration）保留——清单里的两条说明就按这个事实写。
 */
const restorePrompt = ref(false);
const restoreBusy = ref(false);
async function confirmRestore() {
  restoreBusy.value = true;
  try {
    // reset 会连带删掉自定义主题的图片目录（ADR-0020 D15）。删不掉的 id 会返回来，
    // 这时配置已经恢复默认了——不能说「失败」，但要说明磁盘上还有残留。
    const failed = await reset();
    restorePrompt.value = false;
    if (failed.length > 0) {
      notify("err", t("system.restoreThemeCleanupFailed", { count: failed.length }));
    } else {
      notify("ok", t("settings.restoreSuccess"));
    }
  } finally {
    restoreBusy.value = false;
  }
}

const updateBusy = computed(() =>
  ["checking", "downloading", "installing"].includes(updateState.value.status),
);

const updateTitle = computed(() => {
  switch (updateState.value.status) {
    case "checking":
      return t("system.update.checking");
    case "upToDate":
      return t("system.update.upToDate");
    case "available":
      return t("system.update.available", { version: updateState.value.version });
    case "downloading":
      return t("system.update.downloading", { percent: updateState.value.percent });
    case "installing":
      return t("system.update.installing");
    case "error":
      return t("system.update.failed");
    default:
      return t("system.update.title");
  }
});

const updateHint = computed(() => {
  switch (updateState.value.status) {
    case "available":
      return installedBuild.value === false
        ? t("system.update.portableHint")
        : t("system.update.downloadHint");
    case "installing":
      return t("system.update.installerHint");
    case "error":
      return updateState.value.message;
    case "idle":
      return installedBuild.value === false ? t("system.update.portableIdle") : "";
    default:
      return "";
  }
});

const updateAction = computed<"check" | "install" | null>(() => {
  const status = updateState.value.status;
  if (status === "available") {
    return installedBuild.value === false ? null : "install";
  }
  if (status === "checking" || status === "downloading" || status === "installing") return null;
  return "check";
});

onMounted(async () => {
  wireQrImages();
  if (!isTauri()) return;
  version.value = await getVersion();
  await Promise.all([refreshAutostart(), loadInstallKind()]);
  // 托盘菜单也能切自启，两边共用注册表这份事实。
  unlisten = await listen<boolean>("autostart-changed", (event) => {
    autostart.value = event.payload;
  });
});

onBeforeUnmount(() => {
  unlisten?.();
  clearTimeout(messageTimer);
});
</script>

<template>
  <div class="system-page">
    <section class="section">
      <div class="section-head">
        <span class="section-title">{{ t("system.startupAndWindow") }}</span>
      </div>

      <div class="lines">
        <div class="row">
          <div class="row-copy">
            <span class="row-name">{{ t("system.autostart") }}</span>
            <span class="row-hint">{{ t("system.autostartHint") }}</span>
          </div>
          <button
            type="button"
            class="switch"
            role="switch"
            :aria-checked="autostart"
            :class="{ on: autostart }"
            :disabled="busy"
            @click="toggleAutostart"
          >
            <i></i>
          </button>
        </div>

        <div class="row">
          <div class="row-copy">
            <span class="row-name">{{ t("system.closeToTray") }}</span>
            <span class="row-hint">{{ t("system.closeToTrayHint") }}</span>
          </div>
          <button
            type="button"
            class="switch"
            role="switch"
            :aria-checked="settings.system.closeToTray"
            :class="{ on: settings.system.closeToTray }"
            @click="settings.system.closeToTray = !settings.system.closeToTray"
          >
            <i></i>
          </button>
        </div>

        <div class="row">
          <div class="row-copy">
            <span class="row-name">{{ t("system.noArgMode") }}</span>
            <span class="row-hint">{{ t("system.noArgHint") }}</span>
          </div>
          <div class="segmented">
            <button
              type="button"
              :class="{ selected: settings.launch.noArgMode === 'settings' }"
              @click="setNoArgMode('settings')"
            >
              {{ t("system.noArgSettings") }}
            </button>
            <button
              type="button"
              :class="{ selected: settings.launch.noArgMode === 'fullscreen' }"
              @click="setNoArgMode('fullscreen')"
            >
              {{ t("system.noArgFullscreen") }}
            </button>
          </div>
        </div>
      </div>

      <p v-if="message" class="system-message" :class="message.tone">{{ message.text }}</p>
    </section>

    <section class="section">
      <div class="section-head">
        <span class="section-title">{{ t("language.title") }}</span>
      </div>
      <div class="segmented narrow">
        <button
          v-for="option in LANGUAGE_OPTIONS"
          :key="option.value"
          type="button"
          :class="{ selected: settings.ui.language === option.value }"
          @click="settings.ui.language = option.value"
        >
          {{ t(option.label) }}
        </button>
      </div>
      <p class="row-hint">{{ t("language.hint") }}</p>
    </section>

    <section class="section">
      <div class="section-head">
        <span class="section-title">{{ t("uiAppearance.title") }}</span>
      </div>
      <div class="segmented narrow">
        <button
          v-for="option in APPEARANCE_OPTIONS"
          :key="option.value"
          type="button"
          :class="{ selected: settings.ui.appearance === option.value }"
          @click="settings.ui.appearance = option.value"
        >
          {{ t(option.label) }}
        </button>
      </div>
      <p class="row-hint">{{ t("uiAppearance.hint") }}</p>
    </section>

    <section v-if="tauri" class="section">
      <div class="section-head">
        <span class="section-title">{{ t("system.configMigration") }}</span>
      </div>
      <div class="lines">
        <div class="row">
          <div class="row-copy">
            <span class="row-name">{{ t("system.configExport") }}</span>
            <span class="row-hint">{{ t("system.configExportHint") }}</span>
          </div>
          <button type="button" class="secondary" :disabled="exportBusy" @click="exportConfig">
            {{ t("system.configExportAction") }}
          </button>
        </div>
        <div class="row">
          <div class="row-copy">
            <span class="row-name">{{ t("system.configImport") }}</span>
            <span class="row-hint">{{ t("system.configImportHint") }}</span>
          </div>
          <button type="button" class="secondary" :disabled="importBusy" @click="startImport">
            {{ t("system.configImportAction") }}
          </button>
        </div>
      </div>
    </section>

    <section class="section">
      <div class="section-head">
        <span class="section-title">{{ t("system.restoreSection") }}</span>
      </div>
      <div class="lines">
        <div class="row">
          <div class="row-copy">
            <span class="row-name">{{ t("system.restoreDefaults") }}</span>
            <span class="row-hint">{{ t("system.restoreHint") }}</span>
          </div>
          <button type="button" class="secondary" @click="restorePrompt = true">
            {{ t("system.restoreAction") }}
          </button>
        </div>
      </div>
    </section>

    <section class="section">
      <div class="section-head">
        <span class="section-title">{{ t("system.about") }}</span>
      </div>

      <div class="lines">
        <div class="row">
          <span class="row-name">{{ t("system.version") }}</span>
          <span class="info-value">{{ version || "—" }}</span>
        </div>

        <div class="row update">
          <div class="row-copy">
            <span class="row-name" :class="{ err: updateState.status === 'error' }">
              {{ updateTitle }}
            </span>
            <span v-if="updateHint" class="row-hint update-hint">{{ updateHint }}</span>
          </div>
          <button
            v-if="updateAction === 'check'"
            type="button"
            class="secondary"
            :disabled="updateBusy"
            @click="checkForUpdate({})"
          >
            {{ t(updateState.status === "error" ? "system.update.retry" : "system.update.check") }}
          </button>
          <button
            v-else-if="updateAction === 'install'"
            type="button"
            class="primary"
            :disabled="updateBusy"
            @click="installUpdate"
          >
            {{ t("system.update.downloadInstall") }}
          </button>
        </div>
      </div>
    </section>

    <!-- 关于作者、公众号与赞助：图片一律走远端仓库，不占安装包体积。 -->
    <div id="about-cards">
      <section class="section">
        <div class="section-head">
          <span class="section-title">{{ t("system.aboutAuthor") }}</span>
        </div>
        <p class="about-text">{{ t("system.aboutText") }}</p>
        <div class="about-links">
          <button type="button" class="secondary" @click="openAboutLink('website')">
            {{ t("system.officialWebsite") }}
          </button>
          <button type="button" class="secondary" @click="openAboutLink('projects')">
            {{ t("system.openSource") }}
          </button>
          <button type="button" class="secondary" @click="openAboutLink('issues')">
            {{ t("system.feedback") }}
          </button>
        </div>
      </section>

      <section class="section">
        <div class="section-head">
          <span class="section-title">{{ t("system.followWechat") }}</span>
        </div>
        <div class="qr-slot qr-banner">
          <img data-qr="wechat-official.png" :alt="t('system.wechatQrAlt')" referrerpolicy="no-referrer" />
          <p class="qr-fail">{{ t("system.wechatQrFail") }}</p>
        </div>
      </section>

      <section class="section">
        <div class="section-head">
          <span class="section-title">{{ t("system.sponsor") }}</span>
        </div>
        <p class="about-text">{{ t("system.sponsorText") }}</p>
        <div class="qr-row">
          <div class="qr-slot qr-tile">
            <img data-qr="alipay.png" :alt="t('system.alipayQrAlt')" referrerpolicy="no-referrer" />
            <p class="qr-fail">{{ t("system.alipayQrFail") }}</p>
          </div>
          <div class="qr-slot qr-tile">
            <img data-qr="wechat-pay.png" :alt="t('system.wechatPayQrAlt')" referrerpolicy="no-referrer" />
            <p class="qr-fail">{{ t("system.wechatPayQrFail") }}</p>
          </div>
        </div>
      </section>
    </div>

    <!-- 恢复默认的影响清单。不能只用一句 confirm：reset() 动六段配置，
         用户有权在清掉之前看到「哪些自建内容会消失」。 -->
    <Transition name="fade">
      <div v-if="restorePrompt" class="confirm-backdrop" @click.self="restorePrompt = false">
        <div class="confirm" role="dialog" aria-modal="true" :aria-label="t('system.restoreDefaults')">
          <strong>{{ t("system.restoreConfirmTitle") }}</strong>
          <p class="confirm-lead">{{ t("system.restoreConfirmLead") }}</p>
          <ul class="confirm-list">
            <li>{{ t("system.restoreScopeTheme") }}</li>
            <li>{{ t("system.restoreScopeClock") }}</li>
            <li>{{ t("system.restoreScopeSaver") }}</li>
            <li>{{ t("system.restoreScopeStartup") }}</li>
            <li>{{ t("system.restoreScopeLanguage") }}</li>
            <li>{{ t("system.restoreScopeAppearance") }}</li>
          </ul>
          <p class="confirm-keep">{{ t("system.restoreKeep") }}</p>
          <div class="confirm-actions">
            <button type="button" class="secondary" @click="restorePrompt = false">
              {{ t("system.restoreCancel") }}
            </button>
            <button type="button" class="primary" :disabled="restoreBusy" @click="confirmRestore">
              {{ t("system.restoreAction") }}
            </button>
          </div>
        </div>
      </div>
    </Transition>

    <ConfigBundleDialog
      v-if="importSummary"
      :summary="importSummary"
      :busy="importBusy"
      @cancel="cancelImport"
      @confirm="confirmImport"
    />
  </div>
</template>

<style scoped>
.system-page {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.segmented.narrow {
  align-self: flex-start;
  /* 不设固定宽度：容器按按钮内容收缩，后面才不会多出一截空灰底。
     早先写死 208/264px 是按"别铺满整行"来的，但 align-self:flex-start
     本身就已经贴合内容，宽度反而成了多余的灰。 */
  width: max-content;
}

.info-value {
  font-size: 11px;
  color: var(--ui-text);
}

.update-hint {
  max-width: 380px;
}

.system-message {
  font-size: 10.5px;
}

.system-message.ok {
  color: var(--ui-success);
}

.system-message.err {
  color: var(--ui-danger);
}

.about-text {
  font-size: 11px;
  line-height: 1.65;
  color: var(--ui-text-secondary);
}

.about-links {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.qr-row {
  display: flex;
  gap: 12px;
}

.qr-slot {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 116px;
  border-radius: var(--radius-panel);
  background: var(--ui-surface-sunken);
  overflow: hidden;
}

.qr-slot img {
  display: block;
  max-width: 100%;
  height: auto;
}

.qr-tile {
  flex: 1;
  min-width: 0;
  padding: 8px;
}

.qr-tile img {
  width: 128px;
}

.qr-banner img {
  width: 100%;
  max-width: 400px;
}

.qr-fail {
  display: none;
  margin: 0;
  padding: 12px;
  font-size: 11px;
  line-height: 1.6;
  color: var(--ui-text-muted);
  text-align: center;
}

.qr-slot.failed .qr-fail {
  display: block;
}

/* 弹窗盖在整窗之上（含标题栏），不能放进 .page-scroll——那会跟着滚动。 */
.confirm-backdrop {
  position: fixed;
  inset: 0;
  z-index: 600;
  display: grid;
  place-items: center;
  background: var(--ui-scrim);
}

.confirm {
  width: min(420px, calc(100% - 48px));
  padding: 18px 20px 16px;
  border: 1px solid var(--ui-divider-strong);
  border-radius: 6px;
  background: var(--ui-surface);
  box-shadow: var(--ui-shadow-dialog);
}

.confirm strong {
  display: block;
  font-size: 14px;
  font-weight: 600;
  color: var(--ui-text);
}

.confirm-lead,
.confirm-keep {
  margin: 9px 0 0;
  font-size: 11px;
  line-height: 1.6;
  color: var(--ui-text-secondary);
}

.confirm-list {
  margin: 8px 0 0;
  padding: 0 0 0 17px;
  font-size: 11px;
  line-height: 1.75;
  color: var(--ui-text-secondary);
}

.confirm-list li + li {
  margin-top: 1px;
}

.confirm-keep {
  margin-top: 10px;
  padding-top: 9px;
  border-top: 1px solid var(--ui-divider);
  font-size: 10px;
  color: var(--ui-text-muted);
}

.confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.14s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
