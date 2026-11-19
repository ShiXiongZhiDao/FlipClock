export type Locale = "zh" | "en";
export type LanguagePreference = "system" | "zh" | "en";

export function localeFromSystem(system: string): Locale;
export function resolveLocale(preference: LanguagePreference, system: string): Locale;
export function interpolate(
  text: string,
  params?: Record<string, string | number>,
): string;
export function errorKey(code: string): string;
export function translate(
  dictionary: { zh: Record<string, string>; en: Record<string, string> },
  locale: Locale,
  key: string,
  params?: Record<string, string | number>,
): string;
export function parseErrorPayload(text: string): {
  code: string;
  params: Record<string, string>;
} | null;
