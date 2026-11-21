<script setup lang="ts">
import { computed } from "vue";

import { fontSampleStyle } from "../../lib/fonts";

const props = defineProps<{ fontId: string }>();

const sampleStyle = computed(() => fontSampleStyle(props.fontId));
</script>

<template>
  <span class="font-sample" :style="sampleStyle" aria-hidden="true">
    <span>08</span>
    <i class="sample-dots"><b></b><b></b></i>
    <span>38</span>
  </span>
</template>

<style scoped>
.font-sample {
  display: inline-flex;
  align-items: center;
  gap: 0.16em;
  max-width: 100%;
  font-family: var(--font-clock, var(--font-ui));
  font-weight: var(--digit-weight, 400);
  letter-spacing: var(--digit-spacing, -0.02em);
  font-size: var(--font-sample-size, 20px);
  line-height: var(--font-sample-line, 1.15);
  color: var(--font-sample-color, var(--ui-text-secondary));
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
</style>