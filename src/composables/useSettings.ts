import { invoke } from "@tauri-apps/api/core";
import { reactive, ref, watch } from "vue";

import { isTauri } from "../lib/env";
import {
  BRIGHTNESS_MAX,
  BRIGHTNESS_MIN,
  CLOCK_SCALE_MAX,
  CLOCK_SCALE_MIN,
  defaultSettings,
  GLASS_TRANSPARENCY_MAX,
  GLASS_TRANSPARENCY_MIN,
  SOUND_VOLUME_MAX,
  SOUND_VOLUME_MIN,
  type DigitFont,
  type Settings,
  type SettingsPatch,
} from "../types/settings";

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

/** 只接受本版本认识的字体；旧配置或手改文件带来的未知值一律落回 Bebas。 */
function readFont(value: unknown): DigitFont {
  return value === "bahnschrift" || value === "system" ? value : "bebas";
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

function patchOf(): SettingsPatch {
  return {
    appearance: {
      theme: settings.appearance.theme,
      glass: { ...settings.appearance.glass },
      clockScale: settings.appearance.clockScale,
      digitLayout: settings.appearance.digitLayout,
      brightness: settings.appearance.brightness,
      font: settings.appearance.font,
    },
    time: { ...settings.time },
    sound: { ...settings.sound },
    screensaver: { ...settings.screensaver },
    launch: { ...settings.launch },
  };
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
    if (!loaded.value) return;
    clearTimeout(timer);
    timer = setTimeout(flush, 200);
  },
  { deep: true },
);

async function load() {
  if (loaded.value) return;
  if (!isTauri()) {
    // 仅开发期：在浏览器里核对另一种排布的几何时才看 URL，
    // 打包后 import.meta.env.DEV 为 false，这段整体被裁掉。
    if (import.meta.env.DEV) {
      const params = new URLSearchParams(window.location.search);
      if (params.get("layout") === "per-pair") settings.appearance.digitLayout = "per-pair";
      if (params.get("font")) settings.appearance.font = readFont(params.get("font"));
      if (params.get("colon") === "1") settings.time.showColon = true;
      const brightness = Number(params.get("brightness"));
      if (params.get("brightness") && Number.isFinite(brightness)) {
        settings.appearance.brightness = clamp(brightness, BRIGHTNESS_MIN, BRIGHTNESS_MAX);
      }
    }
    loaded.value = true;
    return;
  }
  loading ??= (async () => {
    try {
      const remote = await invoke<Settings>("get_settings");
      settings.schemaVersion = remote.schemaVersion ?? 1;
      // 旧版本配置没有 appearance.glass / clockScale，缺哪段就补默认，
      // 别让整块外观变 undefined。
      settings.appearance = {
        theme: remote.appearance?.theme ?? settings.appearance.theme,
        glass: {
          transparency: clamp(
            remote.appearance?.glass?.transparency ?? settings.appearance.glass.transparency,
            GLASS_TRANSPARENCY_MIN,
            GLASS_TRANSPARENCY_MAX,
          ),
        },
        clockScale: clamp(
          remote.appearance?.clockScale ?? settings.appearance.clockScale,
          CLOCK_SCALE_MIN,
          CLOCK_SCALE_MAX,
        ),
        // 后端只会给出这两个值；这里仍显式判一次，避免旧配置或手改文件带进未知 id。
        digitLayout:
          remote.appearance?.digitLayout === "per-pair" ? "per-pair" : "per-digit",
        brightness: clamp(
          remote.appearance?.brightness ?? settings.appearance.brightness,
          BRIGHTNESS_MIN,
          BRIGHTNESS_MAX,
        ),
        font: readFont(remote.appearance?.font),
      };
      settings.time = {
        format: remote.time?.format ?? settings.time.format,
        showDate: remote.time?.showDate ?? settings.time.showDate,
        showSeconds: remote.time?.showSeconds ?? settings.time.showSeconds,
        showColon: remote.time?.showColon ?? false,
      };
      settings.sound = {
        enabled: remote.sound?.enabled ?? settings.sound.enabled,
        volume: clamp(
          remote.sound?.volume ?? settings.sound.volume,
          SOUND_VOLUME_MIN,
          SOUND_VOLUME_MAX,
        ),
      };
      settings.screensaver = remote.screensaver ?? settings.screensaver;
      settings.launch = remote.launch ?? settings.launch;
      settings.registration = remote.registration;
      loaded.value = true;
    } catch (e) {
      error.value = String(e);
    }
  })();
  await loading;
}

/** 只重置用户可改的几段，不动注册信息与回滚点。 */
function reset() {
  const base = defaultSettings();
  settings.appearance = base.appearance;
  settings.time = base.time;
  settings.sound = base.sound;
  settings.screensaver = base.screensaver;
  settings.launch = base.launch;
}

export function useSettings() {
  return { settings, loaded, error, load, flush, reset };
}
