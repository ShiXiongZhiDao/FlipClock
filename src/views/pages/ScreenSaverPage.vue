<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";

import { useScreensaverStatus } from "../../composables/useScreensaverStatus";
import { useSettings } from "../../composables/useSettings";
import { errorText, t } from "../../lib/i18n";
import { SAVER_STATE_UI, saverUiState, saverWindowsOff } from "../../lib/saverStatus";
import type { MonitorMode } from "../../types/settings";

const { settings } = useSettings();
const {
  status,
  statusError,
  busy,
  refresh,
  install: installComponent,
  restore: restoreComponent,
  openWindowsSettings,
} = useScreensaverStatus();

const message = ref<{ tone: "ok" | "err"; text: string } | null>(null);
let messageTimer: ReturnType<typeof setTimeout> | undefined;

const uiState = computed(() => saverUiState(status.value));

const statusInfo = computed(() => {
  const state = uiState.value;
  if (!state) return null;
  const meta = SAVER_STATE_UI[state];
  return { label: t(meta.pageLabel), tone: meta.tone };
});

const primaryLabel = computed(() => {
  const state = uiState.value;
  if (!state) return null;
  const key = SAVER_STATE_UI[state].pageAction;
  return key ? t(key) : null;
});

/**
 * 只读状态行：启用/关闭交给 Windows 屏保设置，避免在应用里再造一个写注册表的开关。
 * 未安装时显示「—」，不要让人以为 FlipClock 已经在待命。
 */
const windowsState = computed(() => {
  const value = status.value;
  if (!value || value.state === "notInstalled") return "—";
  return value.windowsEnabled ? t("saver.windowsOn") : t("saver.windowsOff");
});

const showEnable = computed(() => saverWindowsOff(status.value));

function notify(tone: "ok" | "err", text: string) {
  message.value = { tone, text };
  clearTimeout(messageTimer);
  messageTimer = setTimeout(() => (message.value = null), 4200);
}

async function install() {
  try {
    await installComponent();
    notify("ok", t("saver.installToast"));
  } catch (e) {
    notify("err", errorText(e));
  }
}

async function restore() {
  try {
    await restoreComponent();
    notify("ok", t("saver.restoreToast"));
  } catch (e) {
    notify("err", errorText(e));
  }
}

async function openSystemSettings() {
  try {
    await openWindowsSettings();
  } catch (e) {
    notify("err", errorText(e));
  }
}

function setMonitorMode(mode: MonitorMode) {
  settings.screensaver.monitorMode = mode;
}

onMounted(refresh);

onBeforeUnmount(() => clearTimeout(messageTimer));
</script>

