<script setup lang="ts">
import FlipClock from "../../components/clock/FlipClock.vue";
import { useClock } from "../../composables/useClock";
import { useClockScale } from "../../composables/useClockScale";
import { useSettings } from "../../composables/useSettings";
import { t } from "../../lib/i18n";
import type { DigitLayout, TimeFormat } from "../../types/settings";

const { now } = useClock();
const { settings } = useSettings();
const { scalePercent, previewScale, SCALE_MIN, SCALE_MAX, SCALE_STEP } = useClockScale();

function setFormat(format: TimeFormat) {
  settings.time.format = format;
}

function setLayout(layout: DigitLayout) {
  settings.appearance.digitLayout = layout;
}
</script>

<template>
  <div class="time-page">
    <section class="sample-card">
      <div class="card-head">
        <strong>{{ t("time.currentStyle") }}</strong>
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

    <aside class="card">
      <div class="card-title">{{ t("time.format") }}</div>
      <div class="segmented stretch">
        <button
          type="button"
          :class="{ selected: settings.time.format === '24h' }"
          @click="setFormat('24h')"
        >
          {{ t("time.format24") }}
        </button>
        <button
          type="button"
          :class="{ selected: settings.time.format === '12h' }"
          @click="setFormat('12h')"
        >
          {{ t("time.format12") }}
        </button>
      </div>

      <div class="divider"></div>

      <div class="row">
        <div class="row-copy">
          <span class="row-name">{{ t("time.showDate") }}</span>
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
          <span class="row-name">{{ t("time.showSeconds") }}</span>
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

      <div class="row">
        <div class="row-copy">
          <span class="row-name">{{ t("time.showColon") }}</span>
        </div>
        <button
          type="button"
          class="switch"
          role="switch"
          :aria-checked="settings.time.showColon"
          :class="{ on: settings.time.showColon }"
          @click="settings.time.showColon = !settings.time.showColon"
        >
          <i></i>
        </button>
      </div>

      <div class="divider"></div>

      <div class="layout-block">
        <span class="row-name">{{ t("time.digitLayout") }}</span>
        <div class="segmented stretch">
          <button
            type="button"
            :class="{ selected: settings.appearance.digitLayout === 'per-digit' }"
            @click="setLayout('per-digit')"
          >
            {{ t("time.layoutPerDigit") }}
          </button>
          <button
            type="button"
            :class="{ selected: settings.appearance.digitLayout === 'per-pair' }"
            @click="setLayout('per-pair')"
          >
            {{ t("time.layoutPerPair") }}
          </button>
        </div>
      </div>

      <div class="scale-block">
        <div class="scale-head">
          <span class="row-name">{{ t("clock.scale") }}</span>
          <span class="scale-value">{{ scalePercent }}%</span>
        </div>
        <input
          v-model.number="scalePercent"
          class="range"
          type="range"
          :min="SCALE_MIN"
          :max="SCALE_MAX"
          :step="SCALE_STEP"
          :aria-label="t('clock.scaleAria')"
        />
      </div>

    </aside>
  </div>
</template>

<style scoped>
.time-page {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1.25fr) minmax(270px, 1fr);
  gap: 16px;
}

.sample-card {
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
  flex: 0 1 auto;
  height: clamp(180px, 54%, 330px);
  min-height: 180px;
  margin: auto 14px;
  border: 1px solid var(--ui-divider);
  border-radius: var(--radius-card);
  overflow: hidden;
  container-type: size;
}

.card {
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--ui-scroll) transparent;
}

.layout-block {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.scale-block {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.scale-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
}

.scale-value {
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
</style>
