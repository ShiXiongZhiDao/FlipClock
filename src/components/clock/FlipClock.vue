<script setup lang="ts">
import { computed, watch } from "vue";

import FlipGroup from "./FlipGroup.vue";
import { playFlipSound } from "../../lib/flipSound";
import { locale } from "../../lib/i18n";
import type { DigitFont, DigitLayout, Theme, TimeFormat } from "../../types/settings";

const props = defineProps<{
  now: Date;
  format: TimeFormat;
  showDate: boolean;
  showSeconds: boolean;
  theme: Theme;
  /** 每张翻页卡承载几位数字。两种排布共用同一套翻页动画，只有几何与分组不同。 */
  layout: DigitLayout;
  /** 时、分之间的两个小圆点。关掉时连间距一起收起，不留空洞。 */
  showColon: boolean;
  /** 数字字体。只影响钟面数字，日期行与 AM/PM 仍用界面字体。 */
  font: DigitFont;
  /** 钟面整体亮度（只压暗），1 = 原样。 */
  brightness: number;
  /** stage = 全屏屏保，panel = 嵌在卡片里(按容器宽度缩放)，thumb = 系统预览的缩略画布 */
  variant: "stage" | "panel" | "thumb";
  /** 用户设定的时钟缩放比例，只作用于 stage；另两种形态按容器自适应。 */
  scale?: number;
  /**
   * 面板形态下额外乘的系数，用来在预览卡里演示时钟缩放：
   * 1 表示刚好铺满卡片。只有「时间与日期」页会传。
   */
  previewScale?: number;
  /** 翻页音效；为 null 或 enabled=false 时不发声。 */
  sound?: { enabled: boolean; volume: number } | null;
}>();

const pad = (n: number) => String(n).padStart(2, "0");

const reducedMotion =
  typeof window.matchMedia === "function" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const halfDuration = reducedMotion ? 0 : 280;

const hours = computed(() => {
  const h = props.now.getHours();
  return pad(props.format === "12h" ? h % 12 || 12 : h);
});

const minutes = computed(() => pad(props.now.getMinutes()));
const seconds = computed(() => pad(props.now.getSeconds()));

const meridiem = computed(() => {
  if (props.format !== "12h") return "";
  return props.now.getHours() < 12 ? "AM" : "PM";
});

const scaleValue = computed(() => props.scale ?? 1);
const previewScale = computed(() => props.previewScale ?? 1);

/** 只统计真正显示出来的数字：秒数关掉时，秒的变化不该触发音效。 */
const stamp = computed(() =>
  props.showSeconds
    ? `${hours.value}${minutes.value}${seconds.value}`
    : `${hours.value}${minutes.value}`,
);

/**
 * 一次走时只响一次。三组卡片可能在同一秒一起翻，逐卡发声会叠成三倍音量；
 * 只有 stage 形态发声——设置窗口里每秒滴答会持续打断用户操作。
 */
watch(stamp, () => {
  if (props.variant !== "stage" || !props.sound?.enabled) return;
  playFlipSound(props.sound.volume);
});

// 日期行跟「生效语言」走，不再独立读 navigator.language（ADR-0018）：
// 用户选了英文界面就不该看到中文日期。固定用 zh-CN/en-US 两个 locale，
// 与 i18n 的归并规则一致，也避免地区变体带来的格式漂移。
const dateLocale = computed(() => (locale.value === "zh" ? "zh-CN" : "en-US"));
const dateText = computed(() => {
  const formatted = new Intl.DateTimeFormat(dateLocale.value, {
    weekday: "short",
    month: "short",
    day: "numeric",
  }).format(props.now);

  // 中文里「月」「日」在 ICU 里是 literal 而不是 month/day 字段，
  // 所以不能按 parts 拼，只能整串取再补一个空格。
  if (locale.value === "zh") {
    return formatted.replace(/\s*(周[一二三四五六日天])/, " $1");
  }
  return formatted.replace(/,/g, " · ").toUpperCase();
});
</script>

<template>
  <div
    class="clock"
    :class="[`clock--${variant}`, { 'clock--pair': layout === 'per-pair' }]"
    :data-clock-theme="theme"
    :data-clock-font="font"
    :style="{
      '--flip-half': `${halfDuration}ms`,
      '--user-scale': scaleValue,
      '--preview-scale': previewScale,
      '--brightness': brightness,
    }"
  >
    <div v-if="showDate" class="date-line">{{ dateText }}</div>

    <div class="clock-row">
      <FlipGroup :value="hours" :half-duration="halfDuration" :layout="layout" />

      <div v-if="showColon" class="separator" aria-hidden="true"><i></i><i></i></div>

      <FlipGroup :value="minutes" :half-duration="halfDuration" :layout="layout" />

      <template v-if="showSeconds">
        <div v-if="showColon" class="separator" aria-hidden="true"><i></i><i></i></div>
        <FlipGroup :value="seconds" :half-duration="halfDuration" :layout="layout" />
      </template>

      <div v-if="meridiem" class="meridiem">{{ meridiem }}</div>
    </div>
  </div>
</template>

<style scoped>
.clock {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--clock-gap);
  /* 字体分两处给：数字读 --font-clock，日期行与 AM/PM 读 --font-ui。
     这里不设兜底 family，否则是一条永远被覆盖的死声明。 */
  /* 亮度只压暗整块钟面：背景、卡片、数字、日期一起变，纯黑仍是纯黑。 */
  filter: brightness(var(--brightness, 1));
  background:
    radial-gradient(ellipse at 50% 42%, var(--clock-glow), transparent 64%),
    var(--clock-bg);
  overflow: hidden;
  --font-scale: var(--font-scale-single, 1);
}