<template>
  <div class="saver-page">
    <section class="hero">
      <span class="hero-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24">
          <rect x="3" y="4" width="18" height="13" rx="2" />
          <path d="M8 21h8M12 17v4M12 7v4l2.5 1.5" />
        </svg>
      </span>
      <span class="hero-copy">
        <strong>{{ t("saver.productTitle") }}</strong>
        <small>{{ t("saver.productDesc") }}</small>
      </span>
      <span v-if="statusInfo" class="badge" :class="statusInfo.tone">{{ statusInfo.label }}</span>
      <button
        v-if="primaryLabel"
        class="primary"
        type="button"
        :disabled="busy"
        @click="install"
      >
        {{ primaryLabel }}
      </button>
    </section>

    <p class="install-note">{{ t("saver.installNote") }}</p>

    <p v-if="statusError" class="msg err" :title="statusError">
      {{ t("saver.statusReadError") }}
      <button type="button" class="mini" @click="refresh">
        {{ t("saver.retry") }}
      </button>
    </p>

    <Transition name="msg">
      <p v-if="message" class="msg" :class="message.tone">{{ message.text }}</p>
    </Transition>

    <section class="section">
      <div class="section-head">
        <span class="section-title">{{ t("saver.monitorTitle") }}</span>
      </div>
      <div class="segmented narrow">
        <button
          type="button"
          :class="{ selected: settings.screensaver.monitorMode === 'primary' }"
          @click="setMonitorMode('primary')"
        >
          {{ t("saver.monitorPrimary") }}
        </button>
        <button
          type="button"
          :class="{ selected: settings.screensaver.monitorMode === 'all' }"
          @click="setMonitorMode('all')"
        >
          {{ t("saver.monitorAll") }}
        </button>
      </div>
    </section>

    <section class="section">
      <div class="section-head">
        <span class="section-title">{{ t("saver.details") }}</span>
        <button type="button" class="mini" @click="openSystemSettings">
          {{ t("saver.openSettings") }}
        </button>
      </div>

      <div class="kv">
        <span class="kv-key">{{ t("saver.currentTarget") }}</span>
        <span class="kv-value">{{ status?.activePath || t("saver.notSet") }}</span>

        <span class="kv-key">{{ t("saver.windowsState") }}</span>
        <span class="kv-value">
          {{ windowsState }}
          <template v-if="showEnable">
            ·
            <button type="button" class="mini" @click="openSystemSettings">
              {{ t("saver.enableWindows") }}
            </button>
          </template>
        </span>

        <span class="kv-key">{{ t("saver.componentVersion") }}</span>
        <span class="kv-value">
          {{
            status?.state === "installed"
              ? t("saver.versionMatch")
              : status?.state === "outdated"
                ? t("saver.versionMismatch")
                : "—"
          }}
        </span>

        <span class="kv-key">{{ t("saver.rollbackPoint") }}</span>
        <span class="kv-value">
          {{ status?.rollbackAvailable ? t("saver.rollbackRecorded") : t("saver.rollbackNone") }}
        </span>
      </div>

      <div
        v-if="status?.state === 'installed' || status?.rollbackAvailable"
        class="actions"
      >
        <button
          v-if="status?.state === 'installed'"
          class="secondary"
          type="button"
          :disabled="busy"
          @click="install"
        >
          {{ t("saver.redeploy") }}
        </button>
        <button
          v-if="status?.rollbackAvailable"
          class="secondary"
          type="button"
          :disabled="busy"
          @click="restore"
        >
          {{ t("saver.restoreOriginal") }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.saver-page {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

/* 屏保这一段是整页唯一的主操作，给一个带边框的 hero 把它抬起来。
   其余段落仍是平铺的——一页里只能有一个「重点」。 */
.hero {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 14px;
  margin-bottom: 14px;
  border: 1px solid var(--ui-divider-strong);
  border-radius: var(--radius-panel);
  background: var(--ui-hero);
}

.hero-icon {
  flex: none;
  width: 36px;
  height: 36px;
  display: grid;
  place-items: center;
  border-radius: 4px;
  background: var(--ui-brand-tile);
  color: var(--ui-brand-ink);
  /* 深色壳里深底方块贴着深色面板没有边界，补一圈亮边 */
  box-shadow: inset 0 0 0 1px var(--ui-tile-ring);
}

.hero-icon svg {
  width: 20px;
  height: 20px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.hero-copy {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.hero-copy strong {
  font-size: 11.5px;
  font-weight: 620;
  color: var(--ui-text);
}

.hero-copy small {
  font-size: 10px;
  color: var(--ui-text-muted);
}

.install-note {
  margin: -4px 2px 14px;
  font-size: 9.5px;
  line-height: 1.5;
  color: var(--ui-text-muted);
}

.msg {
  margin: -6px 0 14px;
  padding: 8px 10px;
  font-size: 10.5px;
  border-radius: var(--radius-card);
}

.msg.ok {
  color: var(--ui-success);
  background: var(--ui-success-soft);
}

.msg.err {
  color: var(--ui-danger);
  background: var(--ui-danger-soft);
}

.kv {
  display: grid;
  grid-template-columns: 78px minmax(0, 1fr);
  gap: 7px 12px;
  font-size: 10.5px;
}

.kv-key {
  color: var(--ui-text-muted);
}

.kv-value {
  color: var(--ui-text-secondary);
  word-break: break-all;
}

.actions {
  display: flex;
  gap: 8px;
}

/* 打开系统设置是「跳出去」的动作，给它一条淡下划线；
   界面里没有蓝色，靠下划线而不是颜色来区分「这是链接」。 */
.mini {
  padding: 2px 0;
  font-size: 10px;
  color: var(--ui-text-secondary);
  text-decoration: underline;
  text-decoration-color: var(--ui-link-line);
  text-underline-offset: 3px;
  transition: color 0.12s ease, text-decoration-color 0.12s ease;
}

.mini:hover {
  color: var(--ui-text);
  text-decoration-color: var(--ui-link-line-hover);
}

.segmented.narrow {
  align-self: flex-start;
  /* 不设固定宽度：容器按按钮内容收缩，后面才不会多出一截空灰底。
     早先写死 208/264px 是按"别铺满整行"来的，但 align-self:flex-start
     本身就已经贴合内容，宽度反而成了多余的灰。 */
  width: max-content;
}

.msg-enter-active,
.msg-leave-active {
  transition: opacity 0.16s ease;
}

.msg-enter-from,
.msg-leave-to {
  opacity: 0;
}
</style>
