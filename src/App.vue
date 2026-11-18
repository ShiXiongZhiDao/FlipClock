<script setup lang="ts">
import { computed } from "vue";

import ImmersiveClockView from "./views/ImmersiveClockView.vue";
import PreviewView from "./views/PreviewView.vue";
import SettingsView from "./views/SettingsView.vue";
import { readBoot } from "./types/boot";

const boot = readBoot();

// 调试窗口化运行时没有原生的输入监听，退回 DOM 事件也得能退出。
const saverExitsOnInput = computed(() => boot.exitOnInput || boot.debugWindowed);
</script>

<template>
  <SettingsView v-if="boot.mode === 'settings'" />
  <ImmersiveClockView
    v-else-if="boot.mode === 'screensaver'"
    :exit-on-input="saverExitsOnInput"
    action="end"
  />
  <ImmersiveClockView v-else-if="boot.mode === 'clock'" :exit-on-input="false" action="leave" />
  <PreviewView v-else />
</template>
