<script setup lang="ts">
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";

import { useSettings } from "../../composables/useSettings";
import { isTauri } from "../../lib/env";
import { useUpdater } from "../../lib/updater";
import type { ScreenSaverStatus } from "../../types/settings";

const { settings } = useSettings();

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
    notify("ok", next ? "已加入开机启动" : "已从开机启动移除");
  } catch (error) {
    // 写注册表失败（组策略、权限）时要让用户看见原因，开关保持原状。
    notify("err", String(error));
    await refreshAutostart();
  } finally {
    busy.value = false;
  }
}

/** 术语表里的四种注册状态一个都不能少：二分成「已装/未装」会把过期说成没装。 */
const SAVER_LABEL: Record<string, string> = {
  notInstalled: "未安装",
  installed: "已安装",
  missing: "文件缺失",
  outdated: "版本不匹配",
};

const saverLabel = computed(() =>
  saver.value ? (SAVER_LABEL[saver.value.state] ?? saver.value.state) : "—",
);
const saverNeedsInstall = computed(
  () => saver.value?.state === "outdated" || saver.value?.state === "missing",
);

async function reinstallSaver() {
  if (!isTauri() || busy.value) return;
  busy.value = true;
  try {
    saver.value = await invoke<ScreenSaverStatus>("install_screensaver");
    notify("ok", "屏保组件已更新为当前版本");
  } catch (error) {
    notify("err", String(error));
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
      return "正在检查更新…";
    case "upToDate":
      return "已是最新版本";
    case "available":
      return `有新版本 v${updateState.value.version}`;
    case "downloading":
      return `正在下载 ${updateState.value.percent}%`;
    case "installing":
      return "正在安装，应用会退出并重新启动";
    case "error":
      return "检查更新失败";
    default:
      return "更新";
  }
});

const updateHint = computed(() => {
  switch (updateState.value.status) {
    case "available":
      return installedBuild.value === false
        ? "当前是便携版：自动安装只能替换安装版，请到 Release 页下载新包。"
        : "下载完成后应用会短暂退出，由安装器替换文件。";
    case "installing":
      return "安装器接管后会自动把 FlipClock 拉起来。";
    case "error":
      return updateState.value.message;
    case "idle":
      return installedBuild.value === false ? "便携版可以检查，但不能就地安装。" : "";
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
      <div class="card-title">启动</div>

      <div class="row">
        <div class="row-copy">
          <span class="row-name">开机自启</span>
          <span class="row-hint">登录 Windows 后驻留托盘，不弹窗口</span>
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
          <span class="row-name">关闭窗口时最小化到托盘</span>
          <span class="row-hint">关掉后点关闭按钮就退出整个程序</span>
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
      <div class="card-title">关于</div>
      <div class="info-row">
        <span>版本</span>
        <strong>{{ version || "—" }}</strong>
      </div>
      <div class="info-row">
        <span>屏保组件</span>
        <span class="info-state">
          <strong :class="{ warn: saverNeedsInstall }">{{ saverLabel }}</strong>
          <button
            v-if="saverNeedsInstall"
            type="button"
            class="link-button"
            :disabled="busy"
            @click="reinstallSaver"
          >
            重新安装
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
          {{ updateState.status === "error" ? "重试" : "检查更新" }}
        </button>
        <button
          v-else-if="updateAction === 'install'"
          type="button"
          class="ghost-button accent"
          :disabled="updateBusy"
          @click="installUpdate"
        >
          下载并安装
        </button>
      </div>
    </section>
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
</style>
