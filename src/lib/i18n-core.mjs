/**
 * 界面语言的纯逻辑。设计见 docs/adr/0018。
 *
 * 这里刻意不依赖 vue、tauri、DOM 或任何 npm 包，好让 CI 直接 `node` 就能验证
 * 归并/插值/错误码解析——publish-public.yml 不装依赖，也没有 pnpm。
 * i18n.ts 负责字典与响应式外壳，调用这里的函数。
 */

/** 系统语言归并：zh-* → 中文，其余 → 英文。zh-TW 明确归到简体，不做地区变体。 */
export function localeFromSystem(system) {
  return system.toLowerCase().startsWith("zh") ? "zh" : "en";
}

/** 偏好三态 → 生效语言二值。system 由系统语言归并，其余直接用偏好。 */
export function resolveLocale(preference, system) {
  return preference === "system" ? localeFromSystem(system) : preference;
}

/** {name} 占位符替换；缺失参数原样保留，方便一眼看出调用方漏传。 */
export function interpolate(text, params) {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name) =>
    Object.prototype.hasOwnProperty.call(params, name) ? String(params[name]) : whole,
  );
}

/**
 * Rust 错误码是裸名（copySaverFailed），字典 key 带 error. 前缀。
 * 2026-11-19 就是漏了这个前缀，让用户看到裸 JSON。放在这里好让闸直接验证。
 */
export function errorKey(code) {
  return `error.${code}`;
}

/** 查字典 + 插值。字典由调用方传入，避免本模块持有状态。 */
export function translate(dictionary, locale, key, params) {
  return interpolate(dictionary[locale][key], params);
}

/**
 * 把 Rust 的错误串解析成码与参数；认不出就返回 null。
 * 认不出的错误原样显示，不吞第三方（如 updater）的英文错误。
 */
export function parseErrorPayload(text) {
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object") return null;
  const obj = parsed;
  if (typeof obj.code !== "string") return null;
  const params = {};
  // 转发全部标量参数，不只 detail/key/label。
  //
  // 白名单式转发过一次真实的坑：Rust 新增 limit_mb/limit_side 后，字典里的
  // {limitMb} 拿不到值，用户看到的是没被替换的裸占位符；而类型检查与当时的
  // i18n 闸都只比 key 集合，谁都没拦住。code 不转发——它是查表用的，不是插值参数。
  for (const [key, value] of Object.entries(obj)) {
    if (key === "code") continue;
    if (typeof value === "string" || typeof value === "number") params[key] = value;
  }
  return { code: obj.code, params };
}
