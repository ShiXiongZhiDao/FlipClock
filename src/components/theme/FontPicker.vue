<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, watch, type ComponentPublicInstance } from "vue";

import { t } from "../../lib/i18n";
import {
  ensureFontLoaded,
  fontMetrics,
  fontSampleStyle,
  fontState,
} from "../../lib/fonts";
import {
  BUILTIN_FONTS,
  FONT_LABELS,
  isBuiltinFontId,
} from "../../lib/fontRegistry";
import type { CustomFont } from "../../types/settings";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    /** 全局字体库里的导入字体；内置三款由组件自己补。 */
    fonts: CustomFont[];
    /** 设置窗口为 true：显示改名/删除；全屏配置只选不管。 */
    manage?: boolean;
  }>(),
  { manage: false },
);

const emit = defineEmits<{
  (event: "update:modelValue", value: string): void;
  (event: "load-failed", font: CustomFont): void;
  (event: "rename", font: CustomFont): void;
  (event: "remove", font: CustomFont): void;
}>();

const tileEls = new Map<string, HTMLElement>();
let observer: IntersectionObserver | null = null;

function setTileEl(
  id: string,
  el: Element | ComponentPublicInstance | null,
): void {
  const node = el instanceof HTMLElement ? el : null;
  if (!node) {
    tileEls.delete(id);
    return;
  }
  tileEls.set(id, node);
  observer?.observe(node);
}

function scrollActive(): void {
  void nextTick(() => {
    tileEls.get(props.modelValue)?.scrollIntoView({ block: "nearest" });
  });
}

function warning(font: CustomFont): string {
  const state = fontState(font.id);
  if (state === "failed" || state === "missing") return t("appearance.fontMissing");
  if (fontMetrics(font.id)?.uneven) return t("appearance.fontUneven");
  return "";
}

/**
 * 选中前先确认字体可用：加载期间当前字体继续显示，不先改 profile.font，
 * 避免闪一帧 Bebas；失败保持当前字体并交给父层 toast。
 */
async function choose(id: string): Promise<void> {
  if (props.modelValue === id) return;
  if (isBuiltinFontId(id)) {
    emit("update:modelValue", id);
    return;
  }
  const entry = props.fonts.find((font) => font.id === id);
  if (!entry) return;
  if (await ensureFontLoaded(entry)) {
    emit("update:modelValue", id);
  } else {
    emit("load-failed", entry);
  }
}

onMounted(() => {
  if (typeof IntersectionObserver !== "function") {
    for (const font of props.fonts) void ensureFontLoaded(font);
    scrollActive();
    return;
  }
  observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const id = (entry.target as HTMLElement).dataset.fontId;
        const font = id ? props.fonts.find((item) => item.id === id) : undefined;
        if (font) void ensureFontLoaded(font);
      }
    },
    { rootMargin: "48px" },
  );
  for (const el of tileEls.values()) observer.observe(el);
  scrollActive();
});

onBeforeUnmount(() => observer?.disconnect());

watch(() => props.modelValue, scrollActive);
</script>

<template>
  <div class="font-picker">
    <section class="font-group">
      <div class="font-group-label">{{ t("appearance.fontGroupBuiltin") }}</div>
      <div class="font-grid">
        <div v-for="font in BUILTIN_FONTS" :key="font" class="font-tile">
          <button
            type="button"
            class="font-choice"
            :class="{ active: modelValue === font }"
            @click="choose(font)"
          >
            <span class="font-sample" :style="fontSampleStyle(font)" aria-hidden="true">
              <span>08</span>
              <i class="sample-dots"><b></b><b></b></i>
              <span>38</span>
            </span>
            <small class="font-name">{{ t(FONT_LABELS[font]) }}</small>
            <span v-if="modelValue === font" class="font-check" aria-hidden="true">
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M3 8.5l3 3 7-7" />
              </svg>
            </span>
          </button>
        </div>
      </div>
    </section>

    <section v-if="fonts.length" class="font-group">
      <div class="font-group-label">{{ t("appearance.fontGroupImported") }}</div>
      <div class="font-grid">
        <div
          v-for="font in fonts"
          :key="font.id"
          :ref="(el) => setTileEl(font.id, el)"
          class="font-tile"
          :data-font-id="font.id"
        >
          <button
            type="button"
            class="font-choice"
            :class="{ active: modelValue === font.id }"
            @click="choose(font.id)"
          >
            <span class="font-sample" :style="fontSampleStyle(font.id)" aria-hidden="true">
              <span>08</span>
              <i class="sample-dots"><b></b><b></b></i>
              <span>38</span>
            </span>
            <small class="font-name" :title="font.name">{{ font.name }}</small>
            <small v-if="warning(font)" class="font-status">{{ warning(font) }}</small>
            <span v-if="modelValue === font.id" class="font-check" aria-hidden="true">
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M3 8.5l3 3 7-7" />
              </svg>
            </span>
          </button>

          <span
            v-if="warning(font)"
            class="font-warning"
            :title="warning(font)"
            :aria-label="warning(font)"
          >
            !
          </span>

          <div v-if="manage" class="font-actions">
            <button
              type="button"
              :title="t('appearance.fontRename')"
              :aria-label="t('appearance.fontRename')"
              @click.stop="emit('rename', font)"
            >
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4">
                <path d="M11.2 2.4l2.4 2.4-8 8H3.2v-2.4z" />
              </svg>
            </button>
            <button
              type="button"
              class="danger"
              :title="t('appearance.fontRemove')"
              :aria-label="t('appearance.fontRemove')"
              @click.stop="emit('remove', font)"
            >
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4">
                <path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8h5.8l.6-8" />
              </svg>
            </button>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.font-picker {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
}

