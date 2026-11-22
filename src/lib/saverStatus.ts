import type { MessageKey } from "./i18n";
import type { RegistrationState, ScreenSaverStatus } from "../types/settings";

/** 界面上的屏保状态：注册状态之外，多一个「已安装且 Windows 开关打开」的就绪态。 */
export type SaverUiState = RegistrationState | "ready";

interface SaverUiMeta {
  pageLabel: MessageKey;
  tone: "idle" | "ok" | "warn" | "err";
  /** 就绪态不显示标题，只留刚安装后的等待时间提示。 */
  stripTitle: MessageKey | null;
  stripAction: MessageKey | null;
  pageAction: MessageKey | null;
  note: MessageKey | null;
}

/** 状态条与屏保页共用同一张表，避免两处各自维护 switch 后文案漂移。 */
export const SAVER_STATE_UI: Record<SaverUiState, SaverUiMeta> = {
  notInstalled: {
    pageLabel: "saver.status.notInstalled",
    tone: "idle",
    stripTitle: "saver.strip.notInstalled",
    stripAction: "saver.strip.install",
    pageAction: "saver.installShort",
    note: "saver.strip.notInstalledNote",
  },
  installed: {
    pageLabel: "saver.status.installed",
    tone: "idle",
    stripTitle: "saver.strip.installed",
    stripAction: null,
    pageAction: null,
    note: null,
  },
  ready: {
    pageLabel: "saver.status.ready",
    tone: "ok",
    stripTitle: null,
    stripAction: null,
    pageAction: null,
    note: null,
  },
  missing: {
    pageLabel: "saver.status.missing",
    tone: "err",
    stripTitle: "saver.strip.missing",
    stripAction: "saver.strip.repair",
    pageAction: "saver.repairShort",
    note: null,
  },
  outdated: {
    pageLabel: "saver.status.outdated",
    tone: "warn",
    stripTitle: "saver.strip.outdated",
    stripAction: "saver.strip.update",
    pageAction: "saver.updateShort",
    note: null,
  },
};

export function saverUiState(status: ScreenSaverStatus | null): SaverUiState | null {
  if (!status) return null;
  return status.state === "installed" && status.windowsEnabled ? "ready" : status.state;
}

export function saverWindowsOff(status: ScreenSaverStatus | null): boolean {
  return Boolean(status && status.state !== "notInstalled" && status.windowsEnabled === false);
}