/* 两位一卡时两个字形挤一张卡，字号要按该字体在双位卡里重新标定。 */
.clock--pair {
  --font-scale: var(--font-scale-pair, 1);
}

/* 全屏/屏保：所有尺寸都由 --unit = 1vmin × 用户缩放 推导。
   放大到超出屏幕时卡片会被裁掉——这是 450% 上限的预期行为。 */
.clock--stage {
  position: fixed;
  inset: 0;
  --unit: calc(1vmin * var(--user-scale, 1));
  --clock-gap: calc(3.6 * var(--unit));
  --card-w: calc(16.5 * var(--unit));
  --card-h: calc(23.2 * var(--unit));
  --digit-size: calc(18.8 * var(--unit));
  --card-gap: calc(1.15 * var(--unit));
  --group-gap: calc(2.3 * var(--unit));
  --sep-dot: calc(0.75 * var(--unit));
  --date-size: calc(2.2 * var(--unit));
  --radius: calc(1.3 * var(--unit));
}

/* 两位一卡：两张 1:1 方卡。卡宽按「行宽对齐」构造：
   2×33 + 2×3.2 组距 + 1.1 分隔点 = 73.5，与一位一卡的 73.65 几乎相等，
   所以两套排布共用同一个 CLOCK_FIT_SCALE，不必再维护第二个魔数。 */
.clock--stage.clock--pair {
  --card-w: calc(33 * var(--unit));
  --card-h: var(--card-w);
  --digit-size: calc(34.5 * var(--unit));
  --group-gap: calc(3.2 * var(--unit));
  --sep-dot: calc(1.1 * var(--unit));
  --date-size: calc(2.6 * var(--unit));
  --radius: calc(2.64 * var(--unit));
  --clock-gap: calc(3.2 * var(--unit));
}

/* 嵌在卡片里：卡片尺寸同时受容器宽高约束（宽高取小者），
   所以无论放进宽扁的横幅还是接近方形的大舞台，时钟都不会溢出或缩成一小撮。
   --preview-scale 只给「时间与日期」页用：按「最大值刚好铺满卡片」标定，
   拖动缩放滑杆时这张卡里的时钟会跟着变大变小。容器需要 container-type: size。 */
.clock--panel {
  width: 100%;
  height: 100%;
  --card-w: calc(min(12.6cqw, 45cqh) * var(--preview-scale, 1));
  --card-h: calc(var(--card-w) * 1.474);
  --digit-size: calc(var(--card-w) * 1.21);
  --card-gap: calc(var(--card-w) * 0.09);
  --group-gap: calc(var(--card-w) * 0.16);
  --sep-dot: calc(var(--card-w) * 0.079);
  --date-size: calc(var(--card-w) * 0.145);
  --radius: calc(var(--card-w) * 0.105);
  --clock-gap: calc(var(--card-w) * 0.34);
  padding: 1.6cqw;
  border-radius: var(--radius-card);
}

/* 同样按行宽对齐：一位一卡是 4×12.6cqw，两位一卡是 2×23cqw，两者行宽都在 51cqw 附近。
   高度上限比一位一卡高（62cqh）：方卡的卡高等于卡宽，沿用 45cqh 会让它在
   扁长的预览卡里缩成一小撮；一位一卡的 45cqh 折算成卡高本来就是 66cqh。 */
.clock--panel.clock--pair {
  --card-w: calc(min(23cqw, 62cqh) * var(--preview-scale, 1));
  --card-h: var(--card-w);
  --digit-size: calc(var(--card-w) * 1.045);
  --group-gap: calc(var(--card-w) * 0.097);
  --sep-dot: calc(var(--card-w) * 0.033);
  --date-size: calc(var(--card-w) * 0.079);
  --radius: calc(var(--card-w) * 0.08);
  --clock-gap: calc(var(--card-w) * 0.097);
}

.clock--thumb {
  position: absolute;
  inset: 0;
  --clock-gap: 30px;
  --card-w: 148px;
  --card-h: 208px;
  --digit-size: 168px;
  --card-gap: 10px;
  --group-gap: 20px;
  --sep-dot: 7px;
  --date-size: 24px;
  --radius: 12px;
}

.clock--thumb.clock--pair {
  --card-w: 296px;
  --card-h: 296px;
  --digit-size: 310px;
  --group-gap: 28px;
  --sep-dot: 10px;
  --date-size: 23px;
  --radius: 24px;
  --clock-gap: 28px;
}

.clock-row {
  display: flex;
  align-items: center;
  gap: var(--group-gap);
}

.separator {
  display: flex;
  flex-direction: column;
  gap: calc(var(--sep-dot) * 2.2);
}

.separator i {
  display: block;
  width: var(--sep-dot);
  height: var(--sep-dot);
  border-radius: 50%;
  background: var(--digit);
  opacity: 0.62;
}

.date-line {
  font-family: var(--font-ui);
  font-size: var(--date-size);
  font-weight: 500;
  letter-spacing: 0.26em;
  text-transform: uppercase;
  color: var(--date-color);
  white-space: nowrap;
}

.meridiem {
  font-family: var(--font-ui);
  font-size: calc(var(--date-size) * 0.92);
  font-weight: 700;
  letter-spacing: 0.16em;
  color: var(--date-color);
  align-self: flex-start;
  margin-top: calc(var(--card-h) * 0.14);
}
</style>
