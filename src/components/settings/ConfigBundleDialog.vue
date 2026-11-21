<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";

import { t, locale } from "../../lib/i18n";
import { FONT_LABELS, isBuiltinFontId } from "../../lib/fontRegistry";
import {
  CUSTOM_FONT_LIMIT,
  CUSTOM_THEME_LIMIT,
  THEME_LABELS,
  type BuiltinThemeId,
} from "../../types/settings";
import type {
  ConfigBundleSelection,
  ConfigBundleSummary,
} from "../../types/configBundle";

const props = defineProps<{
  summary: ConfigBundleSummary;
  busy?: boolean;
}>();

const emit = defineEmits<{
  (event: "cancel"): void;
  (event: "confirm", selection: ConfigBundleSelection): void;
}>();

const builtinProfiles = ref(true);
const preferences = ref(true);
const selectedThemes = reactive<Record<string, boolean>>(
  Object.fromEntries(props.summary.customThemes.map((theme) => [theme.id, true])),
);
const selectedFonts = reactive<Record<string, boolean>>(
  Object.fromEntries(props.summary.fonts.map((font) => [font.id, true])),
);

const selectedThemeIds = computed(() => {
  const ids = new Set<string>();
  if (builtinProfiles.value) {
    for (const theme of props.summary.builtinThemes) ids.add(theme.id);
  }
  for (const theme of props.summary.customThemes) {
    if (selectedThemes[theme.id]) ids.add(theme.id);
  }
  return ids;
});

const requiredFontIds = computed(
  () =>
    new Set(
      props.summary.fonts
        .filter((font) => font.usedBy.some((id) => selectedThemeIds.value.has(id)))
        .map((font) => font.id),
    ),
);

watch(
  requiredFontIds,
  (required, previous) => {
    for (const id of required) selectedFonts[id] = true;
    if (!previous) return;
    for (const id of previous) {
      if (!required.has(id)) selectedFonts[id] = false;
    }
  },
  { immediate: true },
);

const themeOverLimit = computed(() => {
  const over = new Set<string>();
  let slots = Math.max(0, CUSTOM_THEME_LIMIT - props.summary.existingCustomThemes);
  for (const theme of props.summary.customThemes) {
    if (!selectedThemes[theme.id] || theme.conflict === "same") continue;
    if (slots > 0) slots -= 1;
    else over.add(theme.id);
  }
  return over;
});

const fontOverLimit = computed(() => {
  const over = new Set<string>();
  let slots = Math.max(0, CUSTOM_FONT_LIMIT - props.summary.existingFonts);
  for (const font of props.summary.fonts) {
    if (!selectedFonts[font.id] || font.missing || font.duplicate) continue;
    if (slots > 0) slots -= 1;
    else over.add(font.id);
  }
  return over;
});

const anySelected = computed(
  () =>
    builtinProfiles.value ||
    preferences.value ||
    props.summary.customThemes.some((theme) => selectedThemes[theme.id]) ||
    props.summary.fonts.some((font) => selectedFonts[font.id]),
);

const dateText = computed(() =>
  new Date(props.summary.exportedAt * 1000).toLocaleString(
    locale.value === "zh" ? "zh-CN" : "en-US",
  ),
);

function themeName(id: string): string {
  if (props.summary.builtinThemes.some((theme) => theme.id === id)) {
    const key = THEME_LABELS[id as BuiltinThemeId];
    return key ? t(key) : id;
  }
  return props.summary.customThemes.find((theme) => theme.id === id)?.name ?? id;
}

const currentThemeName = computed(() => themeName(props.summary.currentTheme));

function fontName(id: string): string {
  if (isBuiltinFontId(id)) return t(FONT_LABELS[id]);
  return props.summary.fonts.find((font) => font.id === id)?.name ?? id;
}

function usedByLabel(usedBy: string[]): string {
  return usedBy.map(themeName).join(locale.value === "zh" ? "、" : ", ");
}

function fontRequired(id: string): boolean {
  return requiredFontIds.value.has(id);
}

function conflictLabel(conflict: string): string {
  if (conflict === "same") return t("system.configExisting");
  if (conflict === "id" || conflict === "name") return t("system.configCopy");
  return "";
}

