/**
 * 主题命名：重名加序号，并把名字压进 16 字符（ADR-0020 D13）。
 *
 * 为什么必须在这里截断：复制主题生成的是「<原名字> 副本」，原名字到 14 字符时
 * 结果就已经超限，而超限要到保存时才被 validateThemes 拦下——用户看到的是
 * 「复制不了」，不是「名字太长」。截断至少能让复制成功，且立刻能改名。
 *
 * 规则与 Rust 侧 `settings.rs` 的 `numbered_name` 一致：序号占几位就从词干让出几位，
 * 按字符数算（中文一个字算一个）。两边要同步改——Rust 那边负责载入期的修复，
 * 这里负责创建期就不产生非法值。
 */
import { THEME_NAME_MAX } from "../../types/settings";

/**
 * 把词干压到 16 − reserve 字符。给「复制为新主题」这类**先拼后缀再截断**的场景用：
 * 直接截断拼好的整串会把「副本」切成「副」，所以先按后缀长度让位。
 */
export function fitThemeStem(name: string, reserve: number): string {
  return [...name].slice(0, Math.max(1, THEME_NAME_MAX - reserve)).join("");
}

export function uniqueThemeName(base: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  const fit = (stem: string, suffix: string): string =>
    [...stem].slice(0, Math.max(1, THEME_NAME_MAX - [...suffix].length)).join("") + suffix;

  if ([...base].length <= THEME_NAME_MAX && !used.has(base)) return base;

  // 太长就截断；截断后可能正好撞上已有的名字，那就继续加序号。
  const trimmed = fit(base, "");
  if (!used.has(trimmed)) return trimmed;

  for (let index = 2; index < 100; index += 1) {
    const candidate = fit(base, ` ${index}`);
    if (!used.has(candidate)) return candidate;
  }
  return fit(base, " 99");
}
