<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";

import FlipClock from "../clock/FlipClock.vue";
import { useClock } from "../../composables/useClock";
import { previewScaleOf } from "../../composables/useClockScale";
import { useToast } from "../../composables/useToast";
import { playFlipSound } from "../../lib/flipSound";
import { applyLanguage, errorText, previewLanguage, t, type MessageKey } from "../../lib/i18n";
import { rangeFill } from "../../lib/rangeFill";
import { settingsPatchOf } from "../../lib/settingsPatch";
import { forgetThemeAssets, themeAssetUrl } from "../../lib/theme/assets";
import { compositeOver, contrastRatio } from "../../lib/theme/color";
import { themeLabel } from "../../lib/theme/labels";
import { cardSurfaceColor } from "../../lib/theme/material";
import { swatchStyle } from "../../lib/theme/swatch";
import { uniqueThemeName } from "../../lib/theme/names";
import { BUILTIN_THEMES, blankTheme, isBuiltinThemeId, resolveTheme } from "../../lib/theme/presets";
import { ensureProfiles, cloneProfile, profileFor, profileOf, profileSummary, setSoundVolume } from "../../lib/theme/profile";
import {
  resetStudio,
  studioDraft,
  studioEditingThemeId,
  studioInitialJson,
  studioIsDirty,
  studioNewThemeIds,
  studioTab,
  type StudioTab,
} from "../../lib/theme/studio";
import {
  BACKGROUND_DIM_MAX,
  BRIGHTNESS_MAX,
  BRIGHTNESS_MIN,
  CARD_OPACITY_MIN,
  CLOCK_SCALE_MAX,
  CLOCK_SCALE_MIN,
  CUSTOM_THEME_LIMIT,
  DEFAULT_CLOCK_PROFILE,
  FONT_LABELS,
  OPACITY_MAX,
  PART_OPACITY_MIN,
  THEME_NAME_MAX,
  type BackgroundFit,
  type DigitFont,
  type Settings,
  type ThemeMaterial,
  type ThemeSpec,
} from "../../types/settings";

const { now } = useClock();
const { toast, notify } = useToast(2600);

const MATERIALS: ThemeMaterial[] = ["matte", "metallic", "soft"];
const MATERIAL_LABELS: Record<ThemeMaterial, MessageKey> = {
  matte: "appearance.material.matte",
  metallic: "appearance.material.metallic",
  soft: "appearance.material.soft",
};
const FITS: BackgroundFit[] = ["cover", "tile"];
const FONTS: DigitFont[] = ["bebas", "bahnschrift", "system"];
const TABS: StudioTab[] = ["theme", "clock", "sound"];
const SCALE_MIN = Math.round(CLOCK_SCALE_MIN * 100);
const SCALE_MAX = Math.round(CLOCK_SCALE_MAX * 100);
const BRIGHTNESS_MIN_PCT = Math.round(BRIGHTNESS_MIN * 100);
const BRIGHTNESS_MAX_PCT = Math.round(BRIGHTNESS_MAX * 100);

const SWATCHES = [
  "#d4ad61",
  "#c9ccd1",
  "#b87333",
  "#e0b0a0",
  "#3b4045",
  "#0b0a08",
  "#fff0cd",
  "#8a8f95",
];

const draft = computed(() => studioDraft.value);
const profile = computed(() => (draft.value ? profileOf(draft.value) : DEFAULT_CLOCK_PROFILE));

/** 主题卡预览要画「这张卡自己的钟面」，不能借用当前主题的配置。 */
function cardProfile(theme: ThemeSpec) {
  return draft.value ? profileFor(draft.value, theme.id) : DEFAULT_CLOCK_PROFILE;
}

const currentTheme = computed<ThemeSpec | null>(() =>
  draft.value
    ? resolveTheme(draft.value.appearance.theme, draft.value.appearance.customThemes)
    : null,
);

const themeList = computed<ThemeSpec[]>(() => {
  if (!draft.value) return [];
  return [
    ...Object.values(BUILTIN_THEMES),
    ...draft.value.appearance.customThemes,
  ];
});

const editingTheme = computed<ThemeSpec | null>(() => {
  const id = studioEditingThemeId.value;
  if (!id || !draft.value) return null;
  return draft.value.appearance.customThemes.find((theme) => theme.id === id) ?? null;
});

const backgroundUrl = computed(() =>
  currentTheme.value
    ? themeAssetUrl(currentTheme.value.id, currentTheme.value.background.image)
    : null,
);
const digitUrl = computed(() =>
  currentTheme.value ? themeAssetUrl(currentTheme.value.id, currentTheme.value.digit.image) : null,
);

const cardSurface = computed(() =>
  editingTheme.value
    ? cardSurfaceColor(editingTheme.value.card, editingTheme.value.background.color)
    : "#000000",
);

const draftContrast = computed(() => {
  const theme = editingTheme.value;
  if (!theme) return 21;
  const digit = compositeOver(theme.digit.color, theme.digit.opacity / 100, cardSurface.value);
  return contrastRatio(digit, cardSurface.value);
});

const nameError = ref<MessageKey | null>(null);
const exitPrompt = ref(false);
/**
 * 确认框弹开时焦点落在对话框容器，不落在任何按钮上。
 *
 * 落在「继续编辑」上会顶着一圈 focus 环——真实 WebView2 里程序化 focus 也算
 * :focus-visible，而无头 Chrome 不算，这正是上一版漏掉的原因。落在容器上则
 * 三条都满足：按钮不顶环、误按 Enter 不会触发保存/放弃、键盘 Tab 仍能逐个聚焦。
 */
const confirmRef = ref<HTMLElement | null>(null);
watch(exitPrompt, (open) => {
  if (!open) return;
  void nextTick(() => confirmRef.value?.focus());
});
/** 抽屉默认收起，指针靠近右边缘才滑出。 */
const drawerVisible = ref(false);
const drawerRef = ref<HTMLElement | null>(null);
let hideTimer: ReturnType<typeof setTimeout> | undefined;
let allowClose = false;
let unlistenClose: (() => void) | undefined;

const EDGE_SHOW = 72;
const DRAWER_WIDTH = 400;
const HIDE_DELAY = 500;

