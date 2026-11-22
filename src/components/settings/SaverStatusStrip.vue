<script setup lang="ts">
import { computed } from "vue";

import { t } from "../../lib/i18n";
import { SAVER_STATE_UI, saverUiState, saverWindowsOff } from "../../lib/saverStatus";
import type { ScreenSaverStatus } from "../../types/settings";

const props = defineProps<{
  status: ScreenSaverStatus | null;
  error: boolean;
  busy: boolean;
  justInstalled: boolean;
}>();

const emit = defineEmits<{
  (e: "install"): void;
  (e: "retry"): void;
  (e: "details"): void;
  (e: "open-settings"): void;
}>();

const uiState = computed(() => saverUiState(props.status));
const meta = computed(() => (uiState.value ? SAVER_STATE_UI[uiState.value] : null));
const ready = computed(() => uiState.value === "ready");
const compact = computed(() => ready.value);

const tone = computed(() => (props.error ? "err" : (meta.value?.tone ?? "idle")));

const title = computed((): string => {
  if (props.error) return t("saver.statusReadError");
  return meta.value?.stripTitle ? t(meta.value.stripTitle) : "";
});

const action = computed((): string | null => {
  if (props.error) return null;
  return meta.value?.stripAction ? t(meta.value.stripAction) : null;
});

const note = computed((): string | null => {
  if (props.error) return null;
  return meta.value?.note ? t(meta.value.note) : null;
});

const windowsOff = computed(() => !props.error && saverWindowsOff(props.status));
</script>

<template>
  <section
    v-if="(status || error) && (!ready || justInstalled)"
    class="saver-strip"
    :class="{ compact }"
    aria-live="polite"
  >
    <div v-if="!ready" class="strip-main">
      <span class="badge strip-status" :class="tone === 'idle' ? undefined : tone">{{ title }}</span>
      <button
        v-if="action"
        type="button"
        class="primary strip-action"
        :disabled="busy"
        @click="emit('install')"
      >
        {{ action }}
      </button>
      <button v-else-if="error" type="button" class="strip-link" @click="emit('retry')">
        {{ t("saver.retry") }}
      </button>
      <button
        v-if="!error"
        type="button"
        class="strip-link strip-detail"
        @click="emit('details')"
      >
        {{ t("saver.strip.details") }}
      </button>
    </div>

    <p v-if="note" class="strip-note">{{ note }}</p>

    <p v-if="windowsOff" class="strip-note">
      {{ t("saver.strip.windowsOff") }} ·
      <button type="button" class="strip-link" @click="emit('open-settings')">
        {{ t("saver.enableWindows") }}
      </button>
    </p>

    <p v-if="!error && justInstalled && ready" class="strip-note">
      {{ t("saver.waitHint") }} ·
      <button type="button" class="strip-link" @click="emit('open-settings')">
        {{ t("saver.open") }}
      </button>
    </p>
  </section>
</template>

<style scoped>
/* 与左栏其他段落同一种排版：只留一条上分隔线，不做卡片。 */
.saver-strip {
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 1px 0;
  margin-bottom: 12px;
  border-top: 1px solid var(--ui-divider-strong);
}

/* 已就绪时收成一行灰字：没有要用户做的事，就不该继续占一段。 */
.saver-strip.compact {
  padding: 0 1px;
  margin-bottom: 10px;
  border-top-color: transparent;
}

.strip-main {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.strip-status {
  flex: 1;
  min-width: 0;
  white-space: normal;
  line-height: 1.4;
}

.strip-action {
  flex: none;
  padding: 4px 9px;
  font-size: 10px;
}

.strip-detail {
  flex: none;
  margin-left: auto;
}

.strip-note {
  margin: 0;
  font-size: 9.5px;
  line-height: 1.5;
  color: var(--ui-text-muted);
}

.strip-link {
  padding: 0;
  font-size: inherit;
  color: var(--ui-text-secondary);
  text-decoration: underline;
  text-decoration-color: var(--ui-link-line);
  text-underline-offset: 3px;
  transition: color 0.12s ease, text-decoration-color 0.12s ease;
}

.strip-link:hover {
  color: var(--ui-text);
  text-decoration-color: var(--ui-link-line-hover);
}
</style>