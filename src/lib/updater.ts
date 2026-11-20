/**
 * 更新检查的状态机（ADR-0013）。
 *
 * 只做两件事：问「有没有新版」、把新版交给安装器。装完之后 Windows 上进程会被
 * 安装器结束——这是系统限制，不是这里能绕开的，界面必须提前说清楚。
 *
 * 状态放在模块级：系统页与以后可能出现的弹窗共用同一份事实，不会各查各的。
 */

import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { readonly, ref } from "vue";

import { trackUpdateChecked } from "./analytics";
import { isTauri } from "./env";

export type UpdaterState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "upToDate" }
  | { status: "available"; version: string; notes?: string }
  | { status: "downloading"; percent: number }
  | { status: "installing" }
  | { status: "error"; message: string };

const state = ref<UpdaterState>({ status: "idle" });
/** 主程序是不是住在安装目录里。便携版只提示下载，不给「安装」按钮。 */
const installed = ref<boolean | null>(null);

/** check() 的返回对象只在本次会话里有效，下载要复用同一份。 */
let pending: Update | null = null;
let autoCheckScheduled = false;

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useUpdater() {
  const busy = () =>
    state.value.status === "checking" ||
    state.value.status === "downloading" ||
    state.value.status === "installing";

  async function loadInstallKind(): Promise<boolean> {
    if (!isTauri()) return false;
    if (installed.value !== null) return installed.value;
    installed.value = await invoke<boolean>("is_installed_build").catch(() => false);
    return installed.value;
  }

  /**
   * `silent`：启动时的自动检查。失败就当没查过——离线、DNS 抽风都不该在
   * 「关于」里留一条红色报错，用户没主动问这件事。
   */
  async function checkForUpdate(options: { silent?: boolean } = {}): Promise<void> {
    if (!isTauri() || busy()) return;
    await loadInstallKind();

    state.value = { status: "checking" };
    try {
      const update = await check({ timeout: options.silent ? 15000 : 30000 });
      // 检查发生即记一次，不区分自动还是手动：事件描述的是动作本身。
      trackUpdateChecked();
      if (!update) {
        pending = null;
        state.value = { status: "upToDate" };
        return;
      }
      pending = update;
      state.value = {
        status: "available",
        version: update.version,
        notes: update.body?.trim() || undefined,
      };
    } catch (error) {
      pending = null;
      state.value = options.silent
        ? { status: "idle" }
        : { status: "error", message: messageOf(error) };
    }
  }

  /** 设置窗口出现 5 秒后再查：先把界面渲染完，别和启动抢带宽与 CPU。 */
  function scheduleAutoCheck(delayMs = 5000): void {
    if (!isTauri() || autoCheckScheduled) return;
    autoCheckScheduled = true;
    setTimeout(() => {
      // 用户在这 5 秒里已经手动查过，就不要覆盖他的结果。
      if (state.value.status === "idle") void checkForUpdate({ silent: true });
    }, delayMs);
  }

  async function installUpdate(): Promise<void> {
    if (!pending || state.value.status !== "available") return;
    if (!(await loadInstallKind())) return;

    const update = pending;
    try {
      state.value = { status: "downloading", percent: 0 };
      let downloaded = 0;
      let total = 0;
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            total = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            if (total > 0) {
              state.value = {
                status: "downloading",
                percent: Math.min(99, Math.floor((downloaded / total) * 100)),
              };
            }
            break;
          case "Finished":
            break;
        }
      });
      state.value = { status: "installing" };
      // Windows 上安装器会先结束本进程，这一行通常走不到；
      // 留着是为了「安装器被用户取消」时还能把界面拉回来。
      await relaunch().catch(() => undefined);
    } catch (error) {
      state.value = { status: "error", message: messageOf(error) };
    }
  }

  return {
    state: readonly(state),
    installed: readonly(installed),
    loadInstallKind,
    checkForUpdate,
    scheduleAutoCheck,
    installUpdate,
  };
}