/** 只有正在输入文字才留住抽屉；滑杆/按钮的焦点不该让它赖着不走。 */
function isEditingText(): boolean {
  const active = document.activeElement;
  if (!active || !drawerRef.value?.contains(active)) return false;
  return (
    (active instanceof HTMLInputElement &&
      (active.type === "text" || active.type === "search" || active.type === "number")) ||
    active instanceof HTMLTextAreaElement
  );
}

function onPointerMove(event: PointerEvent): void {
  const edge = window.innerWidth - event.clientX;
  // 进入右边缘，或已经展开且指针还在抽屉范围内：保持展开。
  if (edge <= EDGE_SHOW || (drawerVisible.value && edge <= DRAWER_WIDTH)) {
    clearTimeout(hideTimer);
    drawerVisible.value = true;
    return;
  }
  if (!drawerVisible.value) return;
  clearTimeout(hideTimer);
  // 指针回到钟面就收起；不再因为「刚点过抽屉」或按钮焦点而拖着不收。
  hideTimer = setTimeout(() => {
    if (exitPrompt.value || nameError.value || isEditingText()) return;
    drawerVisible.value = false;
  }, HIDE_DELAY);
}

const scalePercent = computed({
  get: () => Math.round(profile.value.clockScale * 100),
  set: (value: number) => {
    profile.value.clockScale = value / 100;
  },
});

const brightnessPercent = computed({
  get: () => Math.round(profile.value.brightness * 100),
  set: (value: number) => {
    profile.value.brightness = value / 100;
  },
});

const volumePercent = computed({
  get: () => Math.round(profile.value.sound.volume * 100),
  set: (value: number) => {
    setSoundVolume(profile.value, value);
  },
});

function selectTheme(id: string): void {
  if (!draft.value) return;
  draft.value.appearance.theme = id;
  studioEditingThemeId.value = null;
  nameError.value = null;
}

function editTheme(id: string): void {
  if (!draft.value) return;
  draft.value.appearance.theme = id;
  studioEditingThemeId.value = id;
  nameError.value = null;
}

/**
 * 在主题墙直接新建。全屏窗口里不能再调 launch("theme")——那会开新窗口，
 * 这里改的是同一份草稿，保存/放弃都跟着当前窗口走。
 */
function newTheme(): void {
  const d = draft.value;
  if (!d) return;
  if (d.appearance.customThemes.length >= CUSTOM_THEME_LIMIT) {
    notify(t("appearance.limitReached", { limit: CUSTOM_THEME_LIMIT }));
    return;
  }
  const id = `custom-${crypto.randomUUID()}`;
  const name = uniqueThemeName(
    t("appearance.myTheme"),
    d.appearance.customThemes.map((item) => item.name),
  );

  // 继承来源要在切走之前取：ADR-0022 D7「你看到什么，新主题就是什么」。
  const from = d.appearance.theme;
  d.appearance.customThemes.push(blankTheme(id, name));
  // 新主题必须有自己的钟面配置，否则切过去时 profileOf 会拿到兜底对象，
  // 用户调的字体/亮度不会随草稿保存。
  ensureProfiles(d);
  // ensureProfiles 只补**默认值**，不是继承。这里必须显式拷一份当前主题的配置，
  // 否则新建主题会悄悄把秒数、音量、缩放打回默认——用户在设置页新建时不会这样
  // （那条路径走 buildStudioPayload，默认 profileFrom 就是当前主题）。
  d.appearance.profiles[id] = cloneProfile(profileFor(d, from));
  studioNewThemeIds.value.push(id);
  editTheme(id);
  drawerVisible.value = true;
}

async function pickImage(kind: "background" | "digit"): Promise<void> {
  const theme = editingTheme.value;
  if (!theme) return;
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: t("appearance.imageFilter"), extensions: ["png", "jpg", "jpeg", "webp"] }],
  });
  if (!selected || typeof selected !== "string") return;
  try {
    const filename = await invoke<string>("import_theme_image", {
      themeId: theme.id,
      sourcePath: selected,
      kind,
    });
    // 旧文件不在这里删：草稿可能被放弃，删了会让配置指向一个不存在的文件。
    // 保存成功后再统一清掉不再引用的旧图。
    if (kind === "background") theme.background.image = filename;
    else theme.digit.image = filename;
    forgetThemeAssets(theme.id);
  } catch (error) {
    notify(errorText(error));
  }
}

function clearImage(kind: "background" | "digit"): void {
  const theme = editingTheme.value;
  if (!theme) return;
  if (kind === "background") theme.background.image = null;
  else theme.digit.image = null;
  forgetThemeAssets(theme.id);
}

function audition(): void {
  const result = playFlipSound(profile.value.sound.volume);
  if (result === "unsupported") notify(t("sound.unsupported"));
  else if (result === "muted") notify(t("sound.muted"));
}

function setLanguage(value: string): void {
  if (!draft.value) return;
  const language = value === "zh" || value === "en" || value === "system" ? value : "system";
  draft.value.ui.language = language;
  // 预览只改本窗口文字；托盘与持久化留到保存。
  previewLanguage(language);
}

function failOn(id: string, key: MessageKey): false {
  drawerVisible.value = true;
  studioTab.value = "theme";
  studioEditingThemeId.value = id;
  nameError.value = key;
  return false;
}

/** 保存前校验：名字、重名、数量上限。失败时切到出错的主题并高亮。 */
function validateThemes(): boolean {
  const d = draft.value;
  if (!d) return false;
  if (d.appearance.customThemes.length > CUSTOM_THEME_LIMIT) {
    notify(t("appearance.limitReached", { limit: CUSTOM_THEME_LIMIT }));
    drawerVisible.value = true;
    studioTab.value = "theme";
    return false;
  }
  const names = new Set<string>();
  for (const theme of d.appearance.customThemes) {
    const name = theme.name.trim();
    if (!name) return failOn(theme.id, "appearance.nameRequired");
    if ([...name].length > THEME_NAME_MAX) return failOn(theme.id, "appearance.nameTooLong");
    if (names.has(name)) return failOn(theme.id, "appearance.nameTaken");
    names.add(name);
  }
  return true;
}

