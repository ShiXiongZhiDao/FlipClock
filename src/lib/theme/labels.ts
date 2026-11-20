/**
 * 主题在界面上的名字。
 *
 * 内置主题没有 name，按 id 查字典；自定义主题用用户起的名字。
 * 两个页面各写一份时，其中一份靠 `as keyof` 断言绕过字典查表——
 * 自定义 id 一旦走到那里就会拿到 undefined。这里用类型守卫收成一个入口。
 */
import { t } from "../i18n";
import { THEME_LABELS, type ThemeSpec } from "../../types/settings";
import { isBuiltinThemeId } from "./presets";

export function themeLabel(theme: ThemeSpec): string {
  if (isBuiltinThemeId(theme.id)) return t(THEME_LABELS[theme.id]);
  return theme.name || theme.id;
}
