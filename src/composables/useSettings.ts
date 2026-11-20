import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { reactive, ref, watch } from "vue";

import { trackFeatureToggled } from "../lib/analytics";
import { cloneJson } from "../lib/clone";
import { initThemeAssets } from "../lib/theme/assets";
import { isTauri } from "../lib/env";
import { applyLanguage } from "../lib/i18n";
import { ensureProfiles, profileOf } from "../lib/theme/profile";
import { settingsPatchOf } from "../lib/settingsPatch";
import {
  BRIGHTNESS_MAX,
  BRIGHTNESS_MIN,
  CLOCK_SCALE_MAX,
  CLOCK_SCALE_MIN,
  defaultSettings,
  SOUND_VOLUME_MAX,
  SOUND_VOLUME_MIN,
  type ClockProfile,
  type DigitFont,
  type Settings,
  type SettingsPatch,
} from "../types/settings";

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

/** 只接受本版本认识的字体；旧配置或手改文件带来的未知值一律落回 Bebas。 */
function readFont(value: unknown): DigitFont {
  return value === "bahnschrift" || value === "system" ? value : "bebas";
}

function clampProfile(profile: ClockProfile): void {
  profile.clockScale = clamp(profile.clockScale, CLOCK_SCALE_MIN, CLOCK_SCALE_MAX);
  profile.brightness = clamp(profile.brightness, BRIGHTNESS_MIN, BRIGHTNESS_MAX);
  profile.sound.volume = clamp(profile.sound.volume, SOUND_VOLUME_MIN, SOUND_VOLUME_MAX);
}

/**
 * 全局只有一份设置状态。
 * 侧边栏里的每个页面都读同一份，改任何一页都会即时反映到预览，也只写一次盘。
 */
const settings = reactive<Settings>(defaultSettings());
const loaded = ref(false);
const error = ref<string | null>(null);

let loading: Promise<void> | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;
/** 读盘/重载期间挂起防抖保存，避免把补齐的默认值写回磁盘。 */
let hydrating = false;

function beginHydration(): void {
  clearTimeout(timer);
  hydrating = true;
}

function endHydration(): void {
  hydrating = false;
}

function patchOf(): SettingsPatch {
  return settingsPatchOf(settings);
}

async function flush() {
  // 浏览器里只有内存里的状态，没有可写的目标；不去碰不存在的后端。
  if (!loaded.value || !isTauri()) return;
  try {
    await invoke("save_settings", { patch: patchOf() });
    error.value = null;
  } catch (e) {
    error.value = String(e);
  }
}

watch(
  settings,
  () => {
    // 读盘/重载期间补的默认值只留在内存（ADR-0022：缺失按默认运行，不静默写回）。
    if (!loaded.value || hydrating) return;
    clearTimeout(timer);
    timer = setTimeout(flush, 200);
  },
  { deep: true },
);

// 语言偏好独立监听：切换界面文字要比 200ms 的落盘防抖更早，
// 而且托盘重建走的是另一条命令，不跟着 save_settings 走。
let languageBaseline: string | null = null;
watch(
  () => settings.ui.language,
  (language) => {
    applyLanguage(language);
    // watch 回调在微任务里跑，可能晚于 load() 里设置基线。只比较值是否真的
    // 偏离基线，不用布尔标志——否则「加载配置」会被误记成「用户切换语言」。
    if (languageBaseline !== null && language !== languageBaseline) {
      trackFeatureToggled("language", language !== "system");
      languageBaseline = language;
    }
  },
  { immediate: true },
);

/**
 * 把远端的外观与每个主题的钟面配置合并进来。
 *
 * load 与 reload 必须用同一套缺省与钳制：预览窗口改完主题回到设置页时，
 * 两条路径如果各写一份，迟早会漂移成「重载后某个值多/少钳了一次」。
 */
function applyRemoteAppearance(remote: Settings): void {
  const appearance = remote.appearance;
  settings.appearance = {
    theme: appearance?.theme ?? settings.appearance.theme,
    customThemes: cloneJson(appearance?.customThemes ?? []),
    profiles: cloneJson(appearance?.profiles ?? {}),
  };
  // 配置表与主题表必须一一对应；再兜一次边界，手改配置也不会带进越界值。
  ensureProfiles(settings);
  for (const profile of Object.values(settings.appearance.profiles)) {
    clampProfile(profile);
  }
}

function applyRemoteUi(remote: Settings): void {
  const language = remote.ui?.language;
  settings.ui = {
    language: language === "zh" || language === "en" || language === "system" ? language : "system",
  };
}

/**
 * 预览窗口关闭后重新拉一次配置。
 *
 * 预览窗口是独立的 WebView，它保存的主题与配置主窗口不会自动知道；
 * Rust 在关窗时发 `theme-preview-closed`，这里接住再读一遍。
 */
