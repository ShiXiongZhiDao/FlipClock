/**
 * 配置对象的深拷贝。
 *
 * 配置里只有 JSON 能表示的字段（字符串、数字、布尔、数组、普通对象），
 * JSON 往返足够。不用 `structuredClone`：它碰到 Vue 的响应式代理在部分
 * WebView 上会抛 DataCloneError，而这个函数正好都作用在响应式对象上。
 */
export function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