/** 放弃时删掉本次新导入、初始配置里没有的图片；旧图必须留着给回滚后的配置用。 */
async function cleanupImportedImages(): Promise<void> {
  const d = draft.value;
  if (!d || !studioInitialJson.value) return;
  const initial = JSON.parse(studioInitialJson.value) as Settings;
  const jobs: Promise<unknown>[] = [];
  for (const theme of d.appearance.customThemes) {
    const before = initial.appearance.customThemes.find((item) => item.id === theme.id);
    // 新主题整个目录会被删，不必逐张处理。
    if (!before) continue;
    const keep = new Set(
      [before.background.image, before.digit.image].filter((name): name is string => !!name),
    );
    for (const name of [theme.background.image, theme.digit.image]) {
      if (name && !keep.has(name)) {
        jobs.push(
          invoke("remove_theme_image", { themeId: theme.id, filename: name }).catch(() => {}),
        );
      }
    }
  }
  await Promise.all(jobs);
}

/** 只删除本次保存后不再被引用的旧图片；保存前一律不动。 */
async function cleanupImages(): Promise<void> {
  const d = draft.value;
  if (!d || !studioInitialJson.value) return;
  const initial = JSON.parse(studioInitialJson.value) as Settings;
  const jobs: Promise<unknown>[] = [];
  for (const old of initial.appearance.customThemes) {
    const now = d.appearance.customThemes.find((theme) => theme.id === old.id);
    if (!now) continue;
    const pairs: Array<[string | null, string | null]> = [
      [old.background.image, now.background.image],
      [old.digit.image, now.digit.image],
    ];
    for (const [before, after] of pairs) {
      if (before && before !== after) {
        jobs.push(
          invoke("remove_theme_image", { themeId: old.id, filename: before }).catch(() => {}),
        );
      }
    }
  }
  await Promise.all(jobs);
}

async function closeWindow(): Promise<void> {
  allowClose = true;
  try {
    await invoke("close_theme_preview");
    resetStudio();
  } catch (error) {
    allowClose = false;
    notify(errorText(error));
  }
}

async function save(): Promise<boolean> {
  const d = draft.value;
  if (!d) return false;
  if (!validateThemes()) return false;
  for (const theme of d.appearance.customThemes) theme.name = theme.name.trim();
  try {
    await invoke("save_settings", { patch: settingsPatchOf(d) });
  } catch (error) {
    notify(errorText(error));
    return false;
  }
  await cleanupImages();
  // 语言在保存时才算数：这时才切托盘，与整场草稿的承诺一致。
  applyLanguage(d.ui.language);
  await closeWindow();
  return true;
}

async function discard(): Promise<void> {
  // 放弃前清掉本次新导入的图片：草稿回滚了，这些文件没有任何配置再引用。
  await cleanupImportedImages();
  if (studioNewThemeIds.value.length > 0) {
    await Promise.all(
      studioNewThemeIds.value.map((id) => {
        forgetThemeAssets(id);
        return invoke("delete_theme_dir", { themeId: id }).catch(() => {});
      }),
    );
  }
  await closeWindow();
}

function requestClose(): void {
  if (studioIsDirty()) {
    drawerVisible.value = true;
    exitPrompt.value = true;
  } else {
    void discard();
  }
}

async function onSaveFromPrompt(): Promise<void> {
  exitPrompt.value = false;
  await save();
}

async function onDiscardFromPrompt(): Promise<void> {
  exitPrompt.value = false;
  await discard();
}

function onKey(event: KeyboardEvent): void {
  if (event.key !== "Escape") return;
  if (exitPrompt.value) {
    exitPrompt.value = false;
    return;
  }
  requestClose();
}

watch(
  () => studioDraft.value?.ui.language,
  (language) => {
    if (language) previewLanguage(language);
  },
);

onMounted(async () => {
  if (draft.value) ensureProfiles(draft.value);
  window.addEventListener("keydown", onKey);
  window.addEventListener("pointermove", onPointerMove);
  unlistenClose = await getCurrentWindow().onCloseRequested((event) => {
    if (allowClose) return;
    event.preventDefault();
    requestClose();
  });
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
  window.removeEventListener("pointermove", onPointerMove);
  clearTimeout(hideTimer);
  unlistenClose?.();
});
</script>

