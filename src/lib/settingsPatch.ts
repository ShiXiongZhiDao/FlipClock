/**
 * 发给后端的整份配置补丁。
 *
 * 主窗口的防抖保存与全屏配置的「保存」必须用同一份构造逻辑，
 * 否则两条写入路径迟早会在某个字段上漂移。
 */
import { cloneJson } from "./clone";
import type { Settings, SettingsPatch } from "../types/settings";

export function settingsPatchOf(settings: Settings): SettingsPatch {
  return {
    appearance: {
      theme: settings.appearance.theme,
      // 深拷贝：patch 是发给后端的快照，不能继续指向响应式对象。
      customThemes: cloneJson(settings.appearance.customThemes),
      fonts: cloneJson(settings.appearance.fonts),
      profiles: cloneJson(settings.appearance.profiles),
    },
    screensaver: { ...settings.screensaver },
    launch: { ...settings.launch },
    system: { ...settings.system },
    ui: { ...settings.ui },
  };
}
