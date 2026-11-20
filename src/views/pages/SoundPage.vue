<script setup lang="ts">
import { computed, ref } from "vue";

import { useSettings } from "../../composables/useSettings";
import { useTheme } from "../../composables/useTheme";
import { playFlipSound } from "../../lib/flipSound";
import { t } from "../../lib/i18n";
import { themeLabel } from "../../lib/theme/labels";
import { setSoundVolume } from "../../lib/theme/profile";
import { SOUND_VOLUME_MAX, SOUND_VOLUME_MIN } from "../../types/settings";

const { toggleSound } = useSettings();
const { current: currentTheme, profile } = useTheme();

const VOLUME_MIN = Math.round(SOUND_VOLUME_MIN * 100);
const VOLUME_MAX = Math.round(SOUND_VOLUME_MAX * 100);

const volumePercent = computed({
  get: () => Math.round(profile.value.sound.volume * 100),
  set: (value: number) => {
    setSoundVolume(profile.value, value);
  },
});

/** 试听失败要说出来，不能点了没反应也不解释。 */
const problem = ref<string | null>(null);

function audition() {
  problem.value = null;
  const result = playFlipSound(profile.value.sound.volume);
  if (result === "unsupported") problem.value = t("sound.unsupported");
  else if (result === "muted") problem.value = t("sound.muted");
}
</script>

<template>
  <div class="sound-page">
    <section class="section">
      <div class="section-head">
        <span class="section-title">{{ t("sound.title") }}</span>
        <span class="section-note">{{ t("scope.currentTheme", { name: themeLabel(currentTheme) }) }}</span>
      </div>

      <div class="lines">
        <div class="row">
          <div class="row-copy">
            <span class="row-name">{{ t("sound.enable") }}</span>
            <span class="row-hint">{{ t("sound.stageOnly") }}</span>
          </div>
          <button
            type="button"
            class="switch"
            role="switch"
            :aria-checked="profile.sound.enabled"
            :class="{ on: profile.sound.enabled }"
            @click="toggleSound"
          >
            <i></i>
          </button>
        </div>

        <div class="row volume-row" :class="{ dim: !profile.sound.enabled }">
          <div class="range-head">
            <span class="row-name">{{ t("sound.volume") }}</span>
            <span class="range-value">{{ volumePercent }}%</span>
          </div>
          <input
            v-model.number="volumePercent"
            class="range"
            type="range"
            :min="VOLUME_MIN"
            :max="VOLUME_MAX"
            step="1"
            :style="{ '--fill': `${volumePercent}%` }"
            :disabled="!profile.sound.enabled"
            :aria-label="t('sound.volumeAria')"
          />
        </div>
      </div>
    </section>

    <section class="section audition">
      <span class="audition-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24">
          <path d="M4 10v4h3l5 4V6l-5 4H4Z" />
          <path d="M16 9a4 4 0 0 1 0 6M18.5 6.5a7.5 7.5 0 0 1 0 11" />
        </svg>
      </span>
      <span class="audition-copy">
        <strong>{{ t("sound.auditionTitle") }}</strong>
        <small :class="{ warn: !!problem }">{{ problem ?? t("sound.auditionHint") }}</small>
      </span>
      <button type="button" class="secondary" @click="audition">{{ t("sound.audition") }}</button>
    </section>
  </div>
</template>

<style scoped>
.sound-page {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.section-note {
  font-size: 9.5px;
  color: var(--ui-text-muted);
  max-width: 60%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 音量行是「标签 + 数值 + 滑杆」的竖排，不能沿用 .lines > .row 的横排。 */
.volume-row {
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
  padding: 10px 0 4px;
}

.volume-row.dim {
  opacity: 0.45;
}

.audition {
  flex-direction: row;
  align-items: center;
  gap: 11px;
}

.audition-icon {
  flex: none;
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  border-radius: 4px;
  background: var(--ui-surface-3);
  color: var(--ui-text-secondary);
}

.audition-icon svg {
  width: 18px;
  height: 18px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.7;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.audition-copy {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.audition-copy strong {
  font-size: 11.5px;
  font-weight: 620;
  color: var(--ui-text);
}

.audition-copy small {
  font-size: 10px;
  color: var(--ui-text-muted);
}

.audition-copy small.warn {
  color: var(--ui-warn);
}
</style>