<template>
  <Teleport to="body">
    <div v-if="draft && currentTheme" class="overlay">
      <FlipClock
        :now="now"
        :format="profile.format"
        :show-date="profile.showDate"
        :show-seconds="profile.showSeconds"
        :theme="currentTheme"
        :background-url="backgroundUrl"
        :digit-url="digitUrl"
        :layout="profile.digitLayout"
        :show-colon="profile.showColon"
        :font="profile.font"
        :brightness="profile.brightness"
        :scale="profile.clockScale"
        :sound="profile.sound"
        variant="stage"
      />

      <!-- 抽屉收起时留在屏幕右边缘的小把手：告诉用户「往右移」有东西 -->
      <button
        v-if="!drawerVisible"
        type="button"
        class="drawer-peek"
        :aria-label="t('studio.open')"
        @click="drawerVisible = true"
      >
        <span class="peek-line"></span>
        <span class="peek-label">{{ t("studio.title") }}</span>
        <span class="peek-line"></span>
      </button>

      <aside
        ref="drawerRef"
        class="drawer"
        :class="{ hidden: !drawerVisible }"
        :inert="!drawerVisible"
      >
        <header class="drawer-head">
          <span class="drawer-title">{{ t("studio.title") }}</span>
          <button type="button" class="icon" :aria-label="t('studio.discard')" @click="requestClose">
            ×
          </button>
        </header>

        <nav class="tabs">
          <button
            v-for="tab in TABS"
            :key="tab"
            type="button"
            :class="{ selected: studioTab === tab }"
            @click="studioTab = tab"
          >
            {{ t(`studio.tab.${tab}` as MessageKey) }}
          </button>
        </nav>

        <div
          class="drawer-body"
          :class="{ 'theme-body': studioTab === 'theme' && !editingTheme }"
        >
          <!-- 主题 -->
          <template v-if="studioTab === 'theme'">
            <template v-if="!editingTheme">
              <div class="theme-grid">
                <div
                  v-for="theme in themeList"
                  :key="theme.id"
                  class="theme-item"
                >
                  <button
                    type="button"
                    class="theme-pick theme-sample"
                    :class="{ 'is-active': draft.appearance.theme === theme.id }"
                    :style="swatchStyle(theme)"
                    @click="selectTheme(theme.id)"
                  >
                    <span class="theme-stage stage theme-sample-stage">
                      <FlipClock
                        :now="now"
                        :format="cardProfile(theme).format"
                        :show-date="cardProfile(theme).showDate"
                        :show-seconds="cardProfile(theme).showSeconds"
                        :theme="theme"
                        :background-url="themeAssetUrl(theme.id, theme.background.image)"
                        :digit-url="themeAssetUrl(theme.id, theme.digit.image)"
                        :layout="cardProfile(theme).digitLayout"
                        :show-colon="cardProfile(theme).showColon"
                        :font="cardProfile(theme).font"
                        :brightness="cardProfile(theme).brightness"
                        :preview-scale="previewScaleOf(cardProfile(theme).clockScale)"
                        variant="panel"
                      />
                    </span>
                    <span class="theme-sample-name">{{ themeLabel(theme) }}</span>
                    <span class="theme-sample-summary">{{ profileSummary(draft, theme.id) }}</span>
                  </button>
                  <span
                    v-if="draft.appearance.theme === theme.id"
                    class="in-use-badge"
                  >
                    {{ t("studio.inUse") }}
                  </span>
                  <button
                    v-if="!isBuiltinThemeId(theme.id)"
                    type="button"
                    class="theme-edit"
                    @click="editTheme(theme.id)"
                  >
                    {{ t("studio.editTheme") }}
                  </button>
                </div>

                <button type="button" class="theme-new" @click="newTheme">
                  <span class="new-stage"><span class="plus">+</span></span>
                  <span class="theme-name theme-sample-name">{{ t("appearance.newTheme") }}</span>
                </button>
              </div>
              <p class="hint">{{ t("studio.themeHint") }}</p>
              <p v-if="isBuiltinThemeId(draft.appearance.theme)" class="hint">
                {{ t("studio.builtinHint") }}
              </p>
            </template>

            <template v-else>
              <button type="button" class="back" @click="studioEditingThemeId = null">
                ← {{ t("studio.back") }}
              </button>

              <label class="field">
                <span class="field-label">{{ t("appearance.themeName") }}</span>
                <input
                  v-model="editingTheme.name"
                  class="text-input"
                  :class="{ invalid: nameError !== null }"
                  type="text"
                  :maxlength="THEME_NAME_MAX"
                  @input="nameError = null"
                />
              </label>

              <div class="group">
                <div class="group-title">{{ t("appearance.background") }}</div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.background") }}</span>
                  <div class="field-controls">
                    <input v-model="editingTheme.background.color" type="color" class="color-input" />
                    <div class="swatches">
                      <button
                        v-for="color in SWATCHES"
                        :key="color"
                        type="button"
                        class="swatch"
                        :style="{ background: color }"
                        @click="editingTheme.background.color = color"
                      />
                    </div>
                  </div>
                </div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.backgroundImage") }}</span>
                  <div class="field-controls">
                    <button type="button" class="mini" @click="pickImage('background')">
                      {{ t("appearance.pickImage") }}
                    </button>
                    <button
                      v-if="editingTheme.background.image"
                      type="button"
                      class="mini"
                      @click="clearImage('background')"
                    >
                      {{ t("appearance.clearImage") }}
                    </button>
                    <div class="segmented tight">
                      <button
                        v-for="fit in FITS"
                        :key="fit"
                        type="button"
                        :class="{ selected: editingTheme.background.fit === fit }"
                        @click="editingTheme.background.fit = fit"
                      >
                        {{ t(`appearance.fit.${fit}` as MessageKey) }}
                      </button>
                    </div>
                  </div>
                </div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.dim") }}</span>
                  <div class="field-controls">
                    <input
                      v-model.number="editingTheme.background.dim"
                      class="range"
                      type="range"
                      min="0"
                      :max="BACKGROUND_DIM_MAX"
                      :style="rangeFill(editingTheme.background.dim, 0, BACKGROUND_DIM_MAX)"
                      step="5"
                    />
                    <span class="value">{{ Math.round(editingTheme.background.dim) }}%</span>
                  </div>
                </div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.backgroundOpacity") }}</span>
                  <div class="field-controls">
                    <input
                      v-model.number="editingTheme.background.opacity"
                      class="range"
                      type="range"
                      :min="PART_OPACITY_MIN"
                      :max="OPACITY_MAX"
                      :style="rangeFill(editingTheme.background.opacity, PART_OPACITY_MIN, OPACITY_MAX)"
                      step="1"
                    />
                    <span class="value">{{ Math.round(editingTheme.background.opacity) }}%</span>
                  </div>
                </div>
              </div>

              <div class="group">
                <div class="group-title">{{ t("appearance.card") }}</div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.card") }}</span>
                  <div class="field-controls">
                    <input v-model="editingTheme.card.color" type="color" class="color-input" />
                    <div class="swatches">
                      <button
                        v-for="color in SWATCHES"
                        :key="color"
                        type="button"
                        class="swatch"
                        :style="{ background: color }"
                        @click="editingTheme.card.color = color"
                      />
                    </div>
                  </div>
                </div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.materialLabel") }}</span>
                  <div class="segmented">
                    <button
                      v-for="material in MATERIALS"
                      :key="material"
                      type="button"
                      :class="{ selected: editingTheme.card.material === material }"
                      @click="editingTheme.card.material = material"
                    >
                      {{ t(MATERIAL_LABELS[material]) }}
                    </button>
                  </div>
                </div>
                <div v-if="editingTheme.card.material === 'metallic'" class="field">
                  <span class="field-label">{{ t("appearance.lineColor") }}</span>
                  <div class="field-controls">
                    <input
                      :value="editingTheme.card.lineColor ?? editingTheme.card.color"
                      type="color"
                      class="color-input"
                      @input="
                        editingTheme.card.lineColor = ($event.target as HTMLInputElement).value
                      "
                    />
                    <button
                      type="button"
                      class="mini"
                      @click="editingTheme.card.lineColor = undefined"
                    >
                      {{ t("appearance.followColor") }}
                    </button>
                  </div>
                </div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.cardOpacity") }}</span>
                  <div class="field-controls">
                    <input
                      v-model.number="editingTheme.card.opacity"
                      class="range"
                      type="range"
                      :min="CARD_OPACITY_MIN"
                      :max="OPACITY_MAX"
                      :style="rangeFill(editingTheme.card.opacity, CARD_OPACITY_MIN, OPACITY_MAX)"
                      step="1"
                    />
                    <span class="value">{{ Math.round(editingTheme.card.opacity) }}%</span>
                  </div>
                </div>
              </div>

              <div class="group">
                <div class="group-title">{{ t("appearance.digit") }}</div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.digit") }}</span>
                  <div class="field-controls">
                    <input v-model="editingTheme.digit.color" type="color" class="color-input" />
                    <div class="swatches">
                      <button
                        v-for="color in SWATCHES"
                        :key="color"
                        type="button"
                        class="swatch"
                        :style="{ background: color }"
                        @click="editingTheme.digit.color = color"
                      />
                    </div>
                  </div>
                </div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.digitMaterial") }}</span>
                  <div class="segmented">
                    <button
                      v-for="material in MATERIALS"
                      :key="material"
                      type="button"
                      :class="{ selected: editingTheme.digit.material === material }"
                      @click="editingTheme.digit.material = material"
                    >
                      {{ t(MATERIAL_LABELS[material]) }}
                    </button>
                  </div>
                </div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.textureImage") }}</span>
                  <div class="field-controls">
                    <button type="button" class="mini" @click="pickImage('digit')">
                      {{ t("appearance.pickImage") }}
                    </button>
                    <button
                      v-if="editingTheme.digit.image"
                      type="button"
                      class="mini"
                      @click="clearImage('digit')"
                    >
                      {{ t("appearance.clearImage") }}
                    </button>
                  </div>
                </div>
                <div class="field">
                  <span class="field-label" />
                  <div class="field-controls">
                    <span class="hint">{{ t("appearance.textureHint") }}</span>
                  </div>
                </div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.digitOpacity") }}</span>
                  <div class="field-controls">
                    <input
                      v-model.number="editingTheme.digit.opacity"
                      class="range"
                      type="range"
                      :min="PART_OPACITY_MIN"
                      :max="OPACITY_MAX"
                      :style="rangeFill(editingTheme.digit.opacity, PART_OPACITY_MIN, OPACITY_MAX)"
                      step="1"
                    />
                    <span class="value">{{ Math.round(editingTheme.digit.opacity) }}%</span>
                  </div>
                </div>
              </div>

              <div class="group">
                <div class="group-title">{{ t("appearance.date") }}</div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.date") }}</span>
                  <div class="field-controls">
                    <input v-model="editingTheme.date.color" type="color" class="color-input" />
                    <input
                      v-model.number="editingTheme.date.opacity"
                      class="range"
                      type="range"
                      :min="PART_OPACITY_MIN"
                      :max="OPACITY_MAX"
                      :style="rangeFill(editingTheme.date.opacity, PART_OPACITY_MIN, OPACITY_MAX)"
                      step="1"
                    />
                    <span class="value">{{ Math.round(editingTheme.date.opacity) }}%</span>
                  </div>
                </div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.seam") }}</span>
                  <div class="field-controls">
                    <input v-model="editingTheme.seam.color" type="color" class="color-input" />
                    <input
                      v-model.number="editingTheme.seam.opacity"
                      class="range"
                      type="range"
                      :min="PART_OPACITY_MIN"
                      :max="OPACITY_MAX"
                      :style="rangeFill(editingTheme.seam.opacity, PART_OPACITY_MIN, OPACITY_MAX)"
                      step="1"
                    />
                    <span class="value">{{ Math.round(editingTheme.seam.opacity) }}%</span>
                  </div>
                </div>
                <div class="field">
                  <span class="field-label">{{ t("appearance.separator") }}</span>
                  <div class="field-controls">
                    <input v-model="editingTheme.separator.color" type="color" class="color-input" />
                    <input
                      v-model.number="editingTheme.separator.opacity"
                      class="range"
                      type="range"
                      :min="PART_OPACITY_MIN"
                      :max="OPACITY_MAX"
                      :style="rangeFill(editingTheme.separator.opacity, PART_OPACITY_MIN, OPACITY_MAX)"
                      step="1"
                    />
                    <span class="value">{{ Math.round(editingTheme.separator.opacity) }}%</span>
                  </div>
                </div>
                <p v-if="!profile.showColon" class="hint">{{ t("appearance.colonOffHint") }}</p>
              </div>

              <div v-if="draftContrast < 3" class="warn">
                {{ t("appearance.contrastWarning", { ratio: draftContrast.toFixed(1) }) }}
              </div>
              <div v-if="nameError" class="warn">{{ t(nameError) }}</div>
            </template>
          </template>

          <!-- 时钟 -->
          <template v-else-if="studioTab === 'clock'">
            <div class="group">
              <div class="group-title">{{ t("time.format") }}</div>
              <div class="segmented stretch">
                <button
                  type="button"
                  :class="{ selected: profile.format === '24h' }"
                  @click="profile.format = '24h'"
                >
                  {{ t("time.format24") }}
                </button>
                <button
                  type="button"
                  :class="{ selected: profile.format === '12h' }"
                  @click="profile.format = '12h'"
                >
                  {{ t("time.format12") }}
                </button>
              </div>
            </div>

            <div class="group">
              <div class="group-title">{{ t("studio.display") }}</div>
              <div class="row">
                  <span class="row-name">{{ t("time.showDate") }}</span>
                <button
                  type="button"
                  class="switch"
                  role="switch"
                  :aria-checked="profile.showDate"
                  :class="{ on: profile.showDate }"
                  @click="profile.showDate = !profile.showDate"
                >
                  <i></i>
                </button>
              </div>
              <div class="row">
                <span class="row-name">{{ t("time.showSeconds") }}</span>
                <button
                  type="button"
                  class="switch"
                  role="switch"
                  :aria-checked="profile.showSeconds"
                  :class="{ on: profile.showSeconds }"
                  @click="profile.showSeconds = !profile.showSeconds"
                >
                  <i></i>
                </button>
              </div>
              <div class="row">
                <span class="row-name">{{ t("time.showColon") }}</span>
                <button
                  type="button"
                  class="switch"
                  role="switch"
                  :aria-checked="profile.showColon"
                  :class="{ on: profile.showColon }"
                  @click="profile.showColon = !profile.showColon"
                >
                  <i></i>
                </button>
              </div>
            </div>

            <div class="group">
              <div class="group-title">{{ t("time.digitLayout") }}</div>
              <div class="segmented stretch">
                <button
                  type="button"
                  :class="{ selected: profile.digitLayout === 'per-digit' }"
                  @click="profile.digitLayout = 'per-digit'"
                >
                  {{ t("time.layoutPerDigit") }}
                </button>
                <button
                  type="button"
                  :class="{ selected: profile.digitLayout === 'per-pair' }"
                  @click="profile.digitLayout = 'per-pair'"
                >
                  {{ t("time.layoutPerPair") }}
                </button>
              </div>
            </div>

            <div class="group">
              <div class="group-title">{{ t("appearance.fontLabel") }}</div>
              <div class="segmented stretch">
                <button
                  v-for="font in FONTS"
                  :key="font"
                  type="button"
                  :class="{ selected: profile.font === font }"
                  @click="profile.font = font"
                >
                  {{ t(FONT_LABELS[font]) }}
                </button>
              </div>
            </div>

            <div class="group">
              <div class="group-title">{{ t("studio.sizeAndBrightness") }}</div>
              <div class="field">
                <span class="field-label">{{ t("clock.scale") }}</span>
                <div class="field-controls">
                  <input
                    v-model.number="scalePercent"
                    class="range"
                    type="range"
                    :min="SCALE_MIN"
                    :max="SCALE_MAX"
                    :style="rangeFill(scalePercent, SCALE_MIN, SCALE_MAX)"
                    step="1"
                  />
                  <span class="value">{{ scalePercent }}%</span>
                </div>
              </div>
              <div class="field">
                <span class="field-label">{{ t("appearance.brightness") }}</span>
                <div class="field-controls">
                  <input
                    v-model.number="brightnessPercent"
                    class="range"
                    type="range"
                    :min="BRIGHTNESS_MIN_PCT"
                    :max="BRIGHTNESS_MAX_PCT"
                    :style="rangeFill(brightnessPercent, BRIGHTNESS_MIN_PCT, BRIGHTNESS_MAX_PCT)"
                    step="1"
                  />
                  <span class="value">{{ brightnessPercent }}%</span>
                </div>
              </div>
            </div>
          </template>

          <!-- 声音 -->
          <template v-else>
            <div class="group">
              <div class="group-title">{{ t("sound.title") }}</div>
              <div class="row">
                <span class="row-name">{{ t("sound.enable") }}</span>
                <button
                  type="button"
                  class="switch"
                  role="switch"
                  :aria-checked="profile.sound.enabled"
                  :class="{ on: profile.sound.enabled }"
                  @click="profile.sound.enabled = !profile.sound.enabled"
                >
                  <i></i>
                </button>
              </div>
              <div class="field">
                <span class="field-label">{{ t("sound.volume") }}</span>
                <div class="field-controls">
                  <input
                    v-model.number="volumePercent"
                    class="range"
                    type="range"
                    :min="0"
                    :max="100"
                    :style="rangeFill(volumePercent, 0, 100)"
                    step="1"
                  />
                  <span class="value">{{ volumePercent }}%</span>
                </div>
              </div>
              <div class="field">
                <span class="field-label" />
                <div class="field-controls">
                  <button type="button" class="mini" @click="audition">
                    {{ t("sound.audition") }}
                  </button>
                </div>
              </div>
            </div>
          </template>
        </div>

        <footer class="drawer-foot">
          <label class="language">
            <span>{{ t("studio.language") }}</span>
            <select :value="draft.ui.language" @change="setLanguage(($event.target as HTMLSelectElement).value)">
              <option value="system">{{ t("language.system") }}</option>
              <option value="zh">{{ t("language.zh") }}</option>
              <option value="en">{{ t("language.en") }}</option>
            </select>
          </label>
          <div class="actions">
            <button type="button" class="secondary action" @click="requestClose">
              {{ t("studio.discard") }}
            </button>
            <button type="button" class="primary action" @click="save">
              {{ t("studio.save") }}
            </button>
          </div>
        </footer>
      </aside>

      <div v-if="exitPrompt" class="confirm-backdrop" @pointerdown.self="exitPrompt = false">
        <div
          ref="confirmRef"
          class="confirm"
          role="alertdialog"
          aria-modal="true"
          tabindex="-1"
          :aria-label="t('studio.exitTitle')"
        >
          <strong>{{ t("studio.exitTitle") }}</strong>
          <p>{{ t("studio.exitBody") }}</p>
          <div class="confirm-actions">
            <button type="button" class="confirm-choice" @click="onDiscardFromPrompt">
              {{ t("studio.discard") }}
            </button>
            <button type="button" class="confirm-choice primary" @click="onSaveFromPrompt">
              {{ t("studio.save") }}
            </button>
          </div>
          <button type="button" class="confirm-continue" @click="exitPrompt = false">
            {{ t("studio.continue") }}
          </button>
        </div>
      </div>

      <div v-if="toast" class="toast">{{ toast }}</div>
    </div>
  </Teleport>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  z-index: 9000;
}

