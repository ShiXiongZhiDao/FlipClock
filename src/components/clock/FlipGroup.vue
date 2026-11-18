<script setup lang="ts">
import { computed } from "vue";

import FlipDigit from "./FlipDigit.vue";
import type { DigitLayout } from "../../types/settings";

const props = defineProps<{
  value: string;
  halfDuration: number;
  /** 一位一卡时每位一张，两位一卡时整组一张。 */
  layout: DigitLayout;
}>();

const perPair = computed(() => props.layout === "per-pair");

/**
 * 每张卡要显示的内容。两位一卡时整组是一个字符串（"12"），
 * 不再拆字符——卡片本身就是一层叶片，装几个字由排版决定。
 */
const cards = computed(() => (perPair.value ? [props.value] : [...props.value]));
</script>

<template>
  <div class="digit-group" :class="{ 'digit-group--pair': perPair }">
    <FlipDigit
      v-for="(content, index) in cards"
      :key="`${layout}-${index}`"
      :value="content"
      :half-duration="halfDuration"
    />
  </div>
</template>

<style scoped>
.digit-group {
  display: flex;
  gap: var(--card-gap);
}

/* 两位一卡的卡片里是两个字形，卡内不该再留卡缝。 */
.digit-group--pair {
  gap: 0;
}
</style>
