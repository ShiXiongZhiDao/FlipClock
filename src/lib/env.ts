/** 判断是否跑在 Tauri 里。浏览器里直接开页面看版式时，系统能力一律跳过。 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
