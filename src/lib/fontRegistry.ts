/**
 * 钟面字体注册表（ADR-0026 D19）。
 *
 * UI 选项、CSS 变量、内置字号/字重/偏移都从这里取；导入字体由 fonts.ts
 * 在运行时补上同形状的 definition。内置数值来自原 fonts.css 的手写标定，
 * 迁移后 fonts.css 不再定义这些变量。
 */
import type { MessageKey } from "./i18n";
import type { DigitFont } from "../types/settings";

export interface ClockFontMetrics {
  singleScale: number;
  pairScale: number;
  shiftEm: number;
  weight: number;
  spacing: string;
  textWeight: number;
  /** 0–9 墨迹中心差超过阈值（旧式数字）。导入字体由运行时测量填入。 */
  uneven?: boolean;
}

export interface ClockFontDefinition {
  id: string;
  /** 内置字体的 i18n 标签；导入字体用 name。 */
  labelKey?: MessageKey;
  name?: string;
  /** CSS 注册名，不带引号。 */
  family: string;
  /** 数字用的完整 font-family 栈。 */
  stack: string;
  source: "builtin" | "imported";
  group: "builtin" | "imported";
  metrics?: ClockFontMetrics;
}

export interface BuiltinFontDefinition extends ClockFontDefinition {
  id: DigitFont;
  labelKey: MessageKey;
  source: "builtin";
  group: "builtin";
  metrics: ClockFontMetrics;
}

export const BUILTIN_FONTS: DigitFont[] = ["bebas", "bahnschrift", "system"];

export const FONT_LABELS: Record<DigitFont, MessageKey> = {
  bebas: "appearance.font.bebas",
  bahnschrift: "appearance.font.bahnschrift",
  system: "appearance.font.systemDefault",
};

export const BUILTIN_FONT_DEFINITIONS: Record<DigitFont, BuiltinFontDefinition> = {
  bebas: {
    id: "bebas",
    labelKey: FONT_LABELS.bebas,
    family: "Bebas Neue",
    stack: '"Bebas Neue", "Arial Narrow", Impact, sans-serif',
    source: "builtin",
    group: "builtin",
    metrics: {
      singleScale: 1,
      pairScale: 1,
      shiftEm: 0.05,
      weight: 400,
      spacing: "-0.02em",
      textWeight: 400,
    },
  },
  bahnschrift: {
    id: "bahnschrift",
    labelKey: FONT_LABELS.bahnschrift,
    family: "Bahnschrift",
    stack: 'Bahnschrift, "Segoe UI", "Arial Narrow", sans-serif',
    source: "builtin",
    group: "builtin",
    metrics: {
      singleScale: 0.98,
      pairScale: 0.72,
      shiftEm: 0.065,
      weight: 500,
      spacing: "0",
      textWeight: 500,
    },
  },
  system: {
    id: "system",
    labelKey: FONT_LABELS.system,
    family: "Segoe UI",
    stack: '"Segoe UI", "Segoe UI Variable Text", Arial, sans-serif',
    source: "builtin",
    group: "builtin",
    metrics: {
      singleScale: 0.96,
      pairScale: 0.78,
      shiftEm: -0.065,
      weight: 500,
      spacing: "-0.01em",
      textWeight: 500,
    },
  },
};

export function isBuiltinFontId(value: string): value is DigitFont {
  return (BUILTIN_FONTS as readonly string[]).includes(value);
}

export function builtinFontDefinition(id: DigitFont): BuiltinFontDefinition {
  return BUILTIN_FONT_DEFINITIONS[id];
}
