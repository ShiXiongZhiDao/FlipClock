/** Rust 在窗口创建时注入的启动载荷，见 src-tauri/src/screensaver.rs。 */
import type { Settings } from "./settings";

export interface BootPayload {
  mode: "settings" | "screensaver" | "preview" | "clock" | "theme-preview";
  exitOnInput: boolean;
  debugWindowed: boolean;
  /**
   * 配置快照：Rust 启动时已经读过一遍配置，顺手带给沉浸式窗口，
   * 让前端第一帧就能画用户选的主题（ADR-0030）。
   *
   * 只有屏保与全屏时钟有；设置窗口、主题预览窗口、浏览器开发模式都没有。
   */
  settings?: Settings;
  /**
   * 主题图片与导入字体的根目录，跟 `settings` 一起只给沉浸式窗口。
   * 有了它，`themeAssetUrl` 在第一帧就能拼出地址（ADR-0030 修订）。
   */
  assets?: {
    themesDir?: string | null;
    fontsDir?: string | null;
  };
  /**
   * 窗口在 webview 画出第一帧之前铺的底色（`#RRGGBB`）。沉浸式窗口与
   * 创建即可见的主题预览窗口有；设置窗口等 `ui_ready`，没有。
   * `index.html` 的 `<head>` 把它落成 `--fc-boot-bg`，见 ADR-0031。
   */
  windowBackground?: string;
  /**
   * 当前主题背景图的 160px 缩略图（`data:image/jpeg;base64,…`）。
   * 有它时页面底色铺这张图而不是纯色，见 `theme_preview.rs` 与 ADR-0031 修订。
   */
  bootBackgroundImage?: string;
}

declare global {
  interface Window {
    __FLIPCLOCK_BOOT__?: BootPayload;
  }
}

const FALLBACK: BootPayload = {
  mode: "settings",
  exitOnInput: false,
  debugWindowed: false,
};

export function readBoot(): BootPayload {
  const injected = window.__FLIPCLOCK_BOOT__;
  if (injected) return { ...FALLBACK, ...injected };

  // 仅开发期：在浏览器里直接开页面核对版式时才看 URL。
  // 打包后 import.meta.env.DEV 为 false，这段会被整体裁掉，启动形态只认 Rust 注入的载荷。
  if (import.meta.env.DEV) {
    const mode = new URLSearchParams(window.location.search).get("mode");
    if (mode === "screensaver" || mode === "preview" || mode === "clock") {
      return { ...FALLBACK, mode };
    }
    return FALLBACK;
  }

  return FALLBACK;
}
