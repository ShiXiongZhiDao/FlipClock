<script setup lang="ts">
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";

import { useSettings } from "../../composables/useSettings";
import { isTauri } from "../../lib/env";
import { errorText, t, type MessageKey } from "../../lib/i18n";
import { useUpdater } from "../../lib/updater";
import type { ScreenSaverStatus } from "../../types/settings";

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

const { settings } = useSettings();

/** 三个语言选项。跟随系统的实际语言由 i18n 在启动时算，不在 UI 上展开地区变体。 */
const LANGUAGE_OPTIONS: { value: "system" | "zh" | "en"; label: MessageKey }[] = [
  { value: "system", label: "language.system" },
  { value: "zh", label: "language.zh" },
  { value: "en", label: "language.en" },
];

/** 自启状态不来自配置文件，而是每次读注册表——任务管理器里禁用也要能反映出来。 */
const autostart = ref(false);
const busy = ref(false);
const version = ref("");
const saver = ref<ScreenSaverStatus | null>(null);
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

/** 术语表里的四种注册状态一个都不能少：二分成「已装/未装」会把过期说成没装。 */
const SAVER_LABEL: Record<string, MessageKey> = {
  notInstalled: "saver.status.notInstalled",
  installed: "saver.status.installed",
  missing: "saver.status.missing",
  outdated: "saver.status.outdated",
};

const saverLabel = computed(() =>
  saver.value
    ? SAVER_LABEL[saver.value.state]
      ? t(SAVER_LABEL[saver.value.state])
      : saver.value.state
    : "—",
);
const saverNeedsInstall = computed(
  () => saver.value?.state === "outdated" || saver.value?.state === "missing",
);

async function reinstallSaver() {
  if (!isTauri() || busy.value) return;
  busy.value = true;
  try {
    saver.value = await invoke<ScreenSaverStatus>("install_screensaver");
    notify("ok", t("system.saverUpdatedToast"));
  } catch (error) {
    notify("err", errorText(error));
  } finally {
    busy.value = false;
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
  saver.value = await invoke<ScreenSaverStatus>("screensaver_status");
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
    <section class="card">
      <div class="card-title">{{ t("language.title") }}</div>
      <div class="language-options">
        <button
          v-for="option in LANGUAGE_OPTIONS"
          :key="option.value"
          type="button"
          class="language-option"
          :class="{ selected: settings.ui.language === option.value }"
          @click="settings.ui.language = option.value"
        >
          {{ t(option.label) }}
        </button>
      </div>
      <p class="row-hint language-hint">{{ t("language.hint") }}</p>
    </section>

    <section class="card">
      <div class="card-title">{{ t("common.startup") }}</div>

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

      <div class="divider"></div>

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

      <p v-if="message" class="system-message" :class="message.tone">
        {{ message.text }}
      </p>
    </section>

    <section class="card">
      <div class="card-title">{{ t("system.about") }}</div>
      <div class="info-row">
        <span>{{ t("system.version") }}</span>
        <strong>{{ version || "—" }}</strong>
      </div>
      <div class="info-row">
        <span>{{ t("system.saverComponent") }}</span>
        <span class="info-state">
          <strong :class="{ warn: saverNeedsInstall }">{{ saverLabel }}</strong>
          <button
            v-if="saverNeedsInstall"
            type="button"
            class="link-button"
            :disabled="busy"
            @click="reinstallSaver"
          >
            {{ t("system.reinstall") }}
          </button>
        </span>
      </div>
      <div
        v-if="saver?.activePath ?? saver?.targetPath"
        class="info-path"
        :title="saver?.activePath ?? saver?.targetPath ?? ''"
      >
        {{ saver?.activePath ?? saver?.targetPath }}
      </div>

      <div class="divider"></div>

      <div class="update-box">
        <div class="update-copy">
          <span class="update-title" :class="{ err: updateState.status === 'error' }">
            {{ updateTitle }}
          </span>
          <span v-if="updateHint" class="update-hint">{{ updateHint }}</span>
        </div>
        <button
          v-if="updateAction === 'check'"
          type="button"
          class="ghost-button"
          :disabled="updateBusy"
          @click="checkForUpdate({})"
        >
          {{ t(updateState.status === "error" ? "system.update.retry" : "system.update.check") }}
        </button>
        <button
          v-else-if="updateAction === 'install'"
          type="button"
          class="ghost-button accent"
          :disabled="updateBusy"
          @click="installUpdate"
        >
          {{ t("system.update.downloadInstall") }}
        </button>
      </div>
    </section>

    <!-- 关于作者、公众号与赞助：图片一律走远端仓库，不占安装包体积。 -->
    <div id="about-cards" class="about-cards">
      <section class="card">
        <div class="card-title">{{ t("system.aboutAuthor") }}</div>
        <p class="about-text">{{ t("system.aboutText") }}</p>
        <div class="about-links">
          <button type="button" class="ghost-button" @click="openAboutLink('website')">
            {{ t("system.officialWebsite") }}
          </button>
          <button type="button" class="ghost-button" @click="openAboutLink('projects')">
            {{ t("system.openSource") }}
          </button>
          <button type="button" class="ghost-button" @click="openAboutLink('issues')">
            {{ t("system.feedback") }}
          </button>
        </div>
      </section>

      <section class="card">
        <div class="card-title">{{ t("system.followWechat") }}</div>
        <div class="qr-slot qr-banner">
          <img data-qr="wechat-official.png" :alt="t('system.wechatQrAlt')" referrerpolicy="no-referrer" />
          <p class="qr-fail">{{ t("system.wechatQrFail") }}</p>
        </div>
      </section>

      <section class="card">
        <div class="card-title">{{ t("system.sponsor") }}</div>
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
  </div>
</template>

<style scoped>
.system-page {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--ui-scroll) transparent;
}

/* 两段设置而已，撑满整页会像少了东西；但也别缩成一条，留到能读的宽度。 */
.card {
  width: 100%;
  max-width: 560px;
}

.row-hint {
  font-size: 10px;
  color: var(--ui-text-muted);
}

.language-options {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  margin-top: 10px;
}

.language-option {
  padding: 8px 10px;
  border: 1px solid var(--ui-divider);
  border-radius: 9px;
  background: var(--ui-bg);
  color: var(--ui-text-secondary);
  cursor: pointer;
  transition: border-color 0.16s ease, box-shadow 0.16s ease;
}

.language-option:hover {
  border-color: var(--ui-accent-line);
}

.language-option.selected {
  border-color: var(--ui-accent-line-strong);
  box-shadow: 0 0 0 1px var(--ui-accent-halo);
  color: var(--ui-text);
}

.language-hint {
  margin: 8px 0 0;
}

.info-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 0;
  font-size: 12px;
  color: var(--ui-text-secondary);
}