.drawer {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  z-index: 9010;
  width: 400px;
  display: flex;
  flex-direction: column;
  /* 半透明 + 模糊：钟面仍能透出一点底色，但文字区靠这层底色保证可读。
     深浅两档由令牌给：深色档是暗色渐变，否则就是一块发光的白板压在钟面上。 */
  background: linear-gradient(180deg, var(--ui-drawer-top), var(--ui-drawer-bottom));
  backdrop-filter: blur(22px) saturate(1.1);
  border-left: 1px solid var(--ui-line);
  border-top-left-radius: 18px;
  border-bottom-left-radius: 18px;
  box-shadow: var(--ui-shadow-drawer);
  transition: transform 0.34s cubic-bezier(0.22, 0.8, 0.24, 1), opacity 0.22s ease;
  will-change: transform;
}

.drawer.hidden {
  transform: translateX(102%);
  opacity: 0;
  pointer-events: none;
}

/* 收起时贴在右边缘的把手：既是提示，也是「点一下就展开」的入口。 */
.drawer-peek {
  position: fixed;
  top: 50%;
  right: 0;
  z-index: 9005;
  transform: translateY(-50%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 10px 7px;
  border: 1px solid var(--ui-line);
  border-right: 0;
  border-radius: 10px 0 0 10px;
  background: var(--ui-peek-bg);
  backdrop-filter: blur(14px);
  color: var(--ui-text);
  box-shadow: var(--ui-shadow-peek);
}

.drawer-peek:hover {
  background: var(--ui-surface);
}

.peek-line {
  width: 14px;
  height: 1px;
  background: var(--ui-text-muted);
}

.peek-label {
  writing-mode: vertical-rl;
  text-orientation: upright;
  font-size: 10px;
  letter-spacing: 0.14em;
}

.drawer-head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 16px 10px;
}

