/**
 * 导入字体的加载、测量与回退（ADR-0026）。
 *
 * 字体文件存在 `<app_config>/fonts/`，通过 asset protocol 读成 ArrayBuffer，
 * 再交给 FontFace——不依赖 MIME，也不给 WebView 任意文件路径。
 * 测量只对导入字体做：内置字体的字族与比例来自 fontRegistry。
 */
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { reactive, ref } from "vue";

import { isTauri } from "./env";
import {
  BUILTIN_FONT_DEFINITIONS,
  isBuiltinFontId,
  type ClockFontDefinition,
  type ClockFontMetrics,
} from "./fontRegistry";
import { FONT_NAME_MAX, type CustomFont } from "../types/settings";

export type FontStatus = "loading" | "loaded" | "failed";

export interface FontMetrics extends ClockFontMetrics {
  /** 0–9 的墨迹中心差超过阈值（旧式数字），样卡需要提示。 */
  uneven: boolean;
}

const base = ref<string | null>(null);
let ready: Promise<void> | null = null;

/** 当前设置里的字体库快照；由 useSettings / 全屏配置窗口调用 setFontLibrary 更新。 */
const library = ref<CustomFont[]>([]);
const status = reactive<Record<string, FontStatus>>({});
const metrics = reactive<Record<string, FontMetrics>>({});
const faces = new Map<string, FontFace>();
const pending = new Map<string, Promise<boolean>>();
const urlCache = new Map<string, string>();

const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

export function initFontAssets(): Promise<void> {
  ready ??= (async () => {
    if (!isTauri()) return;
    base.value = await invoke<string>("fonts_dir_path").catch(() => null);
  })();
  return ready;
}

export function fontAssetUrl(entry: CustomFont): string | null {
  const root = base.value;
  if (!root) return null;
  const hit = urlCache.get(entry.file);
  if (hit) return hit;
  // asset:// 走 URL 语义；Windows 的 PathBuf 反斜杠要统一成正斜杠。
  const url = convertFileSrc(`${root.replace(/\\/g, "/")}/${entry.file}`);
  urlCache.set(entry.file, url);
  return url;
}

export function customFontFamily(id: string): string {
  // 加前缀避免与系统字体/内置字体重名；字体内部 family name 不被修改。
  return `FC-${id}`;
}

export function getFontEntry(id: string): CustomFont | undefined {
  return library.value.find((font) => font.id === id);
}

export function fontState(id: string): "builtin" | "missing" | "idle" | FontStatus {
  if (isBuiltinFontId(id)) return "builtin";
  if (!getFontEntry(id)) return "missing";
  return status[id] ?? "idle";
}

export function fontMetrics(id: string): FontMetrics | undefined {
  return metrics[id];
}

/** 设置里字体库变化时同步快照，并清掉已删除字体的状态与已注册的 FontFace。 */
export function setFontLibrary(fonts: CustomFont[]): void {
  library.value = fonts.map((font) => ({ ...font }));
  const ids = new Set(library.value.map((font) => font.id));
  for (const id of Object.keys(status)) {
    if (!ids.has(id)) {
      delete status[id];
      delete metrics[id];
    }
  }
  for (const [key, face] of faces) {
    const id = key.slice(0, key.indexOf("/"));
    if (!ids.has(id)) {
      document.fonts.delete(face);
      faces.delete(key);
    }
  }
}

export function forgetFont(id: string): void {
  const entry = getFontEntry(id);
  if (entry) {
    const key = `${id}/${entry.file}`;
    const face = faces.get(key);
    if (face) {
      document.fonts.delete(face);
      faces.delete(key);
    }
  }
  delete status[id];
  delete metrics[id];
}

/**
 * 加载并测量一款导入字体。同一 id 的并发调用共享一个 Promise；
 * 加载完成后才写 loaded，钟面在此之前继续用 Bebas。
 */
export function ensureFontLoaded(entry: CustomFont): Promise<boolean> {
  const id = entry.id;
  if (status[id] === "loaded") return Promise.resolve(true);
  if (status[id] === "failed") return Promise.resolve(false);

  const key = `${id}/${entry.file}`;
  const existing = pending.get(key);
  if (existing) return existing;

  const task = (async () => {
    status[id] = "loading";
    try {
      await initFontAssets();
      const url = fontAssetUrl(entry);
      if (!url) throw new Error("font base unavailable");
      const response = await fetch(url);
      if (!response.ok) throw new Error(`font fetch ${response.status}`);
      const buffer = await response.arrayBuffer();
      const face = new FontFace(customFontFamily(id), buffer, {
        style: "normal",
        weight: "400",
        display: "block",
      });
      await face.load();
      // 加载期间字体可能已被删除：不要把已删字体的 face 留在 document.fonts 里。
      if (!getFontEntry(id)) {
        throw new Error("font removed while loading");
      }
      document.fonts.add(face);
      faces.set(key, face);
      metrics[id] = measureFont(id);
      status[id] = "loaded";
      return true;
    } catch (error) {
      console.warn("font load failed", id, error);
      status[id] = "failed";
      return false;
    } finally {
      pending.delete(key);
    }
  })();

  pending.set(key, task);
  return task;
}

function customFontDefinition(
  entry: CustomFont,
  measured?: FontMetrics,
): ClockFontDefinition {
  const family = customFontFamily(entry.id);
  return {
    id: entry.id,
    name: entry.name,
    family,
    stack: `"${family}", "Bebas Neue", "Segoe UI", sans-serif`,
    source: "imported",
    group: "imported",
    metrics: measured,
  };
}

