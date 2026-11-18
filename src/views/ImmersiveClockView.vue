<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { onBeforeUnmount, onMounted } from "vue";

import FlipClock from "../components/clock/FlipClock.vue";
import { useClock } from "../composables/useClock";
import { useSettings } from "../composables/useSettings";
import { unlockFlipSound } from "../lib/flipSound";

const props = defineProps<{
  /** 屏保模式：任何输入都退出。全屏时钟模式：只有 Esc 退出。 */
  exitOnInput: boolean;
  action: "end" | "leave";
}>();

const { now } = useClock();
const { settings, load } = useSettings();

const SETTLE_GRACE = 700;
const JITTER = 3;
const mountedAt = Date.now();

let origin: { x: number; y: number } | null = null;
let leaving = false;

function leave() {
  if (leaving) return;
  leaving = true;
  const command = props.action === "end" ? "end_screensaver" : "leave_fullscreen_clock";
  invoke(command).catch(() => {
    leaving = false;
  });
}

function onKeyDown(event: KeyboardEvent) {
  event.preventDefault();
  event.stopPropagation();
  if (!props.exitOnInput && event.key !== "Escape") return;
  leave();
}

function onMouseDown(event: MouseEvent) {
  event.preventDefault();
  if (!props.exitOnInput) return;
  leave();
}

function onMouseMove(event: MouseEvent) {
  if (!props.exitOnInput) return;

  const settled = Date.now() - mountedAt > SETTLE_GRACE;
  if (!origin || !settled) {
    origin = { x: event.clientX, y: event.clientY };
    return;
  }
  if (
    Math.abs(event.clientX - origin.x) > JITTER ||
    Math.abs(event.clientY - origin.y) > JITTER
  ) {
    leave();
  }
}

/** 屏保里不该出现右键菜单、拖拽和文本选择。 */
function suppress(event: Event) {
  event.preventDefault();
}

onMounted(() => {
  load();
  // 屏保/全屏里没有用户点击，音频上下文只能靠窗口参数放开自动播放；
  // 这里再解开一次，兼顾浏览器里直接预览的情况。
  unlockFlipSound();
  window.addEventListener("keydown", onKeyDown, true);
  window.addEventListener("mousedown", onMouseDown, true);
  window.addEventListener("mousemove", onMouseMove, true);
  window.addEventListener("contextmenu", suppress, true);
  window.addEventListener("dragstart", suppress, true);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeyDown, true);
  window.removeEventListener("mousedown", onMouseDown, true);
  window.removeEventListener("mousemove", onMouseMove, true);
  window.removeEventListener("contextmenu", suppress, true);
  window.removeEventListener("dragstart", suppress, true);
});
</script>

<template>
  <div class="stage-host">
    <FlipClock
      :now="now"
      :format="settings.time.format"
      :show-date="settings.time.showDate"
      :show-seconds="settings.time.showSeconds"
      :theme="settings.appearance.theme"
      :layout="settings.appearance.digitLayout"
      :show-colon="settings.time.showColon"
      :font="settings.appearance.font"
      :brightness="settings.appearance.brightness"
      variant="stage"
      :scale="settings.appearance.clockScale"
      :sound="settings.sound"
    />
  </div>
</template>
