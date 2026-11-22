<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch, watchEffect } from "vue";

import FlipGroup from "./FlipGroup.vue";
import { playFlipSound } from "../../lib/flipSound";
import { ensureFontLoaded, fontStyleVars, getFontEntry } from "../../lib/fonts";
import { locale } from "../../lib/i18n";
import { buildThemeVars } from "../../lib/theme/material";
import type { DigitLayout, ThemeSpec, TimeFormat } from "../../types/settings";

const props = defineProps<{
  now: Date;
  format: TimeFormat;
  showDate: boolean;
  showSeconds: boolean;
  /** 主题规格。内置主题与自定义主题用同一个结构（ADR-0020）。 */
  theme: ThemeSpec;
  /** 已转成可加载地址的背景图与数字纹理图；不给就是纯色。 */
  backgroundUrl?: string | null;
  digitUrl?: string | null;
  /** 每张翻页卡承载几位数字。两种排布共用同一套翻页动画，只有几何与分组不同。 */
  layout: DigitLayout;
  /** 时、分之间的两个小圆点。关掉时连间距一起收起，不留空洞。 */
  showColon: boolean;
  /** 钟面字体 id：内置 id，或字体库里的 `custom-<uuid>`（ADR-0026）。 */
  font: string;
  /** 钟面整体亮度（只压暗），1 = 原样。 */
  brightness: number;
  /** stage = 全屏屏保，panel = 嵌在卡片里(按容器短边缩放)，thumb = 系统预览的缩略画布 */
  variant: "stage" | "panel" | "thumb";
  /**
   * 用户设定的时钟缩放比例。stage 按视口短边（vmin）换算，
   * panel 按容器短边（cqmin）换算——卡片就是一块小屏，两边同一套几何。
   */
  scale?: number;
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

/**
 * 冒号透明度调到 0 时整块不渲染。
 * 不是因为画不出来——rgba(...,0) 本来就不绘制——而是省掉两个空元素与它们占的宽度，
 * 让「不要冒号」和「关掉显示冒号」在几何上完全等价。
 */
const separatorVisible = computed(
  () => props.showColon && props.theme.separator.opacity > 0,
);

/**
 * 一份主题规格翻成整套 CSS 变量（ADR-0020）。
 * 主题不再靠 `[data-clock-theme]` 属性选择器生效——自定义主题没有预置的类，
 * 所以内置主题也一起走运行时注入，避免两套机制并存。
 */
const themeVars = computed(() =>
  buildThemeVars(props.theme, {
    background: props.backgroundUrl,
    digit: props.digitUrl,
  }),
);

const clockRoot = ref<HTMLElement | null>(null);
const visible = ref(false);
let fontObserver: IntersectionObserver | null = null;

onMounted(() => {
  if (!clockRoot.value) return;
  if (typeof IntersectionObserver !== "function") {
    visible.value = true;
    return;
  }
  fontObserver = new IntersectionObserver(
    (entries) => {
      if (entries.some((entry) => entry.isIntersecting)) visible.value = true;
    },
    { rootMargin: "64px" },
  );
  fontObserver.observe(clockRoot.value);
});

onBeforeUnmount(() => fontObserver?.disconnect());

/** 导入字体按可见性加载；主题墙里的离屏钟面不下载字体。 */
watchEffect(() => {
  if (!visible.value) return;
  const entry = getFontEntry(props.font);
  if (entry) void ensureFontLoaded(entry);
});

const fontVars = computed(() => fontStyleVars(props.font));

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
    ref="clockRoot"
    class="clock"
    :class="[
      `clock--${variant}`,
      { 'clock--pair': layout === 'per-pair', 'clock--seconds': showSeconds },
    ]"
    :data-clock-theme="theme.id"
    :style="[
      themeVars,
      fontVars,
      {
        '--flip-half': `${halfDuration}ms`,
        '--user-scale': scaleValue,
        '--brightness': brightness,
      },
    ]"
  >
    <div v-if="showDate" class="date-line">{{ dateText }}</div>

    <div class="clock-row">
      <FlipGroup :value="hours" :half-duration="halfDuration" :layout="layout" />

      <div v-if="separatorVisible" class="separator" aria-hidden="true"><i></i><i></i></div>

      <FlipGroup :value="minutes" :half-duration="halfDuration" :layout="layout" />

      <template v-if="showSeconds">
        <div v-if="separatorVisible" class="separator" aria-hidden="true"><i></i><i></i></div>
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
  /* 字体分两处给：数字读 --font-clock；日期行与 AM/PM 读 --font-clock-face，
     再回落到界面字体，保证缺 CJK 时仍能显示。 */
  /* 亮度只压暗整块钟面：背景、卡片、数字、日期一起变，纯黑仍是纯黑。 */
  filter: brightness(var(--brightness, 1));
  /* 四层自下而上：背景图 → 底色蒙版 → 压暗 → 光晕。
     蒙版与压暗都是「用一层纯色盖住」，而不是给图片设 opacity——
     后者会把整块背景连底色一起变淡。 */
  background-color: var(--clock-bg);
  background-image:
    radial-gradient(ellipse at 50% 42%, var(--clock-glow), transparent 64%),
    var(--background-dim, linear-gradient(transparent, transparent)),
    var(--background-veil, linear-gradient(transparent, transparent)),
    var(--background-image, none);
  background-size: auto, auto, auto, var(--background-size, cover);
  background-position: center;
  background-repeat: no-repeat, no-repeat, no-repeat, var(--background-repeat, no-repeat);
  overflow: hidden;
  --font-scale: var(--font-scale-single, 1);
  /* 主题里的阴影长度写在 em 上，em 解析的是 .flip-card 的字号（见 FlipDigit）。
     全屏与屏保**保持 13px 不变**：改造前这就是它继承到的值，阴影是恒定像素
     （1080p 默认缩放下约 18px），改这个数等于悄悄换了真机的材质。
     预览（.clock--panel）才按卡宽换算，理由见那一节。 */
  --card-font: 13px;
}

