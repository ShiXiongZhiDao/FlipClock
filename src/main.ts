import { createApp } from "vue";
import App from "./App.vue";

import "./styles/tokens.css";
import "./styles/themes.css";
import "./styles/fonts.css";
import "./styles/global.css";
import "./styles/ui.css";

import { watchStageAspect } from "./lib/stageAspect";
import { preloadClockAssets } from "./lib/theme/preload";
import { readBoot } from "./types/boot";

/**
 * 全软件禁用原生右键菜单。
 *
 * 放在入口而不是某个视图里：所有窗口（设置、全屏配置、屏保、时钟预览）都走这份
 * 入口，一处覆盖全部形态。用捕获阶段确保先于组件内的监听处理。
 *
 * 屏保视图原先自己抑制了一次右键，这里已经统一接管，它只保留 dragstart 抑制。
 * 取舍见 docs/adr/0024：输入框里的右键复制/粘贴一并没了，Ctrl+C/V 不受影响。
 */
document.addEventListener("contextmenu", (event) => event.preventDefault(), true);

// 只有带主题样卡/左栏预览的窗口需要舞台比例；屏保与全屏时钟不挂监听。
const bootMode = readBoot().mode;
if (bootMode === "settings" || bootMode === "theme-preview") {
  watchStageAspect();
}

/**
 * 屏保与全屏时钟先等主题图片与导入字体就位再挂载（ADR-0030 修订）。
 *
 * 不等的话第一帧是纯色底 + 回退字体，资产到位后再跳一次；自定义主题的底色
 * 常常接近经典碳灰，看起来就像「先闪了一下默认主题」。
 */
async function bootstrap(): Promise<void> {
  if (bootMode === "screensaver" || bootMode === "clock") {
    await preloadClockAssets();
  }
  createApp(App).mount("#app");
}

void bootstrap();