.drawer-title {
  font-size: 13px;
  font-weight: 650;
}

.icon {
  margin-left: auto;
  width: 24px;
  height: 24px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--ui-text-secondary);
  font-size: 18px;
  line-height: 1;
}

.tabs {
  /* 与设置页右侧的分段选择完全同一套：深一档轨道 + 纯白选中块 + 极浅投影。 */
  display: flex;
  gap: 1px;
  padding: 2px;
  margin: 0 12px 10px;
  background: var(--ui-track);
  border-radius: var(--radius-control);
}

.tabs button {
  flex: 1;
  border: 0;
  border-radius: 3px;
  padding: 7px 0;
  background: transparent;
  color: var(--ui-text-secondary);
  font-size: 11px;
  transition: background 0.12s ease, color 0.12s ease;
}

.tabs button:hover {
  color: var(--ui-text);
}

.tabs button.selected {
  background: var(--ui-thumb);
  color: var(--ui-text);
  font-weight: 600;
  box-shadow: var(--ui-thumb-shadow);
}

.drawer-body {
  flex: 1;
  overflow-y: auto;
  /* C 稿分组：不编号，靠 17px 上下留白 + 一条极淡的组间线分段。 */
  padding: 8px 22px 24px;
}

/* 主题墙要给缩略钟面让出宽度：左右内边距收到 12px，卡片内边距也收紧。 */
.drawer-body.theme-body {
  padding-left: 12px;
  padding-right: 12px;
}

