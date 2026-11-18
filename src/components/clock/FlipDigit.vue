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
  border-radius: var(--radius);
  perspective: calc(var(--card-h) * 6);
  background: var(--card-bottom);
  box-shadow: var(--card-shadow), inset 0 1px rgba(255, 255, 255, 0.07);
  overflow: hidden;
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
  text-shadow: 0 1px var(--digit-shadow);
  letter-spacing: var(--digit-spacing, -0.02em);
  transform: translateY(var(--digit-shift, 0));
  /* 两位一卡时一张卡里是两个字形，绝不能折行——一折行整块钟面就散了。 */
  white-space: nowrap;
}

.half.top {
  top: 0;
  border-radius: var(--radius) var(--radius) 0 0;
  background: linear-gradient(180deg, var(--card-top), var(--card-top-end));
}

.half.top span {
  top: 0;
}

.half.bottom {
  bottom: 0;
  border-radius: 0 0 var(--radius) var(--radius);
  background: linear-gradient(180deg, var(--card-bottom-start), var(--card-bottom));
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
  background: var(--card-top);
  border-radius: var(--radius) var(--radius) 0 0;
}

.flap.top span {
  top: 0;
}

.flap.bottom {
  bottom: 0;
  transform-origin: top;
  transform: rotateX(90deg);
  background: var(--card-bottom);
  border-radius: 0 0 var(--radius) var(--radius);
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
