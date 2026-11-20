<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";

const props = defineProps<{
  value: string;
  /** 单片翻折时长；下片会提前 OVERLAP 毫秒启动，所以整页略短于两倍。 */
  halfDuration: number;
}>();

/**
 * 两片叶片的重叠时长。下片不等上片完全折平就开始展开（提前 40ms），
 * 观感上才是一口气翻过去，而不是两段动画拼接。
 */
const OVERLAP = 40;

/**
 * 已落定的值：不翻页时上下静止片都显示它；翻页途中仍是旧值，落片结束才更新。
 */
const settled = ref(props.value);
/** 上静止片。翻页一开始就换成新值——上叶片折走后露出的必须是新数字。 */
const top = ref(props.value);
/** 下静止片。必须保持旧值，直到新叶片落下来盖住它。 */
const bottom = ref(props.value);
const flapTop = ref(props.value);
const flapBottom = ref(props.value);
const flipping = ref(false);

const delay = computed(() => Math.max(0, props.halfDuration - OVERLAP));
const total = computed(() => props.halfDuration + delay.value);

/** 翻页途中又来了新目标时，记住最后一个，等本次落片完成后接着翻。 */
let pending: string | null = null;
let timer: ReturnType<typeof window.setTimeout> | undefined;

function commit() {
  settled.value = flapBottom.value;
  top.value = settled.value;
  bottom.value = settled.value;
  flipping.value = false;

  if (pending !== null && pending !== settled.value) {
    const next = pending;
    pending = null;
    begin(next);
  } else {
    pending = null;
  }
}

function begin(next: string) {
  if (next === settled.value) return;

  flipping.value = true;
  flapTop.value = settled.value;
  flapBottom.value = next;
  // 上静止片立刻换新：上叶片折走后露出的就是新数字。
  // 下静止片保持旧值：翻入片落下来之前，下半截必须还是旧数字。
  // （这两行原来写反了，导致下半截瞬切、上半截在 280–560ms 退回旧值。）
  top.value = next;

  clearTimeout(timer);
  timer = setTimeout(commit, total.value);
}

watch(
  () => props.value,
  (next) => {
    if (flipping.value) {
      pending = next;
      return;
    }
    begin(next);
  },
);

onBeforeUnmount(() => clearTimeout(timer));
</script>

<template>
  <div class="flip-card" :class="{ flipping }" :style="{ '--flip-delay': `${delay}ms` }">
    <div class="half top"><span>{{ top }}</span></div>
    <div class="half bottom"><span>{{ bottom }}</span></div>
    <div class="flap top"><span>{{ flapTop }}</span></div>
    <div class="flap bottom"><span>{{ flapBottom }}</span></div>
    <div class="seam"></div>
  </div>
</template>

<style scoped>
.flip-card {
  position: relative;
  flex: none;
  width: var(--card-w);
  height: var(--card-h);
  border-radius: var(--card-radius, var(--radius));
  perspective: calc(var(--card-h) * 6);
  /* 主题里的阴影长度用 em 写（0.6em / 1.4em…），em 解析的是**这里的**字号。
     所以这个字号必须与卡宽成比例，否则阴影是恒定像素：主题卡里 33px 的卡配上
     18px 的模糊，整块卡糊成一团光，与全屏看到的不是同一个东西。
     除数 16.9 来自 1080p、默认缩放下的卡宽 220px ÷ 基准字号 13px，
     即「默认那一档的渲染逐像素不变」，其它尺寸按比例跟随。 */
  font-size: var(--card-font, inherit);
  background: var(--card-bottom);
  /* --card-bevel 让镜面主题换上更硬的金属倒角；--card-edge 画一圈细金边
     （黑标典藏金）。两者都有默认值，老主题的 box-shadow 一字不变。 */
  box-shadow:
    var(--card-shadow),
    inset 0 0.077em var(--card-bevel, rgba(255, 255, 255, 0.07)),
    inset 0 0 0 1px var(--card-edge, transparent);
  overflow: hidden;
}

/* 卡内那圈细金线（黑标典藏金）：默认 display:none，只有设了 --card-inset-ring
   的主题才生成，所以不给其他主题多出一层绘制。 */
.flip-card::after {
  content: "";
  display: var(--card-inset-display, none);
  position: absolute;
  z-index: 6;
  /* 与卡宽成比例：3px 是 220px 卡宽下的取值。 */
  inset: 0.23em;
  border: 1px solid var(--card-inset-ring, transparent);
  border-radius: calc(var(--card-radius, var(--radius)) * 0.6);
  pointer-events: none;
}

.half,
.flap {
  position: absolute;
  left: 0;
  width: 100%;
  height: 50%;
  overflow: hidden;
  backface-visibility: hidden;
}

