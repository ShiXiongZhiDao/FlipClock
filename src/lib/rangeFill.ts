/**
 * 自绘滑杆的「已走过」比例。
 *
 * 全局 `.range` 用 `--fill` 定位渐变分界点；不传这个变量，轨道会停在默认 50%，
 * 滑块在 80% 而填充停在中间——同一件事在两处各算一遍时，第一版就漏传了一半控件。
 * 这里收成一份，设置页与全屏配置共用。
 */
export function rangeFill(value: number, min: number, max: number): Record<string, string> {
  const span = max - min;
  const ratio = span > 0 ? (value - min) / span : 0;
  // 保留到 0.1%：整数百分比在长轨道上会看到 1px 的跳格。
  const percent = Math.min(100, Math.max(0, Math.round(ratio * 1000) / 10));
  return { "--fill": `${percent}%` };
}
