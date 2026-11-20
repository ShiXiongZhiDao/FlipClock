/**
 * 五个内置主题的预设（ADR-0020）。
 *
 * 它们是产品的一部分（截图与文档都指向），所以不可就地修改，只能复制。
 * 数值是从原 themes.css 的变量反推出来的：材质按变量形态判定，
 * 颜色取各变量的中位色，`lineColor` 只在边线色与主色不同族时才写。
 */
import { cloneJson } from "../clone";
import type { BuiltinThemeId, ThemeSpec } from "../../types/settings";

export const BUILTIN_THEME_IDS: BuiltinThemeId[] = [
  "classic-dark",
  "oled-black",
  "retro-light",
  "mirror-gold",
  "black-label",
];

export const BUILTIN_THEMES: Record<BuiltinThemeId, ThemeSpec> = {
  "classic-dark": {
    id: "classic-dark",
    name: "",
    background: { color: "#111315", opacity: 100, image: null, fit: "cover", dim: 0 },
    card: { material: "matte", color: "#303437", opacity: 100 },
    digit: { material: "matte", color: "#f1f0e9", opacity: 100, image: null },
    date: { color: "#9ba19f", opacity: 100 },
    seam: { color: "#000000", opacity: 72 },
    separator: { color: "#f1f0e9", opacity: 62 },
  },
  "oled-black": {
    id: "oled-black",
    name: "",
    background: { color: "#000000", opacity: 100, image: null, fit: "cover", dim: 0 },
    card: { material: "matte", color: "#202020", opacity: 100 },
    digit: { material: "matte", color: "#f8f8f8", opacity: 100, image: null },
    date: { color: "#8a8a8a", opacity: 100 },
    seam: { color: "#000000", opacity: 85 },
    separator: { color: "#f8f8f8", opacity: 62 },
  },
  "retro-light": {
    id: "retro-light",
    name: "",
    background: { color: "#ddd4c5", opacity: 100, image: null, fit: "cover", dim: 0 },
    card: { material: "matte", color: "#403d37", opacity: 100 },
    digit: { material: "matte", color: "#f3ecdf", opacity: 100, image: null },
    date: { color: "#716b60", opacity: 100 },
    seam: { color: "#000000", opacity: 78 },
    separator: { color: "#f3ecdf", opacity: 62 },
  },
  "mirror-gold": {
    id: "mirror-gold",
    name: "",
    background: { color: "#100b06", opacity: 100, image: null, fit: "cover", dim: 0 },
    // 亮金镜面卡 + 深咖压印数字：数字比卡暗，自动判为压印。
    card: { material: "metallic", color: "#c18a37", opacity: 100 },
    digit: { material: "metallic", color: "#2a1a08", opacity: 100, image: null },
    date: { color: "#d2b77f", opacity: 100 },
    seam: { color: "#4a2d0e", opacity: 100 },
    separator: { color: "#2a1a08", opacity: 62 },
  },
  "black-label": {
    id: "black-label",
    name: "",
    background: { color: "#070706", opacity: 100, image: null, fit: "cover", dim: 0 },
    // 近黑卡 + 独立金边 + 浅金凸起数字：数字比卡亮，自动判为凸起。
    card: { material: "metallic", color: "#0c0b09", opacity: 100, lineColor: "#d2aa5b" },
    digit: { material: "metallic", color: "#f2d797", opacity: 100, image: null },
    date: { color: "#b19b70", opacity: 100 },
    seam: { color: "#020202", opacity: 100 },
    separator: { color: "#f2d797", opacity: 62 },
  },
};

/** 自定义主题的默认值：新建时从这个模板起步。 */
export function blankTheme(id: string, name: string): ThemeSpec {
  return {
    id,
    name,
    background: { color: "#111315", opacity: 100, image: null, fit: "cover", dim: 0 },
    card: { material: "matte", color: "#303437", opacity: 100 },
    digit: { material: "matte", color: "#f1f0e9", opacity: 100, image: null },
    date: { color: "#9ba19f", opacity: 100 },
    seam: { color: "#000000", opacity: 72 },
    separator: { color: "#f1f0e9", opacity: 62 },
  };
}

/** 复制一份主题：换 id、换名字，其余原样深拷贝。 */
export function cloneTheme(source: ThemeSpec, id: string, name: string): ThemeSpec {
  return cloneJson({ ...source, id, name });
}

export function isBuiltinThemeId(id: string): id is BuiltinThemeId {
  return (BUILTIN_THEME_IDS as string[]).includes(id);
}

/** 把「当前主题 id」解析成规格；找不到就退回经典碳灰，绝不返回空。 */
export function resolveTheme(id: string, custom: ThemeSpec[]): ThemeSpec {
  if (isBuiltinThemeId(id)) return BUILTIN_THEMES[id];
  return custom.find((theme) => theme.id === id) ?? BUILTIN_THEMES["classic-dark"];
}
