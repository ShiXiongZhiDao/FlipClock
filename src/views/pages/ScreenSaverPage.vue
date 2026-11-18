<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { computed, onMounted, ref } from "vue";

import { useSettings } from "../../composables/useSettings";
import { isTauri } from "../../lib/env";
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

const STATUS_TEXT: Record<RegistrationState, { label: string; tone: string }> = {
  notInstalled: { label: "未安装", tone: "idle" },
  installed: { label: "已安装", tone: "ok" },
  missing: { label: "文件缺失", tone: "err" },
  outdated: { label: "版本不匹配", tone: "warn" },
};

const statusInfo = computed(() =>
  status.value ? STATUS_TEXT[status.value.state] : null,
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
    notify("err", String(e));
  }
}

async function install() {
  busy.value = true;
  try {
    status.value = await invoke<ScreenSaverStatus>("install_screensaver");
    notify("ok", "屏保组件已就位，并已设为当前屏保");
  } catch (e) {
    notify("err", String(e));
  } finally {
    busy.value = false;
  }
}

async function restore() {
  busy.value = true;
  try {
    status.value = await invoke<ScreenSaverStatus>("restore_screensaver");
    notify("ok", "已恢复安装前的屏保设置");
  } catch (e) {
    notify("err", String(e));
  } finally {
    busy.value = false;
  }
}

async function openSystemSettings() {
  try {
    await invoke("open_windows_screensaver_settings");
  } catch (e) {
    notify("err", String(e));
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
        Windows 屏幕保护程序
        <span v-if="statusInfo" class="status" :class="statusInfo.tone">
          {{ statusInfo.label }}
        </span>
      </div>

      <div class="actions">
        <button class="primary" type="button" :disabled="busy" @click="install">
          {{ deployed ? "重新部署屏保组件" : "安装并设为当前屏保" }}
        </button>
        <button class="secondary" type="button" @click="openSystemSettings">
          打开 Windows 屏保设置
        </button>
        <button
          v-if="status?.rollbackAvailable"
          class="secondary"
          type="button"
          :disabled="busy"
          @click="restore"
        >
          恢复原屏保
        </button>
      </div>

      <Transition name="msg">
        <div v-if="message" class="msg" :class="message.tone">{{ message.text }}</div>
      </Transition>

      <div class="divider"></div>

      <div class="kv">
        <span>系统当前指向</span>
        <span class="kv-value">{{ status?.activePath || "（未设置，即屏保为「无」）" }}</span>
        <span>组件版本</span>
        <span class="kv-value">
          {{
            status?.state === "installed"
              ? "与当前程序一致"
              : status?.state === "outdated"
                ? "与当前程序不一致，建议重新部署"
                : "—"
          }}
        </span>
        <span>回滚点</span>
        <span class="kv-value">
          {{ status?.rollbackAvailable ? "已记录安装前的设置" : "无（安装前系统本就没有设置）" }}
        </span>
      </div>
    </section>

    <aside class="side">
      <div class="card">
        <div class="card-title">多显示器</div>
        <div class="segmented stretch">
          <button
            type="button"
            :class="{ selected: settings.screensaver.monitorMode === 'primary' }"
            @click="setMonitorMode('primary')"
          >
            主显示器
          </button>
          <button
            type="button"
            :class="{ selected: settings.screensaver.monitorMode === 'all' }"
            @click="setMonitorMode('all')"
          >
            所有显示器
          </button>
        </div>
      </div>

      <div class="card">
        <div class="card-title">启动</div>
        <div class="segmented stretch">
          <button
            type="button"
            :class="{ selected: settings.launch.noArgMode === 'settings' }"
            @click="setNoArgMode('settings')"
          >
            打开设置窗口
          </button>
          <button
            type="button"
            :class="{ selected: settings.launch.noArgMode === 'fullscreen' }"
            @click="setNoArgMode('fullscreen')"
          >
            直接全屏
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