function cancel(): void {
  if (!props.busy) emit("cancel");
}

function confirm(): void {
  if (props.busy || !anySelected.value) return;
  emit("confirm", {
    builtinProfiles: builtinProfiles.value,
    themeIds: props.summary.customThemes
      .filter((theme) => selectedThemes[theme.id])
      .map((theme) => theme.id),
    fontIds: props.summary.fonts
      .filter((font) => selectedFonts[font.id])
      .map((font) => font.id),
    preferences: preferences.value,
  });
}
</script>

<template>
  <div class="bundle-backdrop" @click.self="cancel">
    <div
      class="bundle-dialog"
      role="dialog"
      aria-modal="true"
      :aria-label="t('system.configImportTitle')"
    >
      <header class="bundle-header">
        <strong>{{ t("system.configImportTitle") }}</strong>
        <span class="bundle-from">
          {{ t("system.configFrom", { version: summary.appVersion, date: dateText }) }}
        </span>
      </header>

      <p class="bundle-lead">
        {{ t("system.configImportLead", {
          themes: summary.customThemes.length,
          fonts: summary.fonts.length,
        }) }}
      </p>
      <p class="bundle-capacity">
        {{ t("system.configCapacity", {
          themes: summary.existingCustomThemes,
          themeLimit: CUSTOM_THEME_LIMIT,
          fonts: summary.existingFonts,
          fontLimit: CUSTOM_FONT_LIMIT,
        }) }}
      </p>
      <p class="bundle-capacity">{{ t("system.configMergeNote") }}</p>
      <p class="bundle-capacity">
        {{ t("system.configCurrentTheme", { name: currentThemeName }) }}
      </p>

      <div class="bundle-body">
        <section class="bundle-section">
          <div class="bundle-section-title">{{ t("system.configBuiltinGroup") }}</div>
          <label class="bundle-row">
            <input v-model="builtinProfiles" type="checkbox" />
            <span class="bundle-name">
              {{ t("system.configBuiltin", { count: summary.builtinThemes.length }) }}
              <em v-if="builtinProfiles">{{ t("system.configIncluded") }}</em>
            </span>
          </label>
        </section>

        <section v-if="summary.customThemes.length" class="bundle-section">
          <div class="bundle-section-title">
            {{ t("system.configCustomThemes") }}
            <em>{{ summary.customThemes.length }}</em>
          </div>
          <label
            v-for="theme in summary.customThemes"
            :key="theme.id"
            class="bundle-row"
            :class="{ current: theme.current }"
          >
            <input v-model="selectedThemes[theme.id]" type="checkbox" />
            <span class="bundle-name">
              {{ theme.name }}
              <em v-if="theme.current">{{ t("system.configCurrent") }}</em>
            </span>
            <small class="bundle-font">{{ fontName(theme.font) }}</small>
            <small v-if="conflictLabel(theme.conflict)" class="bundle-tag">
              {{ conflictLabel(theme.conflict) }}
            </small>
            <small v-if="theme.missingImages.length" class="bundle-warn">
              {{ t("system.configMissingImages", { files: theme.missingImages.join(", ") }) }}
            </small>
            <small v-if="themeOverLimit.has(theme.id)" class="bundle-warn">
              {{ t("system.configOverLimit") }}
            </small>
          </label>
        </section>

        <section v-if="summary.fonts.length" class="bundle-section">
          <div class="bundle-section-title">
            {{ t("system.configFonts") }}
            <em>{{ summary.fonts.length }}</em>
          </div>
          <label v-for="font in summary.fonts" :key="font.id" class="bundle-row">
            <input
              v-model="selectedFonts[font.id]"
              type="checkbox"
              :disabled="fontRequired(font.id)"
            />
            <span class="bundle-name">
              {{ font.name }}
              <em v-if="fontRequired(font.id)">{{ t("system.configRequired") }}</em>
            </span>
            <small v-if="font.usedBy.length" class="bundle-used">
              {{ t("system.configUsedBy", { themes: usedByLabel(font.usedBy) }) }}
            </small>
            <small v-if="font.missing" class="bundle-warn">
              {{ t("system.configMissingFont") }}
            </small>
            <small v-else-if="fontOverLimit.has(font.id)" class="bundle-warn">
              {{ t("system.configOverLimit") }}
            </small>
            <small v-else-if="font.duplicate" class="bundle-tag">
              {{ t("system.configExisting") }}
            </small>
          </label>
        </section>

        <section class="bundle-section">
          <div class="bundle-section-title">{{ t("system.configPreferencesTitle") }}</div>
          <label class="bundle-row">
            <input v-model="preferences" type="checkbox" />
            <span class="bundle-name">{{ t("system.configPreferences") }}</span>
          </label>
        </section>
      </div>

      <p class="bundle-backup">
        {{ t("system.configBackup", { path: summary.backupPath }) }}
      </p>

      <footer class="bundle-actions">
        <button type="button" class="secondary" :disabled="busy" @click="cancel">
          {{ t("system.restoreCancel") }}
        </button>
        <button
          type="button"
          class="primary"
          :disabled="busy || !anySelected"
          @click="confirm"
        >
          {{ busy ? t("system.configImportBusy") : t("system.configImportConfirm") }}
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.bundle-backdrop {
  position: fixed;
  inset: 0;
  z-index: 650;
  display: grid;
  place-items: center;
  background: var(--ui-scrim);
}

