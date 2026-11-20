<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";

import FlipClock from "../components/clock/FlipClock.vue";
import { useClock } from "../composables/useClock";
import { useSettings } from "../composables/useSettings";
import { useTheme } from "../composables/useTheme";

/**
 * 系统预览的框只有约 150×110 逻辑像素。
 * 直接把整屏版式塞进去会挤成一团，所以渲染一张固定 1280×720 的画布再整体缩小——
 * 用户在系统设置里看到的就是「屏保缩小后的样子」。
 */
const CANVAS_WIDTH = 1280;
const CANVAS_HEIGHT = 720;

const { now } = useClock();
const { load } = useSettings();
const { current: currentTheme, profile, backgroundUrl, digitUrl } = useTheme();
const scale = ref(0.1);

function fit() {
  scale.value = Math.min(
    window.innerWidth / CANVAS_WIDTH,
    window.innerHeight / CANVAS_HEIGHT,
  );
}

onMounted(() => {
  load();
  fit();
  window.addEventListener("resize", fit);
});

onBeforeUnmount(() => window.removeEventListener("resize", fit));
</script>

<template>
  <div class="thumb-host">
    <div
      class="thumb-canvas"
      :style="{
        width: `${CANVAS_WIDTH}px`,
        height: `${CANVAS_HEIGHT}px`,
        transform: `scale(${scale})`,
      }"
    >
      <FlipClock
        :now="now"
        :format="profile.format"
        :show-date="profile.showDate"
        :show-seconds="profile.showSeconds"
        :theme="currentTheme"
        :background-url="backgroundUrl"
        :digit-url="digitUrl"
        :layout="profile.digitLayout"
        :show-colon="profile.showColon"
        :font="profile.font"
        :brightness="profile.brightness"
        variant="thumb"
      />
    </div>
  </div>
</template>

<style scoped>
.thumb-host {
  position: fixed;
  inset: 0;
  display: grid;
  place-items: center;
  overflow: hidden;
  background: var(--ui-preview-backdrop);
}

.thumb-canvas {
  flex: none;
  transform-origin: center center;
}
</style>