.half span,
.flap span {
  position: absolute;
  left: 0;
  width: 100%;
  height: 200%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-clock);
  font-weight: var(--digit-weight, 400);
  font-size: calc(var(--digit-size) * var(--font-scale, 1));
  line-height: 1;
  color: var(--digit);
  /* 阴影分两条路：纯色数字用 text-shadow，渐变填充的数字必须用 drop-shadow。
     text-shadow 从字形内部透出来，会把 background-clip:text 的填充盖成暗色
     （实测 0.85 的黑影直接把浅金渐变压成 #252019）。drop-shadow 作用在
     渲染后的图像上，阴影只落在字形外面。默认值保持老主题逐像素不变。 */
  text-shadow: var(--digit-shadow-layer, 0 1px var(--digit-shadow));
  filter: var(--digit-filter, none);
  /* 数字可以是纯色，也可以是一层金属渐变（镜面土豪金那种深咖浮雕）。
     四个变量默认值都让渐变不生效，所以老主题的渲染逐像素不变。 */
  background-image: var(--digit-fill, none);
  -webkit-background-clip: var(--digit-clip, border-box);
  background-clip: var(--digit-clip, border-box);
  -webkit-text-fill-color: var(--digit-ink, currentColor);
  /* 金属倒角的细亮边。宽度默认 0，只有土豪金那两个主题才画；
     paint-order 让描边落在填充下面，只在外缘露出一点，不会糊住渐变。 */
  -webkit-text-stroke-width: var(--digit-stroke-width, 0);
  -webkit-text-stroke-color: var(--digit-stroke-color, transparent);
  paint-order: var(--digit-paint-order, normal);
  /* 只有纹理图填充才用元素级透明度（图片无法「带 alpha 的颜色」）；
     纯色与渐变走颜色自带的 alpha，这里保持 1。 */
  opacity: var(--digit-opacity, 1);
  letter-spacing: var(--digit-spacing, -0.02em);
  transform: translateY(var(--digit-shift, 0));
  /* 两位一卡时一张卡里是两个字形，绝不能折行——一折行整块钟面就散了。 */
  white-space: nowrap;
}

.half.top {
  top: 0;
  border-radius: var(--card-radius, var(--radius)) var(--card-radius, var(--radius)) 0 0;
  /* --card-top-fill 让主题换成多段对角渐变（镜面反射）；
     未定义时退回原来的两色垂直渐变，老主题不变。 */
  background: var(--card-top-fill, linear-gradient(180deg, var(--card-top), var(--card-top-end)));
}

.half.top span {
  top: 0;
}

.half.bottom {
  bottom: 0;
  border-radius: 0 0 var(--card-radius, var(--radius)) var(--card-radius, var(--radius));
  background: var(
    --card-bottom-fill,
    linear-gradient(180deg, var(--card-bottom-start), var(--card-bottom))
  );
}

.half.bottom span {
  bottom: 0;
}

.flap {
  display: none;
  z-index: 5;
}

.flip-card.flipping .flap {
  display: block;
}

.flap.top {
  top: 0;
  transform-origin: bottom;
  /* 翻转的半片必须和静态半片用同一层填充，否则翻到一半会跳色。 */
  background: var(--card-top-fill, var(--card-top));
  border-radius: var(--card-radius, var(--radius)) var(--card-radius, var(--radius)) 0 0;
}

.flap.top span {
  top: 0;
}

.flap.bottom {
  bottom: 0;
  transform-origin: top;
  transform: rotateX(90deg);
  background: var(--card-bottom-fill, var(--card-bottom));
  border-radius: 0 0 var(--card-radius, var(--radius)) var(--card-radius, var(--radius));
}

.flap.bottom span {
  bottom: 0;
}

.flip-card.flipping .flap.top {
  animation: fold-top var(--flip-half) cubic-bezier(0.35, 0.05, 0.7, 0.25) forwards;
}

/* 下片在 --flip-delay（半片时长 − 40ms）后启动；延迟期间保持 rotateX(90deg)
   的侧立姿态（不可见），所以下半截露出的仍是旧数字。 */
.flip-card.flipping .flap.bottom {
  animation: unfold-bottom var(--flip-half) var(--flip-delay)
    cubic-bezier(0.18, 0.7, 0.3, 1) forwards;
}

@keyframes fold-top {
  from {
    transform: rotateX(0deg);
  }
  to {
    transform: rotateX(-90deg);
  }
}

@keyframes unfold-bottom {
  from {
    transform: rotateX(90deg);
  }
  to {
    transform: rotateX(0deg);
  }
}

.seam {
  position: absolute;
  z-index: 7;
  left: 0;
  right: 0;
  top: calc(50% - 0.5px);
  height: 1px;
  background: var(--seam);
  box-shadow: 0 1px var(--seam-light);
}
</style>
