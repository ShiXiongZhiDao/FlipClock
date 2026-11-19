<script setup lang="ts">
import { computed, ref } from "vue";

import { useSettings } from "../../composables/useSettings";
import { playFlipSound } from "../../lib/flipSound";
import { t } from "../../lib/i18n";
import { SOUND_VOLUME_MAX, SOUND_VOLUME_MIN } from "../../types/settings";

const { settings, toggleSound } = useSettings();

const VOLUME_MIN = Math.round(SOUND_VOLUME_MIN * 100);
const VOLUME_MAX = Math.round(SOUND_VOLUME_MAX * 100);

const volumePercent = computed({
  get: () => Math.round(settings.sound.volume * 100),
  set: (value: number) => {
    settings.sound.volume = value / 100;
    // 拖到 0 就是静音；拖回非零顺手把开关打开，避免出现「有音量却没声」。
    settings.sound.enabled = value > 0;
  },
});

/** 试听失败要说出来，不能点了没反应也不解释。 */
const problem = ref<string | null>(null);

function audition() {
  problem.value = null;
  const result = playFlipSound(settings.sound.volume);
  if (result === "unsupported") problem.value = t("sound.unsupported");
  else if (result === "muted") problem.value = t("sound.muted");
}
</script>

<template>
  <div class="sound-page">
    <section class="card sound-card">
      <div class="card-title">{{ t("sound.title") }}</div>

      <div class="row">
        <div class="row-copy">
          <span class="row-name">{{ t("sound.enable") }}</span>
        </div>
        <button
          type="button"
          class="switch"
          role="switch"
          :aria-checked="settings.sound.enabled"
          :class="{ on: settings.sound.enabled }"
          @click="toggleSound"
        >
          <i></i>
        </button>
      </div>

      <div class="divider"></div>

      <div class="volume-block" :class="{ dim: !settings.sound.enabled }">
        <div class="volume-head">
          <span class="row-name">{{ t("sound.volume") }}</span>
          <span class="volume-value">{{ volumePercent }}%</span>
        </div>
        <input
          v-model.number="volumePercent"
          class="range"
          type="range"
          :min="VOLUME_MIN"
          :max="VOLUME_MAX"
          step="1"
          :disabled="!settings.sound.enabled"
          :aria-label="t('sound.volumeAria')"
        />
      </div>

      <div class="audition">
        <button type="button" class="secondary" @click="audition">{{ t("sound.audition") }}</button>
        <span class="audition-note" :class="{ warn: !!problem }">
          {{ problem ?? t("sound.stageOnly") }}
        </span>
      </div>
    </section>
  </div>
</template>

<style scoped>
.sound-page {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: flex-start;
}

/* 只有三项设置，卡片按内容收高、靠上摆放；
   撑满整页会变成一块大面积空白，看起来像少了东西。 */
.sound-card {
  flex: none;
  width: 100%;
  gap: 16px;
}

.volume-block {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.volume-block.dim {
  opacity: 0.45;
}

.volume-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
}

.volume-value {
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

.range:disabled {
  cursor: not-allowed;
}

.audition {
  display: flex;
  align-items: center;
  gap: 12px;
  padding-top: 14px;
  border-top: 1px solid var(--ui-divider);
}

.audition-note {
  font-size: 10px;
  color: var(--ui-text-muted);
}

.audition-note.warn {
  color: var(--ui-warn);
}
</style>