/* 两位一卡时两个字形挤一张卡，字号要按该字体在双位卡里重新标定。 */
.clock--pair {
  --font-scale: var(--font-scale-pair, 1);
}

/* 全屏/屏保：所有尺寸都由 --unit = 1vmin × 用户缩放 推导。
   放大到超出屏幕时卡片会被裁掉——这是 450% 上限的预期行为。

   --row-fit 修正的是另一件事：**行宽会随排布和秒数变化**。一位一卡不带秒
   一行是 73.65u（u 以卡宽 16.5 为 1），开了秒变成 113.15u——同一个缩放值下
   后者会宽出 53%，表现就是「开了秒之后时间被切掉」。这里把 unit 按行宽
   等比收回，让「行占屏比例」只由 scale 决定，与排布/秒数无关。
   数值 = 不带秒的行宽 ÷ 带秒的行宽，改动几何常量时要一起重算。 */
.clock--seconds {
  --row-fit: 0.651; /* 73.65 / 113.15，一位一卡 */
}

.clock--seconds.clock--pair {
  --row-fit: 0.645; /* 73.5 / 114，两位一卡 */
}

.clock--stage {
  position: fixed;
  inset: 0;
  --unit: calc(1vmin * var(--user-scale, 1) * var(--row-fit, 1));
}

/* 窗口在钟面画出来之前铺的是主题底色（ADR-0031）。淡入 200ms 把「底色 → 钟面」
   这一次硬切抹平；只给整屏形态，卡片里的小预览不需要入场动画。 */
@media (prefers-reduced-motion: no-preference) {
  .clock--stage {
    animation: clock-in 200ms ease-out both;
  }
}

@keyframes clock-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

/* 嵌在卡片里：**把这块预览卡当成一块屏幕**。
   单位与 stage 完全同源：stage 的 1vmin ↔ 面板的 1cqmin（容器短边的 1%），
   再乘同一个用户缩放与 --row-fit。于是卡片上的占屏比例就是真屏的占屏比例，
   该裁的时候（311% 这种超出）也一起裁。 */