.font-group {
  display: flex;
  flex-direction: column;
  gap: 5px;
  min-width: 0;
}

.font-group-label {
  font-size: 9px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--ui-text-muted);
}

.font-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
  grid-auto-rows: 72px;
  gap: 5px;
  /* 精确 3 行：3 × 72 + 2 × 5 间距 + 2px 内边距。 */
  max-height: 228px;
  overflow-y: auto;
  padding: 1px 2px 1px 1px;
}

.font-tile {
  position: relative;
  min-width: 0;
}

.font-choice {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  width: 100%;
  height: 100%;
  padding: 6px 3px 5px;
  border: 1px solid var(--ui-divider);
  border-radius: 4px;
  background: var(--ui-surface);
  color: var(--ui-text-muted);
  transition:
    border-color 0.12s ease,
    color 0.12s ease,
    box-shadow 0.12s ease;
}

.font-choice:hover {
  border-color: var(--ui-line-hover);
  color: var(--ui-text-secondary);
}

.font-choice.active {
  border-color: var(--ui-accent);
  box-shadow: inset 0 0 0 1px var(--ui-accent);
  color: var(--ui-text);
}

.font-sample {
  display: inline-flex;
  align-items: center;
  gap: 0.16em;
  max-width: 100%;
  font-family: var(--font-clock, var(--font-ui));
  font-weight: var(--digit-weight, 400);
  letter-spacing: var(--digit-spacing, -0.02em);
  font-size: 20px;
  line-height: 1.15;
  color: var(--ui-text-secondary);
}

.sample-dots {
  display: inline-flex;
  flex-direction: column;
  gap: 0.16em;
  margin: 0 0.04em;
}

.sample-dots b {
  display: block;
  width: 0.14em;
  height: 0.14em;
  border-radius: 50%;
  background: currentColor;
}

.font-choice.active .font-sample {
  color: var(--ui-text);
}

.font-name {
  max-width: 100%;
  font-family: var(--font-ui);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 9px;
}

.font-status {
  max-width: 100%;
  font-family: var(--font-ui);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 8px;
  line-height: 1.1;
  color: var(--ui-accent);
}

.font-check {
  position: absolute;
  top: 3px;
  right: 3px;
  width: 12px;
  height: 12px;
  color: var(--ui-accent);
}

.font-check svg {
  width: 12px;
  height: 12px;
}

.font-warning {
  position: absolute;
  top: 3px;
  left: 3px;
  display: grid;
  place-items: center;
  width: 13px;
  height: 13px;
  border-radius: 50%;
  background: var(--ui-accent);
  color: var(--ui-surface);
  font-size: 9px;
  font-weight: 700;
  cursor: help;
}

.font-actions {
  position: absolute;
  right: 3px;
  bottom: 3px;
  display: flex;
  gap: 2px;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.12s ease;
}

.font-tile:hover .font-actions,
.font-tile:focus-within .font-actions {
  opacity: 1;
  pointer-events: auto;
}

.font-actions button {
  display: grid;
  place-items: center;
  width: 18px;
  height: 18px;
  padding: 0;
  border: 0;
  border-radius: 3px;
  background: var(--ui-thumb);
  color: var(--ui-text-secondary);
  cursor: pointer;
}

.font-actions button:hover {
  color: var(--ui-text);
}

.font-actions button.danger:hover {
  color: var(--ui-danger, #d05a5a);
}

.font-actions svg {
  width: 11px;
  height: 11px;
}
</style>