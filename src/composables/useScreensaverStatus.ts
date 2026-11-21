import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";

import { trackFeatureToggled } from "../lib/analytics";
import { isTauri } from "../lib/env";
import { errorText } from "../lib/i18n";
import type { ScreenSaverStatus } from "../types/settings";

/**
 * 安装状态由设置窗口的两处同时读：左栏预览下的状态条与「屏保」页。
 * 全局只留一份，避免两处各自检测后显示不同的状态。
 */
const status = ref<ScreenSaverStatus | null>(null);
const statusError = ref<string | null>(null);
const busy = ref(false);
/** 本次设置窗口内刚装过；状态条据此多留一行「调整等待时间」。 */
const justInstalled = ref(false);

async function refresh(): Promise<void> {
  if (!isTauri()) return;
  try {
    status.value = await invoke<ScreenSaverStatus>("screensaver_status");
    statusError.value = null;
  } catch (e) {
    // 读不到就不要再拿上一次的「已就绪」糊弄人。
    status.value = null;
    statusError.value = errorText(e);
  }
}

async function install(): Promise<ScreenSaverStatus> {
  busy.value = true;
  try {
    const next = await invoke<ScreenSaverStatus>("install_screensaver");
    status.value = next;
    statusError.value = null;
    justInstalled.value = true;
    trackFeatureToggled("screensaver", true);
    return next;
  } finally {
    busy.value = false;
  }
}

async function restore(): Promise<ScreenSaverStatus> {
  busy.value = true;
  try {
    const next = await invoke<ScreenSaverStatus>("restore_screensaver");
    status.value = next;
    statusError.value = null;
    justInstalled.value = false;
    trackFeatureToggled("screensaver", false);
    return next;
  } finally {
    busy.value = false;
  }
}

function dismissInstalledHint(): void {
  justInstalled.value = false;
}

async function openWindowsSettings(): Promise<void> {
  await invoke("open_windows_screensaver_settings");
  // 用户已经去调等待时间了，刚装完的提示不必再占左栏。
  dismissInstalledHint();
}

export function useScreensaverStatus() {
  return {
    status,
    statusError,
    busy,
    justInstalled,
    dismissInstalledHint,
    refresh,
    install,
    restore,
    openWindowsSettings,
  };
}