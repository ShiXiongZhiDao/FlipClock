/** 配置包（ADR-0027）的前后端形状。字段名与 Rust 的 camelCase 序列化一致。 */

export type BundleConflict = "none" | "same" | "id" | "name";

export interface ConfigBundleThemeSummary {
  id: string;
  name: string;
  current: boolean;
  /** 该主题钟面配置使用的字体 id；内置字体或导入字体。 */
  font: string;
  missingImages: string[];
  conflict: BundleConflict;
}

export interface ConfigBundleFontSummary {
  id: string;
  name: string;
  /** 哪些主题的钟面配置使用它；内置主题 id 或自定义主题 id。 */
  usedBy: string[];
  missing: boolean;
  duplicate: boolean;
}

export interface ConfigBundleSummary {
  formatVersion: number;
  schemaVersion: number;
  appVersion: string;
  exportedAt: number;
  currentTheme: string;
  builtinThemes: ConfigBundleThemeSummary[];
  customThemes: ConfigBundleThemeSummary[];
  fonts: ConfigBundleFontSummary[];
  preferences: boolean;
  existingCustomThemes: number;
  existingFonts: number;
  backupPath: string;
}

export interface ConfigBundleSelection {
  builtinProfiles: boolean;
  themeIds: string[];
  fontIds: string[];
  preferences: boolean;
}

export type ImportSkipReason = "themeLimit" | "fontLimit" | "fontMissing";

export interface ConfigImportSkip {
  id: string;
  name: string;
  reason: ImportSkipReason;
}

export interface ConfigImportResult {
  themes: number;
  fonts: number;
  skipped: ConfigImportSkip[];
  currentTheme: string;
}