.bundle-dialog {
  width: min(560px, calc(100% - 40px));
  max-height: min(84vh, 720px);
  display: flex;
  flex-direction: column;
  padding: 18px 20px 16px;
  border: 1px solid var(--ui-divider-strong);
  border-radius: 6px;
  background: var(--ui-surface);
  box-shadow: var(--ui-shadow-dialog);
}

.bundle-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.bundle-header strong {
  font-size: 14px;
  font-weight: 600;
  color: var(--ui-text);
}

.bundle-from {
  font-size: 9px;
  color: var(--ui-text-muted);
  white-space: nowrap;
}

.bundle-lead,
.bundle-capacity {
  margin: 8px 0 0;
  font-size: 11px;
  line-height: 1.6;
  color: var(--ui-text-secondary);
}

.bundle-capacity {
  margin-top: 2px;
  font-size: 9.5px;
  color: var(--ui-text-muted);
}

.bundle-body {
  min-height: 0;
  margin: 10px -4px 0;
  padding: 0 4px;
  overflow-y: auto;
}

.bundle-section + .bundle-section {
  margin-top: 10px;
}

.bundle-section-title {
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 0 8px 3px;
  font-size: 9px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--ui-text-muted);
}

.bundle-section-title em {
  font-style: normal;
  letter-spacing: 0;
  text-transform: none;
}

.bundle-row {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: 3px 8px;
  padding: 5px 8px;
  border-radius: 4px;
  font-size: 11px;
  color: var(--ui-text-secondary);
  cursor: pointer;
}

.bundle-row:hover {
  background: var(--ui-thumb);
}

.bundle-row.current {
  background: color-mix(in srgb, var(--ui-accent) 7%, transparent);
}

.bundle-row input {
  margin: 2px 0 0;
  accent-color: var(--ui-accent);
}

.bundle-name {
  display: flex;
  align-items: baseline;
  gap: 6px;
  color: var(--ui-text);
}

.bundle-name em,
.bundle-tag,
.bundle-warn,
.bundle-used,
.bundle-font {
  font-style: normal;
  font-size: 9px;
  line-height: 1.4;
}

.bundle-name em {
  color: var(--ui-accent);
}

.bundle-font {
  margin-left: auto;
  color: var(--ui-text-muted);
}

.bundle-used {
  flex-basis: 100%;
  padding-left: 20px;
  color: var(--ui-text-muted);
}

.bundle-tag {
  color: var(--ui-text-muted);
}

.bundle-warn {
  flex-basis: 100%;
  padding-left: 20px;
  color: var(--ui-accent);
}

.bundle-backup {
  margin: 10px 0 0;
  padding-top: 9px;
  border-top: 1px solid var(--ui-divider);
  font-size: 9.5px;
  line-height: 1.5;
  color: var(--ui-text-muted);
  word-break: break-all;
}

.bundle-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 14px;
}
</style>