.group {
  padding: 17px 0;
}

.group + .group {
  border-top: 1px solid var(--ui-divider-soft);
}

.group-title {
  margin: 0 0 12px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}

.field {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.field-label {
  width: 92px;
  flex: none;
  font-size: 11.5px;
  color: var(--ui-text-secondary);
}

.field-controls {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.value {
  width: 38px;
  text-align: right;
  font-size: 10.5px;
  color: var(--ui-text-muted);
  font-variant-numeric: tabular-nums;
}

.hint {
  font-size: 10.5px;
  line-height: 1.5;
  color: var(--ui-text-muted);
}

.text-input {
  flex: 1;
  min-width: 0;
  padding: 6px 9px;
  border-radius: 7px;
  border: 1px solid var(--ui-divider);
  background: var(--ui-surface-3);
  color: var(--ui-text-primary);
  font-size: 12px;
}

.text-input.invalid {
  border-color: var(--ui-warn);
}

.color-input {
  width: 30px;
  height: 24px;
  padding: 0;
  border: 1px solid var(--ui-divider);
  border-radius: 6px;
  background: transparent;
}

.swatches {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}

.swatch {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 1px solid var(--ui-divider);
}

.segmented {
  display: flex;
  gap: 1px;
  padding: 2px;
  border-radius: var(--radius-control);
  background: var(--ui-track);
}

.segmented.tight {
  margin-left: auto;
}

.segmented button {
  border: 0;
  border-radius: 3px;
  padding: 5px 10px;
  background: transparent;
  color: var(--ui-text-secondary);
  font-size: 10.5px;
  white-space: nowrap;
  transition: background 0.12s ease, color 0.12s ease;
}

.segmented button:hover {
  color: var(--ui-text);
}

.segmented button.selected {
  background: var(--ui-thumb);
  color: var(--ui-text);
  font-weight: 600;
  box-shadow: var(--ui-thumb-shadow);
}

.segmented.stretch {
  width: 100%;
}

.segmented.stretch button {
  flex: 1;
}

.range {
  flex: 1;
  min-width: 0;
  /* 与 ui.css 的 .range 保持同一套整数几何：16px 输入 / 4px 轨道 / 12px 钮。
     非整数偏移会让同一页两条轨道被栅格化成不同粗细。 */
  -webkit-appearance: none;
  appearance: none;
  height: 16px;
}

.range::-webkit-slider-runnable-track {
  height: 4px;
  border-radius: 2px;
}

.range::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  margin-top: -4px;
  border-radius: 50%;
}

.mini {
  border: 1px solid var(--ui-divider);
  border-radius: 7px;
  padding: 4px 9px;
  background: var(--ui-surface-3);
  color: var(--ui-text-secondary);
  font-size: 10.5px;
}

.primary {
  border: 1px solid var(--ui-accent);
  border-radius: var(--radius-card);
  padding: 6px 11px;
  background: var(--ui-accent);
  color: var(--ui-accent-ink);
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
  transition: background 0.14s ease, border-color 0.14s ease;
}

.primary:hover {
  background: var(--ui-accent-hover);
  border-color: var(--ui-accent-hover);
}

.secondary {
  border: 1px solid var(--ui-line);
  border-radius: var(--radius-card);
  padding: 6px 11px;
  background: var(--ui-surface);
  color: var(--ui-text);
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
  transition: background 0.14s ease, border-color 0.14s ease;
}

.secondary:hover {
  background: var(--ui-surface-2);
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 9px 0;
}

.group > .row + .row {
  border-top: 1px solid var(--ui-divider);
}

.row-name {
  font-size: 11.5px;
  color: var(--ui-text-secondary);
}

.switch {
  width: var(--ui-switch-w);
  height: var(--ui-switch-h);
  flex: none;
  border: 0;
  padding: var(--ui-switch-pad);
  border-radius: 20px;
  background: var(--ui-switch-off);
  position: relative;
  transition: background 0.18s ease;
}

.switch i {
  position: absolute;
  top: var(--ui-switch-pad);
  left: var(--ui-switch-pad);
  width: var(--ui-knob);
  height: var(--ui-knob);
  border-radius: 50%;
  background: var(--ui-knob-off);
  box-shadow: var(--ui-knob-shadow);
  transition: transform 0.18s ease;
}

.switch.on {
  background: var(--ui-switch-on);
}

.switch.on i {
  background: var(--ui-knob-on);
  transform: translateX(var(--ui-knob-travel));
}

.warn {
  margin-top: 12px;
  padding: 7px 9px;
  border-radius: 7px;
  background: var(--ui-gold-soft);
  color: var(--ui-gold);
  font-size: 10.5px;
  line-height: 1.5;
}

.theme-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
  margin-bottom: 10px;
}

