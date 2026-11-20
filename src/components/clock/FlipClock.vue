<script setup lang="ts">
import { computed, watch } from "vue";

import FlipGroup from "./FlipGroup.vue";
import { playFlipSound } from "../../lib/flipSound";
import { locale } from "../../lib/i18n";
import { buildThemeVars } from "../../lib/theme/material";
import type {
  DigitFont,
  DigitLayout,
  ThemeSpec,
  TimeFormat,
} from "../../types/settings";

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
  /** 数字字体。只影响钟面数字，日期行与 AM/PM 仍用界面字体。 */
  font: DigitFont;
  /** 钟面整体亮度（只压暗），1 = 原样。 */
  brightness: number;
  /** stage = 全屏屏保，panel = 嵌在卡片里(按容器宽度缩放)，thumb = 系统预览的缩略画布 */
  variant: "stage" | "panel" | "thumb";
  /** 用户设定的时钟缩放比例，只作用于 stage；另两种形态按容器自适应。 */
  scale?: number;
  /**
   * 面板形态下额外乘的系数 = 真实的占屏比例（clockScale / CLOCK_FIT_SCALE）。
   * 1 表示「刚好铺满这块预览卡」，>1 就会像真屏一样被裁。
   * 设置页左栏与两处主题墙都传，（全屏配置的）主题卡也不例外——
   * 同一个主题在卡片、左栏、屏保上必须是同一个大小。
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
    :class="[
      `clock--${variant}`,
      { 'clock--pair': layout === 'per-pair', 'clock--seconds': showSeconds },
    ]"
    :data-clock-theme="theme.id"
    :data-clock-font="font"
    :style="[
      themeVars,
      {
        '--flip-half': `${halfDuration}ms`,
        '--user-scale': scaleValue,
        '--preview-scale': previewScale,
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
  /* 字体分两处给：数字读 --font-clock，日期行与 AM/PM 读 --font-ui。
     这里不设兜底 family，否则是一条永远被覆盖的死声明。 */
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

/* 嵌在卡片里：**把这块预览卡当成一块屏幕**。
   先按行宽把卡宽定成「刚好铺满卡片」，再乘 --preview-scale（= 真实占屏比例，
   见 useClockScale）。于是预览与全屏的占屏比例是同一个数，超限时预览也会裁，
   不再出现「预览里装得下、屏保里切掉两侧」。

   cqw 那一路按行宽把卡宽定成刚好铺满，cqh 是安全网（极扁或极高的容器里不至于溢出），
   正常情况下宽度先命中。

   下面的子尺寸是 stage 的卡宽倍数（括号里是 stage 原值 ÷ 卡宽），改一边必须改另一边。 */
.clock--panel {
  width: 100%;
  height: 100%;
  --row-cards: 4.464; /* 73.65 / 16.5：一行有多少个卡宽（含间距、圆点） */
  --col-cards: 1.784; /* 卡高 + 钟面行距 + 日期行 */
  /* 100cqw：预览卡就是一块屏，不留额外余量。留余量会让预览在 240%~260% 这段
     「看着刚好」而真屏已经出界——用户就是按这个差值来报「预览正常、屏保被裁」的。 */
  --card-w: calc(
    min(100cqw / var(--row-cards), 100cqh / var(--col-cards)) * var(--preview-scale, 1)
  );
  /* 阴影字号：预览卡里按卡宽换算，否则 18px 的模糊落在 33px 的卡上会糊成一团光。
     除数 = 参考档的卡宽 ÷ 13px：1080p、默认 225%、一位一卡时卡宽 401px → 30.8。
     全屏那一侧的阴影仍是恒定像素（见 .clock 的 --card-font），所以只有在参考档上
     两者的"阴影/卡宽"比值相同；把缩放拉大或换更高分辨率的屏幕时，预览的阴影会
     比真机偏轻——这是为了让预览可读而接受的近似，几何（裁切）依旧精确。 */
  --card-font: calc(var(--card-w) / 30.8);
  --card-h: calc(var(--card-w) * 1.40606); /* 23.2 / 16.5 */
  --digit-size: calc(var(--card-w) * 1.13939); /* 18.8 / 16.5 */
  --card-gap: calc(var(--card-w) * 0.0697); /* 1.15 / 16.5 */
  --group-gap: calc(var(--card-w) * 0.13939); /* 2.3 / 16.5 */
  --sep-dot: calc(var(--card-w) * 0.04545); /* 0.75 / 16.5 */
  --date-size: calc(var(--card-w) * 0.13333); /* 2.2 / 16.5 */
  --radius: calc(var(--card-w) * 0.07879); /* 1.3 / 16.5 */
  --clock-gap: calc(var(--card-w) * 0.21818); /* 3.6 / 16.5 */
  border-radius: var(--radius-card);
}

.clock--panel.clock--seconds {
  --row-cards: 6.857; /* 113.15 / 16.5 */
}

/* 两位一卡同样按 stage 折算（33 是方卡的卡宽）。 */
.clock--panel.clock--pair {
  --row-cards: 2.227; /* 73.5 / 33 */
  --col-cards: 1.257; /* 卡高 + 行距 + 日期行，方卡更矮 */
  /* 方卡宽是竖卡的两倍，同一个模糊要维持同样的相对重量，除数也要翻倍：
     1080p / 默认 225% 下方卡 802px ÷ 13px = 61.7。 */
  --card-font: calc(var(--card-w) / 61.7);
  --card-h: var(--card-w);
  --digit-size: calc(var(--card-w) * 1.04545); /* 34.5 / 33 */
  --group-gap: calc(var(--card-w) * 0.09697); /* 3.2 / 33 */
  --sep-dot: calc(var(--card-w) * 0.03333); /* 1.1 / 33 */
  --date-size: calc(var(--card-w) * 0.07879); /* 2.6 / 33 */
  --radius: calc(var(--card-w) * 0.08); /* 2.64 / 33 */
  --clock-gap: calc(var(--card-w) * 0.09697); /* 3.2 / 33 */
}

.clock--panel.clock--pair.clock--seconds {
  --row-cards: 3.454; /* 114 / 33 */
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
  font-family: var(--font-ui);
  font-size: var(--date-size);
  font-weight: 500;
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
  font-family: var(--font-ui);
  font-size: calc(var(--date-size) * 0.92);
  font-weight: 700;
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
