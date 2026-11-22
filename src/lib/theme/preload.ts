/**
 * 首帧资产预热（ADR-0030 修订）。
 *
 * 主题图片与导入字体都不是同步可得的：图片要拼出 `asset://` 地址再解码，
 * 导入字体要读文件、注册 FontFace、量一遍字宽。以前这些都在窗口显示之后才做，
 * 于是首帧画的是「纯色背景 + Bebas 回退字体」，等资产到位再跳一次——
 * 自定义主题的底色如果接近经典碳灰，看起来就是「先闪了一下默认主题」。
 *
 * 屏保与全屏时钟因此在 `mount()` 之前等这一步：图片解码完、字体装进
 * `document.fonts`，第一帧就是最终画面。
 *
 * 为什么不用「等资产好了再显示窗口」：窗口在 webview 起来之前就 show 了，
 * 藏内容只会得到一块黑屏，还得再养一条超时兜底的失败路径。这里只推迟挂载，
 * 窗口该亮就亮。
 */
import { useTheme } from "../../composables/useTheme";
import { ensureFontLoaded, getFontEntry } from "../fonts";

/** 坏图或坏字体最多拖住钟面这么久；超时就带着回退先画，不能停在黑屏上。 */
const PRELOAD_TIMEOUT_MS = 1200;

function preloadImage(url: string): Promise<void> {
  return new Promise((resolve) => {
    const image = new Image();
    // 失败也算完成：一张坏图不该拖住屏保，主题底色照常显示。
    image.onload = () => void image.decode().then(resolve, resolve);
    image.onerror = () => resolve();
    image.src = url;
  });
}

export async function preloadClockAssets(): Promise<void> {
  const { backgroundUrl, digitUrl, profile } = useTheme();

  const jobs: Promise<unknown>[] = [];
  for (const url of [backgroundUrl.value, digitUrl.value]) {
    if (url) jobs.push(preloadImage(url));
  }
  const font = getFontEntry(profile.value.font);
  if (font) jobs.push(ensureFontLoaded(font));
  if (jobs.length === 0) return;

  let timedOut = false;
  const timeout = new Promise<void>((resolve) => {
    window.setTimeout(() => {
      timedOut = true;
      resolve();
    }, PRELOAD_TIMEOUT_MS);
  });
  await Promise.race([Promise.allSettled(jobs).then(() => undefined), timeout]);
  if (timedOut) {
    console.warn("clock assets not ready in time; painting with fallbacks");
  }
}
