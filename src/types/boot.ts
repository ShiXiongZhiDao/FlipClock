/** Rust 在窗口创建时注入的启动载荷，见 src-tauri/src/screensaver.rs。 */
export interface BootPayload {
  mode: "settings" | "screensaver" | "preview" | "clock";
  exitOnInput: boolean;
  debugWindowed: boolean;
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
