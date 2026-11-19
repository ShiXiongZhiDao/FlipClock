<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { computed, onMounted, ref } from "vue";

import { useSettings } from "../../composables/useSettings";
import { isTauri } from "../../lib/env";
import { errorText, t, type MessageKey } from "../../lib/i18n";
import type {
  MonitorMode,
  NoArgMode,
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

function setNoArgMode(mode: NoArgMode) {
  settings.launch.noArgMode = mode;
}

onMounted(refresh);
</script>

<template>
  <div class="saver-page">
    <section class="card main">
      <div class="card-title">
        {{ t("saver.windowsTitle") }}
        <span v-if="statusInfo" class="status" :class="statusInfo.tone">
          {{ statusInfo.label }}
        </span>
      </div>

      <div class="actions">
        <button class="primary" type="button" :disabled="busy" @click="install">
          {{ t(deployed ? "saver.redeploy" : "saver.install") }}
        </button>
        <button class="secondary" type="button" @click="openSystemSettings">
          {{ t("saver.openSettings") }}
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

      <Transition name="msg">
        <div v-if="message" class="msg" :class="message.tone">{{ message.text }}</div>
      </Transition>

      <div class="divider"></div>

      <div class="kv">
        <span>{{ t("saver.currentTarget") }}</span>
        <span class="kv-value">{{ status?.activePath || t("saver.notSet") }}</span>
        <span>{{ t("saver.componentVersion") }}</span>
        <span class="kv-value">
          {{
            status?.state === "installed"
              ? t("saver.versionMatch")
              : status?.state === "outdated"
                ? t("saver.versionMismatch")
                : "—"
          }}
        </span>
        <span>{{ t("saver.rollbackPoint") }}</span>
        <span class="kv-value">
          {{ status?.rollbackAvailable ? t("saver.rollbackRecorded") : t("saver.rollbackNone") }}
        </span>
      </div>
    </section>

    <aside class="side">
      <div class="card">
        <div class="card-title">{{ t("saver.monitorTitle") }}</div>
        <div class="segmented stretch">
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
      </div>

      <div class="card">
        <div class="card-title">{{ t("common.startup") }}</div>
        <div class="segmented stretch">
          <button
            type="button"
            :class="{ selected: settings.launch.noArgMode === 'settings' }"
            @click="setNoArgMode('settings')"
          >
            {{ t("saver.launchSettings") }}
          </button>
          <button
            type="button"
            :class="{ selected: settings.launch.noArgMode === 'fullscreen' }"
            @click="setNoArgMode('fullscreen')"
          >
            {{ t("saver.launchFullscreen") }}
          </button>
        </div>
      </div>
    </aside>
  </div>
</template>

<style scoped>
.saver-page {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1.45fr) minmax(262px, 0.82fr);
  gap: 16px;
}

.main {
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--ui-scroll) transparent;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.kv {
  display: grid;
  grid-template-columns: 88px minmax(0, 1fr);
  gap: 7px 12px;
  font-size: 10px;
  color: var(--ui-text-muted);
}

.kv-value {
  color: var(--ui-text-secondary);
  word-break: break-all;
  font-family: var(--font-mono);
  font-size: 9.5px;
}

.msg {
  font-size: 10.5px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--ui-divider);
}

.msg.ok {
  color: var(--ui-success);
  border-color: rgba(132, 184, 153, 0.3);
  background: rgba(132, 184, 153, 0.08);
}

.msg.err {
  color: var(--ui-danger);
  border-color: rgba(217, 119, 119, 0.3);
  background: rgba(217, 119, 119, 0.08);
}

.side {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-height: 0;
}

/* 最后一张卡撑到底，右侧卡片底边才能和主卡片/侧栏对齐 */
.side .card:last-child {
  flex: 1;
}

.msg-enter-active,
.msg-leave-active {
  transition: opacity 0.18s ease;
}

.msg-enter-from,
.msg-leave-to {
  opacity: 0;
}
</style>
