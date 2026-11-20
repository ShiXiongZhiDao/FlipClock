/**
 * 主题用到的颜色运算。
 *
 * 全部是纯函数：输入一个 #RRGGBB，输出另一个 #RRGGBB 或 rgba()。
 * 材质靠这些函数从「一个基色」推导出一整套明暗，用户不需要理解渐变。
 */

const HEX = /^#[0-9a-fA-F]{6}$/;

export function isHexColor(value: string): boolean {
  return HEX.test(value);
}

export function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const n = Number.parseInt(hex.slice(1), 16);
  return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
}

export function rgbToHex(r: number, g: number, b: number): string {
  const clamp = (v: number) => Math.max(0, Math.min(255, Math.round(v)));
  return `#${((clamp(r) << 16) | (clamp(g) << 8) | clamp(b)).toString(16).padStart(6, "0")}`;
}

/** 线性混合两个颜色，t=0 取 a，t=1 取 b。 */
export function mix(a: string, b: string, t: number): string {
  const k = Math.max(0, Math.min(1, t));
  const ca = hexToRgb(a);
  const cb = hexToRgb(b);
  return rgbToHex(
    ca.r + (cb.r - ca.r) * k,
    ca.g + (cb.g - ca.g) * k,
    ca.b + (cb.b - ca.b) * k,
  );
}

export function lighten(hex: string, amount: number): string {
  return mix(hex, "#ffffff", amount);
}

export function darken(hex: string, amount: number): string {
  return mix(hex, "#000000", amount);
}

export function withAlpha(hex: string, alpha: number): string {
  const { r, g, b } = hexToRgb(hex);
  const a = Math.max(0, Math.min(1, alpha));
  return `rgba(${r}, ${g}, ${b}, ${a.toFixed(3).replace(/0+$/, "").replace(/\.$/, "")})`;
}

/** WCAG 的相对亮度，用于对比度与「凸起 / 压印」判定。 */
export function luminance(hex: string): number {
  const { r, g, b } = hexToRgb(hex);
  const channel = (v: number) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/** WCAG 对比度，1:1 到 21:1。 */
export function contrastRatio(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  const hi = Math.max(la, lb);
  const lo = Math.min(la, lb);
  return (hi + 0.05) / (lo + 0.05);
}

/**
 * 把带透明度的前景合成到不透明背景上，返回不透明的 #RRGGBB。
 *
 * 对比度与「凸起 / 压印」都要用合成后的颜色：只看 #RRGGBB 会把
 * 「卡片 0% 透明」当成一块实色卡，也会漏掉半透明数字被背景冲淡的事实。
 */
export function compositeOver(hex: string, alpha: number, backdrop: string): string {
  const a = Math.max(0, Math.min(1, alpha));
  const fg = hexToRgb(hex);
  const bg = hexToRgb(backdrop);
  return rgbToHex(
    fg.r * a + bg.r * (1 - a),
    fg.g * a + bg.g * (1 - a),
    fg.b * a + bg.b * (1 - a),
  );
}

/**
 * 明暗摆幅的自适应系数。
 *
 * 同一个「提亮 50%」用在亮金上正好，用在近黑上会把哑光黑洗成中灰；
 * 而深咖数字又需要比近黑更大的起伏才看得出金属感。
 * 线性公式满足不了这三者（近黑 0.004 与深咖 0.012 的亮度差太小，
 * 需要的幅度却差一倍），所以用平方根标定：
 *   亮金 0.68 → 1.0（满幅）
 *   镜面金 0.32 → 0.99
 *   深咖 0.012 → 0.19
 *   近黑 0.004 → 0.11
 * 这组系数是拿五个内置主题逐个验算出来的，改它就会改坏内置外观。
 */
export function swingScale(hex: string): number {
  return Math.min(1, 1.75 * Math.sqrt(luminance(hex)));
}