.clock--panel {
  width: 100%;
  height: 100%;
  --unit: calc(1cqmin * var(--user-scale, 1) * var(--row-fit, 1));
  /* 阴影字号：预览卡里按卡宽换算，否则 18px 的模糊落在 33px 的卡上会糊成一团光。
     除数 = 参考档的卡宽 ÷ 13px：1080p、默认 225%、一位一卡时卡宽 401px → 30.8。
     全屏那一侧的阴影仍是恒定像素（见 .clock 的 --card-font），所以只有在参考档上
     两者的"阴影/卡宽"比值相同；把缩放拉大或换更高分辨率的屏幕时，预览的阴影会
     比真机偏轻——这是为了让预览可读而接受的近似，几何（裁切）依旧精确。 */
  --card-font: calc(var(--card-w) / 30.8);
  border-radius: var(--radius-card);
}

/* stage 与 panel 共用同一组几何：只有单位不同（vmin / cqmin），倍数必须一致，
   改任何一行都是两边一起改。 */
.clock--stage,
.clock--panel {
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
.clock--stage.clock--pair,
.clock--panel.clock--pair {
  --card-w: calc(33 * var(--unit));
  --card-h: var(--card-w);
  --digit-size: calc(34.5 * var(--unit));
  --group-gap: calc(3.2 * var(--unit));
  --sep-dot: calc(1.1 * var(--unit));
  --date-size: calc(2.6 * var(--unit));
  --radius: calc(2.64 * var(--unit));
  --clock-gap: calc(3.2 * var(--unit));
}

.clock--panel.clock--pair {
  /* 方卡宽是竖卡的两倍，同一个模糊要维持同样的相对重量，除数也要翻倍：
     1080p / 默认 225% 下方卡 802px ÷ 13px = 61.7。 */
  --card-font: calc(var(--card-w) / 61.7);
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
  /* 分割线是可独立调的部件；未给变量时退回数字色与原来的 0.62。 */
  background: var(--separator-color, var(--digit));
  opacity: var(--separator-opacity, 0.62);
}

.date-line {
  font-family: var(--font-clock-face, var(--font-ui)), var(--font-ui);
  font-size: var(--date-size);
  font-weight: var(--font-text-weight, 500);
  font-synthesis: none;
  letter-spacing: 0.26em;
  text-transform: uppercase;
  color: var(--date-color);
  white-space: nowrap;
  /* 日期与 AM/PM 也可以走金属渐变，与数字呼应。
     默认四个变量都不生效，老主题仍是纯色 —— 逐像素不变。 */
  background-image: var(--date-fill, none);
  -webkit-background-clip: var(--date-clip, border-box);
  background-clip: var(--date-clip, border-box);
  -webkit-text-fill-color: var(--date-ink, currentColor);
  -webkit-text-stroke-width: var(--date-stroke-width, 0);
  -webkit-text-stroke-color: var(--date-stroke-color, transparent);
  paint-order: var(--date-paint-order, normal);
}

.meridiem {
  font-family: var(--font-clock-face, var(--font-ui)), var(--font-ui);
  font-size: calc(var(--date-size) * 0.92);
  font-weight: var(--font-text-weight, 500);
  font-synthesis: none;
  letter-spacing: 0.16em;
  color: var(--date-color);
  align-self: flex-start;
  margin-top: calc(var(--card-h) * 0.14);
  /* 与日期行同一套：不跟着渐变的话，12 小时制下 AM/PM 会是唯一一块素色。 */
  background-image: var(--date-fill, none);
  -webkit-background-clip: var(--date-clip, border-box);
  background-clip: var(--date-clip, border-box);
  -webkit-text-fill-color: var(--date-ink, currentColor);
  -webkit-text-stroke-width: var(--date-stroke-width, 0);
  -webkit-text-stroke-color: var(--date-stroke-color, transparent);
  paint-order: var(--date-paint-order, normal);
}
</style>
