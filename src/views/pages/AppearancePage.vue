<script setup lang="ts">
import { computed } from "vue";

import FlipClock from "../../components/clock/FlipClock.vue";
import { useClock } from "../../composables/useClock";
import { useSettings } from "../../composables/useSettings";
import {
  BRIGHTNESS_MAX,
  BRIGHTNESS_MIN,
  FONT_LABELS,
  GLASS_TRANSPARENCY_MAX,
  GLASS_TRANSPARENCY_MIN,
  THEME_LABELS,
  type DigitFont,
  type Theme,
} from "../../types/settings";

const { settings } = useSettings();
/** 三行主题预览共用同一个实时时刻：既走时，又保持配色可比。 */
const { now } = useClock();

const THEMES: Theme[] = ["classic-dark", "oled-black", "retro-light"];
const FONTS: DigitFont[] = ["bebas", "bahnschrift", "system"];

const GLASS_MIN = Math.round(GLASS_TRANSPARENCY_MIN * 100);
const GLASS_MAX = Math.round(GLASS_TRANSPARENCY_MAX * 100);

/** 亮度只压暗：界面用 20–100%，配置存 0.2–1.0。 */
const brightnessPercent = computed({
  get: () => Math.round(settings.appearance.brightness * 100),
  set: (value: number) => {
    settings.appearance.brightness = value / 100;
  },
});
const BRIGHTNESS_MIN_PERCENT = Math.round(BRIGHTNESS_MIN * 100);
const BRIGHTNESS_MAX_PERCENT = Math.round(BRIGHTNESS_MAX * 100);

/** 配置里存 0–1 的透明度，界面用百分比；拖动即改，保存由 useSettings 防抖。 */
const transparency = computed({
  get: () => Math.round(settings.appearance.glass.transparency * 100),
  set: (value: number) => {
    settings.appearance.glass.transparency = value / 100;
  },
});
</script>

<template>
  <div class="appearance">
    <div class="theme-rows">
      <button
        v-for="theme in THEMES"
        :key="theme"
        type="button"
        class="theme-row"
        :class="{ active: settings.appearance.theme === theme }"
        @click="settings.appearance.theme = theme"
      >
        <div class="theme-stage" :data-clock-theme="theme">
          <FlipClock
            :now="now"
            format="24h"
            :show-date="false"
            :show-seconds="false"
            :theme="theme"
            :layout="settings.appearance.digitLayout"
            :show-colon="settings.time.showColon"
            :font="settings.appearance.font"
            :brightness="settings.appearance.brightness"
            variant="panel"
          />
        </div>

        <div class="theme-meta">
          <div class="theme-name">
            {{ THEME_LABELS[theme] }}
            <span v-if="settings.appearance.theme === theme" class="tick">✓ 使用中</span>
          </div>
        </div>
      </button>
    </div>

    <div class="card clock-style-card">
      <div class="card-title">数字外观</div>

      <div class="font-options">
        <button
          v-for="font in FONTS"
          :key="font"
          type="button"
          class="font-option"
          :class="{ active: settings.appearance.font === font }"
          :data-clock-font="font"
          @click="settings.appearance.font = font"
        >
          <span class="font-sample">12</span>
          <small>{{ FONT_LABELS[font] }}</small>
        </button>
      </div>

      <div class="divider"></div>

      <div class="brightness-block">
        <div class="brightness-head">
          <span class="row-name">亮度</span>
          <span class="brightness-value">{{ brightnessPercent }}%</span>
        </div>
        <input
          v-model.number="brightnessPercent"
          class="range"
          type="range"
          :min="BRIGHTNESS_MIN_PERCENT"
          :max="BRIGHTNESS_MAX_PERCENT"
          step="5"
          aria-label="时钟亮度"
        />
      </div>
    </div>

    <div class="card glass-card">
      <div class="glass-head">
        <span class="card-title">界面玻璃</span>
        <span class="glass-value">{{ transparency }}% 透</span>
      </div>
      <input
        v-model.number="transparency"
        class="glass-range"
        type="range"
        :min="GLASS_MIN"
        :max="GLASS_MAX"
        step="1"
        aria-label="界面玻璃透明度"
      />
    </div>
  </div>
</template>

<style scoped>
.appearance {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--ui-scroll) transparent;
}

.theme-rows {
  flex: none;
  display: grid;
  grid-template-rows: repeat(3, minmax(88px, 1fr));
  gap: 10px;
}

.theme-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 214px;
  gap: 14px;
  padding: 9px;
  min-height: 0;
  border: 1px solid rgba(255, 255, 255, 0.72);
  border-radius: var(--radius-panel);
  background-color: var(--ui-surface);
  background-image: var(--ui-sheen);
  box-shadow: var(--ui-card-shadow);
  text-align: left;
  transition: border-color 0.16s ease, box-shadow 0.16s ease;
}

.theme-row:hover {
  border-color: var(--ui-accent-line);
}

.theme-row.active {
  border-color: var(--ui-accent-line-strong);
  box-shadow: 0 0 0 1px var(--ui-accent-halo);
}

.theme-stage {
  position: relative;
  min-height: 88px;
  border-radius: var(--radius-card);
  overflow: hidden;
  border: 1px solid var(--ui-divider);
  container-type: size;
}

.theme-meta {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 6px;
  padding-right: 6px;
  min-width: 0;
}

.theme-name {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12.5px;
  font-weight: 650;
}

.tick {
  font-size: 9px;
  font-weight: 600;
  color: var(--ui-accent);
  letter-spacing: 0.08em;
}

.glass-card {
  flex: none;
  gap: 9px;
  padding: 14px 18px;
}

.clock-style-card {
  flex: none;
  gap: 11px;
  padding: 14px 18px;
}

.font-options {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.font-option {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 8px 10px;
  border: 1px solid var(--ui-divider);
  border-radius: 9px;
  background: var(--ui-bg);
  transition: border-color 0.16s ease, box-shadow 0.16s ease;
}

.font-option:hover {
  border-color: var(--ui-accent-line);
}

.font-option.active {
  border-color: var(--ui-accent-line-strong);
  box-shadow: 0 0 0 1px var(--ui-accent-halo);
}

/* 每一格用自己的字体显示同样的两个字，选之前就能看出字宽差异。 */
.font-sample {
  font-family: var(--font-clock);
  font-weight: var(--digit-weight, 400);
  letter-spacing: var(--digit-spacing, -0.02em);
  font-size: 26px;
  line-height: 1.1;
  color: var(--ui-text);
}

.font-option small {
  font-size: 9px;
  color: var(--ui-text-secondary);
}

.brightness-block {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.brightness-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
}

.brightness-value {
  font-size: 10px;
  color: var(--ui-text-muted);
  font-variant-numeric: tabular-nums;
}

.range {
  width: 100%;
  height: 18px;
  margin: 0;
  accent-color: var(--ui-accent);
  cursor: default;
}

.glass-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.glass-value {
  font-size: 10px;
  color: var(--ui-text-muted);
  font-variant-numeric: tabular-nums;
}

.glass-range {
  width: 100%;
  height: 18px;
  margin: 0;
  accent-color: var(--ui-accent);
  cursor: default;
}
</style>
