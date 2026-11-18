<script setup lang="ts">
import { computed } from "vue";

import FlipClock from "../../components/clock/FlipClock.vue";
import { useClock } from "../../composables/useClock";
import { useSettings } from "../../composables/useSettings";
import {
  CLOCK_FIT_SCALE,
  THEME_LABELS,
  type Theme,
  type TimeFormat,
} from "../../types/settings";

const { now } = useClock();
const { settings } = useSettings();

const THEMES: Theme[] = ["classic-dark", "oled-black", "retro-light"];

/** 与「时间与日期」页同一套标定：100% 只是容器自适应，缩放滑杆要在这里看得出来。 */
const previewScale = computed(() => settings.appearance.clockScale / CLOCK_FIT_SCALE);

function setFormat(format: TimeFormat) {
  settings.time.format = format;
}

function setTheme(theme: Theme) {
  settings.appearance.theme = theme;
}
</script>

<template>
  <div class="workspace">
    <section class="preview-card">
      <div class="card-head">
        <strong>实时效果</strong>
      </div>

      <div class="stage">
        <FlipClock
          :now="now"
          :format="settings.time.format"
          :show-date="settings.time.showDate"
          :show-seconds="settings.time.showSeconds"
          :theme="settings.appearance.theme"
          :layout="settings.appearance.digitLayout"
          :show-colon="settings.time.showColon"
          :font="settings.appearance.font"
          :brightness="settings.appearance.brightness"
          variant="panel"
          :preview-scale="previewScale"
        />
      </div>
    </section>

    <aside class="card settings-card">
      <div class="card-title">快速设置</div>

      <div class="row">
        <div class="row-copy">
          <span class="row-name">时间制式</span>
        </div>
        <div class="segmented">
          <button
            type="button"
            :class="{ selected: settings.time.format === '24h' }"
            @click="setFormat('24h')"
          >
            24H
          </button>
          <button
            type="button"
            :class="{ selected: settings.time.format === '12h' }"
            @click="setFormat('12h')"
          >
            12H
          </button>
        </div>
      </div>

      <div class="row">
        <div class="row-copy">
          <span class="row-name">显示日期与星期</span>
        </div>
        <button
          type="button"
          class="switch"
          role="switch"
          :aria-checked="settings.time.showDate"
          :class="{ on: settings.time.showDate }"
          @click="settings.time.showDate = !settings.time.showDate"
        >
          <i></i>
        </button>
      </div>

      <div class="row">
        <div class="row-copy">
          <span class="row-name">显示秒数</span>
        </div>
        <button
          type="button"
          class="switch"
          role="switch"
          :aria-checked="settings.time.showSeconds"
          :class="{ on: settings.time.showSeconds }"
          @click="settings.time.showSeconds = !settings.time.showSeconds"
        >
          <i></i>
        </button>
      </div>

      <div class="divider"></div>

      <div class="row">
        <div class="row-copy">
          <span class="row-name">显示主题</span>
        </div>
      </div>

      <div class="theme-options">
        <button
          v-for="theme in THEMES"
          :key="theme"
          type="button"
          class="theme-option"
          :class="{ active: settings.appearance.theme === theme }"
          :data-clock-theme="theme"
          @click="setTheme(theme)"
        >
          <span class="theme-swatch">
            <span class="theme-card"><span>1</span></span>
            <span class="theme-card"><span>2</span></span>
          </span>
          <small>{{ THEME_LABELS[theme] }}</small>
        </button>
      </div>
    </aside>
  </div>
</template>

<style scoped>
.workspace {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1.45fr) minmax(262px, 0.82fr);
  gap: 16px;
}

.preview-card {
  display: flex;
  flex-direction: column;
  min-height: 0;
  background-color: var(--ui-surface);
  background-image: var(--ui-sheen);
  border: 1px solid rgba(255, 255, 255, 0.72);
  border-radius: var(--radius-panel);
  box-shadow: var(--ui-card-shadow);
  overflow: hidden;
}

.card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 14px 16px 10px;
  font-size: 12px;
}

.stage {
  position: relative;
  /* 时钟本身是深色的，浅色界面里整块铺满会显得很重；
     收成一条有固定比例的「屏幕」，上下留白，视觉重量就对了。 */
  flex: 0 1 auto;
  height: clamp(180px, 54%, 330px);
  min-height: 180px;
  margin: auto 14px;
  border: 1px solid var(--ui-divider);
  border-radius: var(--radius-card);
  overflow: hidden;
  /* 让内部的翻页卡按容器宽高缩放（cqw / cqh） */
  container-type: size;
}

.settings-card {
  min-height: 0;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--ui-scroll) transparent;
}

.theme-options {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 7px;
}

.theme-option {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 6px 6px 7px;
  border: 1px solid var(--ui-divider);
  border-radius: 9px;
  background: var(--ui-bg);
  text-align: left;
  transition: 0.16s ease;
}

.theme-option.active {
  border-color: var(--ui-accent-line-strong);
  box-shadow: 0 0 0 1px var(--ui-accent-halo);
}

.theme-option small {
  font-size: 9px;
  color: var(--ui-text-secondary);
  white-space: nowrap;
}

.theme-swatch {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  height: 34px;
  border-radius: 5px;
  background: var(--clock-bg);
}

.theme-card {
  display: grid;
  place-items: center;
  width: 18px;
  height: 25px;
  border-radius: 3px;
  background: linear-gradient(180deg, var(--card-top), var(--card-bottom));
  overflow: hidden;
}

.theme-card span {
  font-family: var(--font-digit);
  font-size: 15px;
  line-height: 1;
  color: var(--digit);
}
</style>