async function reload(): Promise<void> {
  if (!isTauri()) return;
  try {
    const remote = await invoke<Settings>("get_settings");
    beginHydration();
    try {
      applyRemoteAppearance(remote);
      // 语言也可能在全屏配置里改过：这一步会同时更新本窗口文字与托盘。
      applyRemoteUi(remote);
    } finally {
      endHydration();
    }
  } catch (error) {
    console.warn("reload settings failed", error);
  }
}

let previewWired = false;
function wirePreviewSync(): void {
  if (previewWired || !isTauri()) return;
  previewWired = true;
  void listen("theme-preview-closed", () => {
    void reload();
  });
}

async function load() {
  if (loaded.value) return;
  // 主题图片的根目录路径要异步取。不 await：路径到位后 base 这个 ref 会触发
  // 模板重算，图片自然补上，不必挡住窗口出现。
  void initThemeAssets();
  if (!isTauri()) {
    // 仅开发期：在浏览器里核对另一种排布的几何时才看 URL，
    // 打包后 import.meta.env.DEV 为 false，这段整体被裁掉。
    if (import.meta.env.DEV) {
      const profile = profileOf(settings);
      const params = new URLSearchParams(window.location.search);
      if (params.get("layout") === "per-pair") profile.digitLayout = "per-pair";
      if (params.get("font")) profile.font = readFont(params.get("font"));
      if (params.get("colon") === "1") profile.showColon = true;
      const brightness = Number(params.get("brightness"));
      if (params.get("brightness") && Number.isFinite(brightness)) {
        profile.brightness = clamp(brightness, BRIGHTNESS_MIN, BRIGHTNESS_MAX);
      }
    }
    loaded.value = true;
    return;
  }
  void wirePreviewSync();
  loading ??= (async () => {
    try {
      const remote = await invoke<Settings>("get_settings");
      beginHydration();
      try {
        settings.schemaVersion = remote.schemaVersion ?? 3;
        applyRemoteAppearance(remote);
        settings.system = {
          closeToTray: remote.system?.closeToTray ?? true,
        };
        settings.screensaver = remote.screensaver ?? settings.screensaver;
        settings.launch = remote.launch ?? settings.launch;
        applyRemoteUi(remote);
        settings.registration = remote.registration;
        loaded.value = true;
        // settings.ui 的赋值已触发 watch，但回调在微任务里才跑。先把基线设成
        // 刚加载进来的值，回调读到相等就不会把「加载配置」误报成用户切换。
        languageBaseline = settings.ui.language;
      } finally {
        endHydration();
      }
    } catch (e) {
      error.value = String(e);
    }
  })();
  await loading;
}

/**
 * 音效开关。关掉时保留音量，重新打开时若音量为 0 就补一个能听见的默认值。
 * 快速设置与「声音」页共用这一份逻辑，避免两处开关行为漂移。
 */
function toggleSound() {
  const profile = profileOf(settings);
  profile.sound.enabled = !profile.sound.enabled;
  if (profile.sound.enabled && profile.sound.volume <= 0) {
    profile.sound.volume = 0.6;
  }
  trackFeatureToggled("sound", profile.sound.enabled);
}

/**
 * 只重置用户可改的几段，不动注册信息与回滚点。
 *
 * 删主题必须连带删图片目录（ADR-0020 D15）。reset 会把 customThemes 整个换掉，
 * 不删的话 `<app_config>/themes/<id>/` 就永远留在磁盘上——而系统页的确认弹窗
 * 明确承诺「当前主题与全部自建主题」会被清除，弹窗说的和做的必须是同一件事。
 *
 * 删除失败不让整体失败：配置已经换掉了，回滚没有意义；把没删掉的 id 返回给调用方，
 * 由界面报一条「有残留」，比假装清干净更诚实。
 */
async function reset(): Promise<string[]> {
  const ids = new Set(settings.appearance.customThemes.map((theme) => theme.id));
  // 当前主题可能已经不在 customThemes 里（悬空 id），它的目录同样要清。
  ids.add(settings.appearance.theme);

  const base = defaultSettings();
  settings.appearance = base.appearance;
  settings.screensaver = base.screensaver;
  settings.launch = base.launch;
  settings.system = base.system;
  settings.ui = base.ui;

  const failed: string[] = [];
  await Promise.all(
    [...ids].map(async (themeId) => {
      try {
        await invoke("delete_theme_dir", { themeId });
      } catch (e) {
        // 内置主题没有目录，delete_theme_dir 对不存在的目录返回 Ok；
        // 走到这里说明真的删不掉（占用、权限），必须留痕。
        console.warn("delete theme dir failed", themeId, e);
        failed.push(themeId);
      }
    }),
  );
  return failed;
}

export function useSettings() {
  return { settings, loaded, error, load, flush, reset, toggleSound };
}