function varsFromDefinition(def: ClockFontDefinition): Record<string, string> {
  const measured = def.metrics ?? BUILTIN_FONT_DEFINITIONS.bebas.metrics;
  return {
    "--font-clock": def.stack,
    "--font-clock-face": `"${def.family}"`,
    "--font-scale-single": String(measured.singleScale),
    "--font-scale-pair": String(measured.pairScale),
    "--digit-weight": String(measured.weight),
    "--digit-spacing": measured.spacing,
    "--digit-shift": `${measured.shiftEm}em`,
    "--font-text-weight": String(measured.textWeight),
  };
}

/** 钟面实际使用的字体 id：导入字体未加载/失败/缺失时先按 Bebas 渲染。 */
export function effectiveFontId(id: string): string {
  if (isBuiltinFontId(id)) return id;
  const entry = getFontEntry(id);
  return entry && status[id] === "loaded" ? id : "bebas";
}

/** 钟面 CSS 变量：内置从注册表取，导入从运行时测量取；fallback 是 Bebas。 */
export function fontStyleVars(id: string): Record<string, string> {
  const effective = effectiveFontId(id);
  if (isBuiltinFontId(effective)) {
    return varsFromDefinition(BUILTIN_FONT_DEFINITIONS[effective]);
  }
  const entry = getFontEntry(id);
  const measured = metrics[id];
  if (!entry || status[id] !== "loaded" || !measured) {
    return varsFromDefinition(BUILTIN_FONT_DEFINITIONS.bebas);
  }
  return varsFromDefinition(customFontDefinition(entry, measured));
}

/** 样卡用的字体样式；导入字体加载完成前显示 fallback，不冒充已加载。 */
export function fontSampleStyle(id: string): Record<string, string> {
  if (isBuiltinFontId(id)) {
    const def = BUILTIN_FONT_DEFINITIONS[id];
    return {
      fontFamily: def.stack,
      fontWeight: String(def.metrics.weight),
      letterSpacing: def.metrics.spacing,
    };
  }
  const measured = metrics[id];
  if (status[id] !== "loaded" || !measured) return {};
  const entry = getFontEntry(id);
  if (!entry) return {};
  return {
    fontFamily: customFontDefinition(entry, measured).stack,
    fontWeight: String(measured.weight),
    letterSpacing: measured.spacing,
  };
}

export function uniqueFontName(
  baseName: string,
  taken: string[],
  fallback: string,
): string {
  const trimmed = baseName.trim() || fallback;
  const stem: string = [...trimmed].slice(0, FONT_NAME_MAX).join("");
  if (!taken.includes(stem)) return stem;
  for (let index = 2; index < 1000; index += 1) {
    const suffix = ` ${index}`;
    const keep = FONT_NAME_MAX - suffix.length;
    const candidate = `${[...stem].slice(0, keep).join("")}${suffix}`;
    if (!taken.includes(candidate)) return candidate;
  }
  return stem;
}

/**
 * 在 100px 下量 0–9：一位一卡只在溢出时缩小，两位一卡按最宽数字对填到 86% 卡宽。
 * 数值与 FlipClock.vue 的卡片几何对应：单卡 16.5×23.2、字号 18.8；
 * 双位卡 33×33、字号 34.5（单位都是 --unit）。
 */
function measureFont(id: string): FontMetrics {
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    return {
      singleScale: 1,
      pairScale: 1,
      shiftEm: 0,
      weight: 400,
      spacing: "0",
      textWeight: 400,
      uneven: false,
    };
  }
  ctx.font = `100px "${customFontFamily(id)}"`;

  let maxDigit = 0;
  let unionTop = 0;
  let unionBottom = 0;
  const centers: number[] = [];
  for (const digit of "0123456789") {
    const m = ctx.measureText(digit);
    maxDigit = Math.max(maxDigit, m.width);
    unionTop = Math.max(unionTop, m.actualBoundingBoxAscent);
    unionBottom = Math.max(unionBottom, m.actualBoundingBoxDescent);
    centers.push((m.actualBoundingBoxAscent - m.actualBoundingBoxDescent) / 2);
  }

  let maxPair = 0;
  for (let tens = 0; tens < 10; tens += 1) {
    for (let ones = 0; ones < 10; ones += 1) {
      maxPair = Math.max(maxPair, ctx.measureText(`${tens}${ones}`).width);
    }
  }

  const inkHeight = Math.max(unionTop + unionBottom, 1);
  const singleWidthLimit = (0.9 * 16.5 * 100) / (18.8 * Math.max(maxDigit, 1));
  const singleHeightLimit = (0.9 * 23.2 * 100) / (18.8 * inkHeight);
  const singleScale = clamp(Math.min(1, singleWidthLimit, singleHeightLimit), 0.4, 1);

  const pairWidthScale = (0.86 * 33 * 100) / (34.5 * Math.max(maxPair, 1));
  const pairHeightScale = (0.9 * 33 * 100) / (34.5 * inkHeight);
  const pairScale = clamp(Math.min(pairWidthScale, pairHeightScale), 0.4, 1.4);

  const lineBox = ctx.measureText("0");
  const lineCenter =
    (lineBox.fontBoundingBoxAscent - lineBox.fontBoundingBoxDescent) / 2;
  const inkCenter = (unionTop - unionBottom) / 2;
  const shiftEm = clamp((inkCenter - lineCenter) / 100, -0.2, 0.2);
  const spread = Math.max(...centers) - Math.min(...centers);

  return {
    singleScale,
    pairScale,
    shiftEm,
    weight: 400,
    spacing: "0",
    textWeight: 400,
    uneven: spread > 6,
  };
}