/* 样卡本体（色块、边框、名字、选中环）全在 ui.css 的 .theme-sample，
   与设置页的主题墙共用一份。这里只留抽屉特有的排布：卡片是居中的窄列。
   注意**不能加 overflow: hidden**：选中环画在 .theme-pick 外面，
   一旦被裁就整圈消失——抽屉里会看不出哪张在用。 */
.theme-item {
  position: relative;
  border-radius: 4px;
  /* 悬停放大 3.5%，与设置页主题墙同一手感（同一个数值与时长）。
     抽屉左右各有 12px 内边距，190px 的卡放大 3.5% 只外扩 3.3px，不会被裁。 */
  transition: transform 0.16s ease;
}

.theme-item:hover {
  transform: scale(1.035);
  /* 放大后压在邻卡之上，否则会被后一张盖住半个角。 */
  z-index: 3;
}

.in-use-badge {
  position: absolute;
  top: 7px;
  right: 7px;
  z-index: 2;
  padding: 3px 7px;
  border-radius: 999px;
  background: var(--ui-accent);
  color: var(--ui-accent-ink);
  font-size: 9px;
  font-weight: 600;
  letter-spacing: 0.04em;
}

.theme-pick {
  width: 100%;
  align-items: center;
  text-align: center;
  border: 0;
  background: var(--ui-surface-3);
}

.theme-edit {
  width: 100%;
  border: 0;
  padding: 5px 0;
  background: transparent;
  color: var(--ui-text-secondary);
  font-size: 10px;
}

/* 新建卡：与设置页**逐条一致**（那边是 .new-theme），只有对齐方式沿用抽屉的居中。
   整块浅色料 + 2.3 的钟面位（透明）+ 加号 + 名字，不套边框。 */
.theme-new {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 6px;
  border: 0;
  border-radius: 4px;
  /* 与设置页同一个 token：第二层灰在白底与抽屉的渐变底上都几乎看不见。 */
  background: var(--ui-surface-3);
  color: var(--ui-text-muted);
  transition: color 0.12s ease;
}

.theme-new:hover {
  color: var(--ui-text-secondary);
}

.new-stage {
  display: grid;
  place-items: center;
  width: 100%;
  aspect-ratio: 2.3;
  border-radius: 2px;
  /* 与整块料同色：主题卡那格有色差是因为主题自己有背景色，新建卡没有主题。 */
  background: transparent;
}

.theme-new .plus {
  font-size: 20px;
  line-height: 1;
  opacity: 0.6;
  transition: opacity 0.12s ease;
}

.theme-new:hover .plus {
  opacity: 1;
}

.back {
  border: 0;
  background: transparent;
  color: var(--ui-text-secondary);
  font-size: 11px;
  padding: 2px 0 10px;
}

.drawer-foot {
  border-top: 1px solid var(--ui-divider);
  padding: 10px 16px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.language {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 10.5px;
  color: var(--ui-text-muted);
}

.language select {
  border: 1px solid var(--ui-divider);
  border-radius: 6px;
  background: var(--ui-surface-3);
  color: var(--ui-text-primary);
  font-size: 10.5px;
  padding: 3px 6px;
}

.actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

/* 底部两个操作是同级的：各占一半宽度，别再让「放弃」像个链接。 */
.action {
  width: 100%;
  padding: 9px 0;
  font-size: 11.5px;
}

.confirm-backdrop {
  position: fixed;
  inset: 0;
  z-index: 9020;
  display: grid;
  place-items: center;
  background: var(--ui-scrim);
}

.confirm {
  position: relative;
  width: 380px;
  /* 顶部留白 = 横杠上方 + 横杠高 3px + 横杠下方，上下对称（各 22px）。 */
  padding: 45px 22px 16px;
  border-radius: 14px;
  background: var(--ui-surface-2);
  border: 1px solid var(--ui-divider);
}

/* 横杠与正文同宽（左右对齐 padding），上下各留 22px。 */
.confirm::before {
  content: "";
  position: absolute;
  top: 23px;
  left: 22px;
  right: 22px;
  height: 3px;
  border-radius: 2px;
  background: var(--ui-accent);
}

/* 弹开时焦点落在这个容器上（tabindex=-1）。容器不该显示焦点环，
   否则整个对话框会多一圈墨框。按钮的焦点环保留给真正的 Tab 导航。 */
.confirm:focus {
  outline: none;
}

.confirm strong {
  font-size: 15px;
  color: var(--ui-text);
}

.confirm p {
  margin: 9px 0 18px;
  font-size: 12px;
  line-height: 1.6;
  color: var(--ui-text-secondary);
}

/* 两个选择等权并列：放弃与保存是这一刻真正的决策，
   「继续编辑」降到下方文字链，不再和它们抢视觉重量。 */
.confirm-actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 9px;
}

.confirm-choice {
  padding: 10px 0;
  border: 1px solid var(--ui-divider-strong);
  border-radius: var(--radius-card);
  background: var(--ui-surface);
  color: var(--ui-text-secondary);
  font-size: 12px;
  font-weight: 600;
  transition: border-color 0.14s ease, background 0.14s ease;
}

.confirm-choice:hover {
  border-color: var(--ui-text-muted);
  color: var(--ui-text);
}

/* 必须写成 .confirm-choice.primary：只写 .primary 会被上面的 .confirm-choice
   按相同优先级覆盖掉背景与文字色，「保存」就退化成和「放弃」一样的白底。 */
.confirm-choice.primary {
  border-color: var(--ui-accent);
  background: var(--ui-accent);
  color: var(--ui-accent-ink);
}

.confirm-choice.primary:hover {
  background: var(--ui-accent-hover);
}

.confirm-continue {
  display: block;
  width: 100%;
  margin-top: 10px;
  padding: 6px 0;
  border: 0;
  background: transparent;
  color: var(--ui-text-muted);
  font-size: 11px;
  transition: color 0.14s ease;
}

.confirm-continue:hover {
  color: var(--ui-text);
}

.toast {
  position: fixed;
  left: 50%;
  bottom: 28px;
  transform: translateX(-50%);
  z-index: 9030;
  padding: 8px 14px;
  border-radius: 8px;
  background: var(--ui-overlay-toast-bg);
  color: var(--ui-overlay-toast-ink);
  font-size: 11px;
}
</style>
