<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { computed, onMounted, ref } from "vue";

import { useSettings } from "../../composables/useSettings";
import { trackFeatureToggled } from "../../lib/analytics";
import { isTauri } from "../../lib/env";
import { errorText, t, type MessageKey } from "../../lib/i18n";
import type {
  MonitorMode,
  RegistrationState,
  ScreenSaverStatus,
} from "../../types/settings";

const { settings } = useSettings();

const status = ref<ScreenSaverStatus | null>(null);
const busy = ref(false);
const message = ref<{ tone: "ok" | "err"; text: string } | null>(null);
let messageTimer: ReturnType<typeof setTimeout> | undefined;

const STATUS_TEXT: Record<RegistrationState, { key: MessageKey; tone: string }> = {
  notInstalled: { key: "saver.status.notInstalled", tone: "idle" },
  installed: { key: "saver.status.installed", tone: "ok" },
  missing: { key: "saver.status.missing", tone: "err" },
  outdated: { key: "saver.status.outdated", tone: "warn" },
};

const statusInfo = computed(() =>
  status.value
    ? { label: t(STATUS_TEXT[status.value.state].key), tone: STATUS_TEXT[status.value.state].tone }
    : null,
);

const deployed = computed(() => status.value?.state === "installed");

function notify(tone: "ok" | "err", text: string) {
  message.value = { tone, text };
  clearTimeout(messageTimer);
  messageTimer = setTimeout(() => (message.value = null), 4200);
}

async function refresh() {
  if (!isTauri()) return;
  try {
    status.value = await invoke<ScreenSaverStatus>("screensaver_status");
  } catch (e) {
    notify("err", errorText(e));
  }
}

async function install() {
  busy.value = true;
  try {
    status.value = await invoke<ScreenSaverStatus>("install_screensaver");
    trackFeatureToggled("screensaver", true);
    notify("ok", t("saver.installToast"));
  } catch (e) {
    notify("err", errorText(e));
  } finally {
    busy.value = false;
  }
}

async function restore() {
  busy.value = true;
  try {
    status.value = await invoke<ScreenSaverStatus>("restore_screensaver");
    trackFeatureToggled("screensaver", false);
    notify("ok", t("saver.restoreToast"));
  } catch (e) {
    notify("err", errorText(e));
  } finally {
    busy.value = false;
  }
}

async function openSystemSettings() {
  try {
    await invoke("open_windows_screensaver_settings");
  } catch (e) {
    notify("err", errorText(e));
  }
}

function setMonitorMode(mode: MonitorMode) {
  settings.screensaver.monitorMode = mode;
}

onMounted(refresh);
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
      <button class="primary" type="button" :disabled="busy" @click="install">
        {{ t(deployed ? "saver.redeploy" : "saver.installShort") }}
      </button>
    </section>

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

      <div v-if="status?.rollbackAvailable" class="actions">
        <button class="secondary" type="button" :disabled="busy" @click="restore">
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