.info-row strong {
  font-weight: 500;
  color: var(--ui-text);
}

.info-row strong.warn {
  color: var(--ui-danger);
}

.info-state {
  display: inline-flex;
  align-items: baseline;
  gap: 8px;
}

.link-button {
  padding: 0;
  font-size: 11px;
  color: var(--ui-accent);
  text-decoration: underline;
  text-underline-offset: 2px;
}

.link-button:disabled {
  opacity: 0.5;
}

.info-path {
  padding-top: 4px;
  font-size: 10px;
  color: var(--ui-text-muted);
  word-break: break-all;
}

.update-box {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-top: 2px;
}

.update-copy {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.update-title {
  font-size: 12px;
  color: var(--ui-text);
}

.update-title.err {
  color: var(--ui-danger);
}

.update-hint {
  font-size: 10px;
  line-height: 1.5;
  color: var(--ui-text-muted);
  word-break: break-word;
}

.ghost-button {
  flex: none;
  padding: 6px 11px;
  font-size: 11px;
  border-radius: 7px;
  border: 1px solid var(--ui-divider);
  background: var(--ui-surface);
  color: var(--ui-text-secondary);
  transition: color 0.16s ease, border-color 0.16s ease;
}

.ghost-button:hover:not(:disabled) {
  color: var(--ui-text);
  border-color: var(--ui-accent);
}

.ghost-button.accent {
  color: var(--ui-accent);
  border-color: var(--ui-accent);
}

.ghost-button:disabled {
  opacity: 0.5;
}

.system-message {
  margin: 4px 0 0;
  font-size: 11px;
}

.system-message.ok {
  color: var(--ui-accent-text);
}

.system-message.err {
  color: var(--ui-danger);
}

.about-cards {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.about-text {
  margin: 0;
  font-size: 11px;
  line-height: 1.6;
  color: var(--ui-text-secondary);
}

.about-links {
  display: flex;
  gap: 8px;
}

.about-links .ghost-button {
  display: inline-flex;
  align-items: center;
}

.qr-row {
  display: flex;
  gap: 14px;
}

.qr-slot {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 120px;
  border-radius: 8px;
  background: var(--ui-track, var(--ui-bg));
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
  width: 132px;
}

.qr-banner img {
  width: 100%;
  max-width: 420px;
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
</style>
